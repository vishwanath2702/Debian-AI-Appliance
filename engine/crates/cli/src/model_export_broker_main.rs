//! DAIA Model Export authorization broker.
//!
//! No caller identity, socket permission, or environment variable grants
//! authorization. An independently verified authorization mechanism must
//! be implemented before export operations are enabled.

use model::ModelExportOperation;

use model::{
    ModelExportArtifact, ModelExportDestinationSelection, ModelExportOperationId,
    ModelExportResourceSelection, ModelExportSource, ModelExportSourceSelection,
};
use serde::de::{self, Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fmt;
use std::io::{self, Read};

/// JSON value whose deserializer rejects duplicate keys recursively.
struct StrictJson(Value);

impl<'de> Deserialize<'de> for StrictJson {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct StrictVisitor;

        impl<'de> Visitor<'de> for StrictVisitor {
            type Value = StrictJson;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a JSON value without duplicate object keys")
            }

            fn visit_bool<E: de::Error>(self, v: bool) -> Result<Self::Value, E> {
                Ok(StrictJson(Value::Bool(v)))
            }

            fn visit_i64<E: de::Error>(self, v: i64) -> Result<Self::Value, E> {
                Ok(StrictJson(Value::from(v)))
            }

            fn visit_u64<E: de::Error>(self, v: u64) -> Result<Self::Value, E> {
                Ok(StrictJson(Value::from(v)))
            }

            fn visit_f64<E: de::Error>(self, v: f64) -> Result<Self::Value, E> {
                let number = serde_json::Number::from_f64(v)
                    .ok_or_else(|| E::custom("invalid JSON number"))?;
                Ok(StrictJson(Value::Number(number)))
            }

            fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
                Ok(StrictJson(Value::String(v.to_owned())))
            }

            fn visit_string<E: de::Error>(self, v: String) -> Result<Self::Value, E> {
                Ok(StrictJson(Value::String(v)))
            }

            fn visit_none<E: de::Error>(self) -> Result<Self::Value, E> {
                Ok(StrictJson(Value::Null))
            }

            fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
                Ok(StrictJson(Value::Null))
            }

            fn visit_some<D: Deserializer<'de>>(
                self,
                deserializer: D,
            ) -> Result<Self::Value, D::Error> {
                StrictJson::deserialize(deserializer)
            }

            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
                let mut values = Vec::new();
                while let Some(value) = seq.next_element::<StrictJson>()? {
                    values.push(value.0);
                }
                Ok(StrictJson(Value::Array(values)))
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut values = BTreeMap::new();
                while let Some((key, value)) = map.next_entry::<String, StrictJson>()? {
                    if values.insert(key, value.0).is_some() {
                        return Err(de::Error::custom("duplicate JSON object key"));
                    }
                }
                Ok(StrictJson(Value::Object(values.into_iter().collect())))
            }
        }

        deserializer.deserialize_any(StrictVisitor)
    }
}

fn exact_object<'a>(
    value: &'a Value,
    fields: &[&str],
) -> Result<&'a serde_json::Map<String, Value>, &'static str> {
    let object = value.as_object().ok_or("expected JSON object")?;
    if object.len() != fields.len() || object.keys().any(|key| !fields.contains(&key.as_str())) {
        return Err("missing or unexpected JSON field");
    }
    Ok(object)
}

fn string_field<'a>(
    object: &'a serde_json::Map<String, Value>,
    key: &str,
) -> Result<&'a str, &'static str> {
    object
        .get(key)
        .and_then(Value::as_str)
        .ok_or("invalid JSON string field")
}

/// Parsed request metadata only; this is not an authorization grant.
#[derive(Debug)]
struct ParsedExportRequest {
    operation: ModelExportOperation,
    selection: Option<ModelExportResourceSelection>,
}

/// Parse untrusted resource intent without accessing the filesystem.
fn parse_export_selection(
    value: &Value,
    operation_source: &ModelExportSource,
) -> Result<ModelExportResourceSelection, &'static str> {
    let selection = exact_object(value, &["source", "destination"])?;

    let source_value = selection.get("source").ok_or("missing source selection")?;
    let source_object = source_value.as_object().ok_or("invalid source selection")?;

    let source = match string_field(source_object, "kind")? {
        "gguf" => {
            let fields = exact_object(source_value, &["kind", "directory", "relative_file"])?;
            if !matches!(operation_source, ModelExportSource::Gguf) {
                return Err("selection source does not match operation");
            }
            ModelExportSourceSelection::gguf(
                string_field(fields, "directory")?,
                string_field(fields, "relative_file")?,
            )
            .map_err(|_| "invalid GGUF source selection")?
        }
        "ollama" => {
            let fields = exact_object(source_value, &["kind", "directory", "relative_manifest"])?;
            if !matches!(operation_source, ModelExportSource::Ollama) {
                return Err("selection source does not match operation");
            }
            ModelExportSourceSelection::ollama(
                string_field(fields, "directory")?,
                string_field(fields, "relative_manifest")?,
            )
            .map_err(|_| "invalid Ollama source selection")?
        }
        _ => return Err("invalid selection source kind"),
    };

    let destination = exact_object(
        selection
            .get("destination")
            .ok_or("missing destination selection")?,
        &["directory"],
    )?;

    let destination = ModelExportDestinationSelection::new(string_field(destination, "directory")?)
        .map_err(|_| "invalid destination selection")?;

    Ok(ModelExportResourceSelection::new(source, destination))
}

/// Parse metadata only. No authorization or file access occurs here.
fn parse_export_request(input: &[u8]) -> Result<ParsedExportRequest, &'static str> {
    const MAX_BYTES: usize = 1024 * 1024;

    if input.is_empty() || input.len() > MAX_BYTES {
        return Err("invalid export request size");
    }

    let mut deserializer = serde_json::Deserializer::from_slice(input);
    let root = StrictJson::deserialize(&mut deserializer)
        .map_err(|_| "invalid or ambiguous export JSON")?;
    deserializer
        .end()
        .map_err(|_| "trailing export JSON data")?;

    let version = root
        .0
        .as_object()
        .and_then(|fields| fields.get("version"))
        .ok_or("missing export request version")?;

    let envelope = match version.as_u64() {
        Some(1) => exact_object(&root.0, &["version", "operation"])?,
        Some(2) => exact_object(&root.0, &["version", "operation", "selection"])?,
        _ => return Err("unsupported export request version"),
    };

    let operation = exact_object(
        envelope.get("operation").ok_or("missing operation")?,
        &["id", "source", "destination_name", "artifacts"],
    )?;

    let id = ModelExportOperationId::new(string_field(operation, "id")?)
        .map_err(|_| "invalid operation ID")?;

    let source = match string_field(operation, "source")? {
        "gguf" => ModelExportSource::Gguf,
        "ollama" => ModelExportSource::Ollama,
        _ => return Err("invalid export source"),
    };

    let destination = string_field(operation, "destination_name")?;
    let items = operation
        .get("artifacts")
        .and_then(Value::as_array)
        .ok_or("invalid artifacts array")?;

    if items.is_empty() || items.len() > 100_000 {
        return Err("invalid artifact count");
    }

    let mut artifacts = Vec::with_capacity(items.len());
    for item in items {
        let fields = exact_object(item, &["relative_path", "sha256", "size_bytes"])?;

        let size = fields
            .get("size_bytes")
            .and_then(Value::as_u64)
            .ok_or("invalid artifact size")?;

        artifacts.push(
            ModelExportArtifact::new(
                string_field(fields, "relative_path")?,
                string_field(fields, "sha256")?,
                size,
            )
            .map_err(|_| "invalid artifact metadata")?,
        );
    }

    let operation = ModelExportOperation::new(id, source.clone(), destination, artifacts)
        .map_err(|_| "invalid export operation")?;

    let selection = match version.as_u64() {
        Some(1) => None,
        Some(2) => Some(parse_export_selection(
            envelope
                .get("selection")
                .ok_or("missing resource selection")?,
            &source,
        )?),
        _ => return Err("unsupported export request version"),
    };

    Ok(ParsedExportRequest {
        operation,
        selection,
    })
}

/// Check the broker's effective identity.
///
/// This is a prerequisite for the privileged broker, not evidence of
/// Polkit authentication or authorization for any export operation.
fn validate_broker_effective_uid(effective_uid: u32) -> Result<(), &'static str> {
    if effective_uid != 0 {
        return Err("DAIA Model Export broker requires privileged activation");
    }

    Ok(())
}

/// Validate the invoking UID supplied by the sanitized pkexec environment.
///
/// This establishes user attribution only. It is not an authorization grant.
fn validate_pkexec_invoking_uid(value: Option<&str>) -> Result<u32, &'static str> {
    let value = value.ok_or("missing pkexec invoking UID")?;

    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err("invalid pkexec invoking UID");
    }

    let uid = value
        .parse::<u32>()
        .map_err(|_| "invalid pkexec invoking UID")?;

    if uid.to_string() != value {
        return Err("non-canonical pkexec invoking UID");
    }

    Ok(uid)
}

/// Canonical binding of an export operation and its selected resources.
///
/// This encodes request intent only. It does not establish Polkit
/// authorization, resource identity, or safe filesystem access.
fn canonical_export_request_bytes(
    operation: &ModelExportOperation,
    selection: &ModelExportResourceSelection,
) -> Vec<u8> {
    let operation_bytes = operation.canonical_bytes();
    let selection_bytes = selection.canonical_bytes();

    let mut bytes = b"DAIA-MODEL-EXPORT-REQUEST\0".to_vec();
    bytes.extend_from_slice(&1_u32.to_be_bytes());

    for field in [&operation_bytes, &selection_bytes] {
        bytes.extend_from_slice(&(field.len() as u64).to_be_bytes());
        bytes.extend_from_slice(field);
    }

    bytes
}

/// An operation-specific authorization grant.
///
/// No production constructor exists until an independently trusted
/// authorization provider is implemented.
#[derive(Clone, Debug, Eq, PartialEq)]
struct VerifiedExportGrant {
    operation_bytes: Vec<u8>,
}

#[cfg(test)]
impl VerifiedExportGrant {
    fn for_test(operation_bytes: Vec<u8>) -> Self {
        Self { operation_bytes }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum AuthorizationDecision {
    Denied,
    VerifiedForOperation(VerifiedExportGrant),
}

fn authorize_export(
    decision: &AuthorizationDecision,
    operation: &ModelExportOperation,
    selection: Option<&ModelExportResourceSelection>,
) -> Result<(), &'static str> {
    let selection = selection.ok_or("DAIA Model Export resource selection required")?;

    match decision {
        AuthorizationDecision::VerifiedForOperation(grant)
            if grant.operation_bytes == canonical_export_request_bytes(operation, selection) =>
        {
            Ok(())
        }
        _ => Err("DAIA Model Export authorization unavailable"),
    }
}

/// The privileged broker accepts no command-line request or authorization claims.
/// All operation metadata must arrive through its bounded stdin parser.
fn validate_broker_arguments(argument_count: usize) -> Result<(), &'static str> {
    if argument_count != 0 {
        return Err("DAIA Model Export broker accepts no arguments");
    }

    Ok(())
}

/// Resource binding is a separate prerequisite from operation authorization.
/// No production resource grant exists until trusted source and destination
/// handles are independently established.
#[derive(Clone, Debug, Eq, PartialEq)]
enum ExportResourceDecision {
    Unverified,
    #[cfg(test)]
    VerifiedForOperation(Vec<u8>),
}

fn validate_export_resources(
    decision: &ExportResourceDecision,
    operation: &ModelExportOperation,
) -> Result<(), &'static str> {
    match decision {
        #[cfg(test)]
        ExportResourceDecision::VerifiedForOperation(bytes)
            if *bytes == operation.canonical_bytes() =>
        {
            Ok(())
        }
        _ => Err("DAIA Model Export resources are not verified"),
    }
}

const MAX_EXPORT_REQUEST_BYTES: u64 = 1024 * 1024;

fn read_export_request<R: Read>(reader: R) -> Result<ParsedExportRequest, &'static str> {
    let mut input = Vec::new();
    reader
        .take(MAX_EXPORT_REQUEST_BYTES + 1)
        .read_to_end(&mut input)
        .map_err(|_| "failed to read export request")?;

    if input.len() as u64 > MAX_EXPORT_REQUEST_BYTES {
        return Err("export request exceeds size limit");
    }

    parse_export_request(&input)
}

fn main() {
    if let Err(error) = validate_broker_arguments(std::env::args_os().skip(1).count()) {
        eprintln!("DAIA Model Export broker: {error}");
        std::process::exit(1);
    }

    // Effective UID is a prerequisite, not proof of Polkit authorization.
    if validate_broker_effective_uid(rustix::process::geteuid().as_raw()).is_err() {
        eprintln!("DAIA Model Export broker requires privileged activation");
        std::process::exit(1);
    }

    let invoking_uid = match std::env::var("PKEXEC_UID") {
        Ok(value) => match validate_pkexec_invoking_uid(Some(&value)) {
            Ok(uid) => uid,
            Err(error) => {
                eprintln!("DAIA Model Export broker: {error}");
                std::process::exit(1);
            }
        },
        Err(_) => {
            eprintln!("DAIA Model Export broker: missing pkexec invoking UID");
            std::process::exit(1);
        }
    };

    let _invoking_uid = invoking_uid;

    let operation = match read_export_request(io::stdin().lock()) {
        Ok(operation) => operation,
        Err(error) => {
            eprintln!("DAIA Model Export broker: {error}");
            std::process::exit(1);
        }
    };

    // Parsing does not authorize an operation. Until a trusted authorization
    // provider exists, every valid request must still be denied.
    if let Err(error) = authorize_export(
        &AuthorizationDecision::Denied,
        &operation.operation,
        operation.selection.as_ref(),
    ) {
        eprintln!("DAIA Model Export broker: {error}");
        std::process::exit(1);
    }

    // Resource verification remains a separate fail-closed prerequisite.
    if let Err(error) =
        validate_export_resources(&ExportResourceDecision::Unverified, &operation.operation)
    {
        eprintln!("DAIA Model Export broker: {error}");
        std::process::exit(1);
    }

    // Defense in depth: no export execution path is connected yet.
    eprintln!("DAIA Model Export execution unavailable");
    std::process::exit(1);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn broker_accepts_no_command_line_arguments() {
        assert!(validate_broker_arguments(0).is_ok());

        for count in [1, 2, 3, 100] {
            assert!(
                validate_broker_arguments(count).is_err(),
                "unexpectedly accepted {count} arguments"
            );
        }
    }

    #[test]
    fn unverified_resources_never_authorize_export() {
        let requested = operation("export");
        assert!(
            validate_export_resources(&ExportResourceDecision::Unverified, &requested).is_err()
        );
    }

    #[test]
    fn resource_binding_rejects_different_operation() {
        let requested = operation("export");
        let decision = ExportResourceDecision::VerifiedForOperation(requested.canonical_bytes());

        assert!(validate_export_resources(&decision, &requested).is_ok());
        assert!(validate_export_resources(&decision, &operation("other-export")).is_err());
    }

    fn request() -> String {
        format!(
            r#"{{"version":1,"operation":{{"id":"operation-1","source":"ollama","destination_name":"export","artifacts":[{{"relative_path":"model.gguf","sha256":"{}","size_bytes":12}}]}}}}"#,
            "a".repeat(64)
        )
    }

    #[test]
    fn parser_accepts_version_two_ollama_selection_without_authorizing() {
        let input = request().replacen(
            r#""version":1"#,
            r#""version":2,"selection":{"source":{"kind":"ollama","directory":"/var/lib/ollama/models","relative_manifest":"registry.ollama.ai/library/qwen/latest"},"destination":{"directory":"/media/export"}}"#,
            1,
        );

        let parsed = parse_export_request(input.as_bytes()).unwrap();
        let selection = parsed
            .selection
            .as_ref()
            .expect("version 2 requires selection");

        assert!(matches!(
            selection.source(),
            ModelExportSourceSelection::Ollama {
                directory,
                relative_manifest
            } if directory == "/var/lib/ollama/models"
                && relative_manifest == "registry.ollama.ai/library/qwen/latest"
        ));
        assert_eq!(selection.destination().directory(), "/media/export");
        assert!(
            authorize_export(
                &AuthorizationDecision::Denied,
                &parsed.operation,
                parsed.selection.as_ref()
            )
            .is_err()
        );
        assert!(
            validate_export_resources(&ExportResourceDecision::Unverified, &parsed.operation)
                .is_err()
        );
    }

    #[test]
    fn parser_accepts_version_two_gguf_selection_without_authorizing() {
        let input = request()
            .replacen(r#""source":"ollama""#, r#""source":"gguf""#, 1)
            .replacen(
                r#""version":1"#,
                r#""version":2,"selection":{"source":{"kind":"gguf","directory":"/media/models","relative_file":"files/example.gguf"},"destination":{"directory":"/media/export"}}"#,
                1,
            );

        let parsed = parse_export_request(input.as_bytes()).unwrap();
        let selection = parsed
            .selection
            .as_ref()
            .expect("version 2 requires selection");

        assert!(matches!(
            selection.source(),
            ModelExportSourceSelection::Gguf {
                directory,
                relative_file
            } if directory == "/media/models"
                && relative_file == "files/example.gguf"
        ));
        assert_eq!(selection.destination().directory(), "/media/export");
        assert!(
            authorize_export(
                &AuthorizationDecision::Denied,
                &parsed.operation,
                parsed.selection.as_ref()
            )
            .is_err()
        );
        assert!(
            validate_export_resources(&ExportResourceDecision::Unverified, &parsed.operation)
                .is_err()
        );
    }

    #[test]
    fn parser_rejects_invalid_version_two_selections() {
        let valid = request().replacen(
            r#""version":1"#,
            r#""version":2,"selection":{"source":{"kind":"ollama","directory":"/var/lib/ollama/models","relative_manifest":"registry.ollama.ai/library/qwen/latest"},"destination":{"directory":"/media/export"}}"#,
            1,
        );

        assert!(parse_export_request(valid.as_bytes()).is_ok());

        let cases = [
            valid.replace(r#""kind":"ollama""#, r#""kind":"gguf""#),
            valid.replace("/var/lib/ollama/models", "/var/lib/../ollama/models"),
            valid.replace("registry.ollama.ai/library/qwen/latest", "../qwen/latest"),
            valid.replace("/media/export", "/media/../export"),
            valid.replace(
                r#""destination":{"directory":"/media/export"}"#,
                r#""destination":{}"#,
            ),
            valid.replace(r#""kind":"ollama""#, r#""kind":"ollama","kind":"ollama""#),
            valid.replace(
                r#""directory":"/media/export""#,
                r#""directory":"/media/export","unexpected":true"#,
            ),
            valid.replace(r#""kind":"ollama""#, r#""kind":"unknown""#),
        ];

        for input in cases {
            assert_ne!(input, valid, "negative fixture must differ");
            assert!(
                parse_export_request(input.as_bytes()).is_err(),
                "accepted invalid selection: {input}"
            );
        }
    }

    #[test]
    fn request_intake_accepts_valid_metadata_without_authorizing() {
        let parsed = read_export_request(request().as_bytes()).unwrap();
        assert_eq!(parsed.operation.destination_name(), "export");
        assert!(
            authorize_export(
                &AuthorizationDecision::Denied,
                &parsed.operation,
                parsed.selection.as_ref()
            )
            .is_err()
        );
    }

    #[test]
    fn request_intake_rejects_empty_input() {
        assert!(read_export_request(&b""[..]).is_err());
    }

    #[test]
    fn request_intake_rejects_oversized_input() {
        let input = vec![b' '; MAX_EXPORT_REQUEST_BYTES as usize + 1];
        assert!(read_export_request(input.as_slice()).is_err());
    }

    #[test]
    fn request_intake_rejects_malformed_json() {
        assert!(read_export_request(&b"{invalid"[..]).is_err());
    }

    #[test]
    fn parser_accepts_valid_metadata_without_authorizing() {
        let parsed = parse_export_request(request().as_bytes()).unwrap();
        assert_eq!(parsed.operation.destination_name(), "export");
        assert_eq!(parsed.operation.artifacts().len(), 1);
        assert!(
            authorize_export(
                &AuthorizationDecision::Denied,
                &parsed.operation,
                parsed.selection.as_ref()
            )
            .is_err()
        );
    }

    #[test]
    fn parser_rejects_nested_duplicate_keys() {
        let valid = request();
        let cases = [
            valid.replace(
                r#""relative_path":"model.gguf""#,
                r#""relative_path":"model.gguf","relative_path":"other.gguf""#,
            ),
            valid.replace(
                r#""sha256":"#,
                r#""unexpected":{"nested":1,"nested":2},"sha256":"#,
            ),
        ];

        for input in cases {
            assert!(parse_export_request(input.as_bytes()).is_err(), "{input}");
        }
    }

    #[test]
    fn parser_rejects_invalid_numeric_boundaries() {
        let valid = request();
        for replacement in ["18446744073709551616", "1.5", "null", "true", r#""12""#] {
            let input = valid.replace(
                r#""size_bytes":12"#,
                &format!(r#""size_bytes":{replacement}"#),
            );
            assert!(parse_export_request(input.as_bytes()).is_err(), "{input}");
        }
    }

    #[test]
    fn parser_rejects_conflicting_artifact_paths() {
        let valid = request();
        let original = format!(
            r#"{{"relative_path":"model.gguf","sha256":"{}","size_bytes":12}}"#,
            "a".repeat(64),
        );
        let duplicate = format!("{original},{original}");
        let input = valid.replace(&original, &duplicate);

        assert_ne!(input, valid, "test fixture must change");
        assert!(parse_export_request(input.as_bytes()).is_err());
    }

    #[test]
    fn parser_rejects_ambiguous_or_invalid_requests() {
        let valid = request();
        let cases = [
            valid.replace(r#""version":1"#, r#""version":2"#),
            valid.replace(r#""version":1"#, r#""version":1,"version":1"#),
            valid.replace(r#""id":"operation-1""#, r#""id":"x","id":"y""#),
            valid.replace(r#""size_bytes":12"#, r#""size_bytes":12,"size_bytes":13"#),
            valid.replace(r#""source":"ollama""#, r#""source":"invalid""#),
            valid.replace(
                r#""destination_name":"export""#,
                r#""destination_name":"../escape""#,
            ),
            valid.replace(
                r#""relative_path":"model.gguf""#,
                r#""relative_path":"../model.gguf""#,
            ),
            valid.replace(r#""size_bytes":12"#, r#""size_bytes":-1"#),
            valid.replace(r#""version":1"#, r#""version":1,"authorized":true"#),
            format!("{valid} true"),
        ];

        for input in cases {
            assert!(parse_export_request(input.as_bytes()).is_err(), "{input}");
        }

        assert!(parse_export_request(&[]).is_err());
        assert!(parse_export_request(&vec![b' '; 1024 * 1024 + 1]).is_err());
    }
    use model::{ModelExportArtifact, ModelExportOperationId, ModelExportSource};

    fn operation(name: &str) -> ModelExportOperation {
        ModelExportOperation::new(
            ModelExportOperationId::new("operation-1").unwrap(),
            ModelExportSource::Ollama,
            name,
            vec![ModelExportArtifact::new("model.gguf", "a".repeat(64), 12).unwrap()],
        )
        .unwrap()
    }

    #[test]
    fn complete_request_binding_changes_with_resources_and_operation() {
        let requested = operation("export");

        let selection = ModelExportResourceSelection::new(
            ModelExportSourceSelection::ollama(
                "/media/models",
                "registry.ollama.ai/library/qwen/latest",
            )
            .unwrap(),
            ModelExportDestinationSelection::new("/media/export").unwrap(),
        );

        let different_source = ModelExportResourceSelection::new(
            ModelExportSourceSelection::ollama(
                "/media/models",
                "registry.ollama.ai/library/qwen/other",
            )
            .unwrap(),
            ModelExportDestinationSelection::new("/media/export").unwrap(),
        );

        let different_destination = ModelExportResourceSelection::new(
            selection.source().clone(),
            ModelExportDestinationSelection::new("/media/other").unwrap(),
        );

        let expected = canonical_export_request_bytes(&requested, &selection);

        assert_eq!(
            expected,
            canonical_export_request_bytes(&requested, &selection)
        );
        assert_ne!(
            expected,
            canonical_export_request_bytes(&requested, &different_source)
        );
        assert_ne!(
            expected,
            canonical_export_request_bytes(&requested, &different_destination)
        );
        assert_ne!(
            expected,
            canonical_export_request_bytes(&operation("other-export"), &selection)
        );
    }

    fn authorization_selection() -> ModelExportResourceSelection {
        ModelExportResourceSelection::new(
            ModelExportSourceSelection::ollama(
                "/media/models",
                "registry.ollama.ai/library/qwen/latest",
            )
            .unwrap(),
            ModelExportDestinationSelection::new("/media/export").unwrap(),
        )
    }

    #[test]
    fn broker_denies_without_verified_authorization() {
        assert!(
            authorize_export(&AuthorizationDecision::Denied, &operation("export"), None).is_err()
        );
    }

    #[test]
    fn broker_accepts_only_matching_operation() {
        let requested = operation("export");
        let decision = AuthorizationDecision::VerifiedForOperation(VerifiedExportGrant::for_test(
            canonical_export_request_bytes(&requested, &authorization_selection()),
        ));

        assert!(authorize_export(&decision, &requested, Some(&authorization_selection())).is_ok());
        assert!(
            authorize_export(
                &decision,
                &operation("other-export"),
                Some(&authorization_selection())
            )
            .is_err()
        );
    }

    #[test]
    fn authorization_rejects_resource_substitution_and_missing_selection() {
        let requested = operation("export");
        let selected = authorization_selection();

        let decision = AuthorizationDecision::VerifiedForOperation(VerifiedExportGrant::for_test(
            canonical_export_request_bytes(&requested, &selected),
        ));

        let changed_source = ModelExportResourceSelection::new(
            ModelExportSourceSelection::ollama(
                "/media/models",
                "registry.ollama.ai/library/qwen/other",
            )
            .unwrap(),
            selected.destination().clone(),
        );

        let changed_destination = ModelExportResourceSelection::new(
            selected.source().clone(),
            ModelExportDestinationSelection::new("/media/other").unwrap(),
        );

        assert!(authorize_export(&decision, &requested, Some(&selected)).is_ok());
        assert!(authorize_export(&decision, &requested, Some(&changed_source)).is_err());
        assert!(authorize_export(&decision, &requested, Some(&changed_destination)).is_err());
        assert!(authorize_export(&decision, &requested, None).is_err());
    }

    #[test]
    fn broker_rejects_untrusted_claims() {
        let requested = operation("export");
        for claim in [
            "authorized",
            "verified",
            "root",
            "pkexec",
            "org.daia.model-export",
            "--authorized",
            "--verified",
        ] {
            let decision = AuthorizationDecision::VerifiedForOperation(
                VerifiedExportGrant::for_test(claim.as_bytes().to_vec()),
            );
            assert!(
                authorize_export(&decision, &requested, Some(&authorization_selection())).is_err(),
                "untrusted claim must not authorize: {claim}"
            );
        }
    }

    #[test]
    fn authorization_rejects_different_operation_identity() {
        let requested = operation("export");
        let different = ModelExportOperation::new(
            ModelExportOperationId::new("operation-2").unwrap(),
            ModelExportSource::Ollama,
            "export",
            vec![ModelExportArtifact::new("model.gguf", "a".repeat(64), 12).unwrap()],
        )
        .unwrap();

        let decision = AuthorizationDecision::VerifiedForOperation(VerifiedExportGrant::for_test(
            canonical_export_request_bytes(&requested, &authorization_selection()),
        ));

        assert!(authorize_export(&decision, &requested, Some(&authorization_selection())).is_ok());
        assert!(authorize_export(&decision, &different, Some(&authorization_selection())).is_err());
    }

    #[test]
    fn authorization_rejects_untrusted_request_metadata() {
        let requested = operation("export");

        for claim in [
            br#"{"authorized":true}"#.as_slice(),
            br#"{"polkit":"approved"}"#.as_slice(),
            br#"{"uid":0}"#.as_slice(),
        ] {
            let decision = AuthorizationDecision::VerifiedForOperation(
                VerifiedExportGrant::for_test(claim.to_vec()),
            );

            assert!(
                authorize_export(&decision, &requested, Some(&authorization_selection())).is_err()
            );
        }
    }

    #[test]
    fn parsed_requests_never_create_authorization_grants() {
        let valid = request();
        let parsed = parse_export_request(valid.as_bytes()).unwrap();

        // Parsing validates metadata, but it cannot authorize an export.
        let decision = AuthorizationDecision::Denied;
        assert!(authorize_export(&decision, &parsed.operation, parsed.selection.as_ref()).is_err());

        // An untrusted authorization claim must not change that result.
        let claimed = valid.replace(r#""version":1"#, r#""version":1,"authorized":true"#);
        assert!(parse_export_request(claimed.as_bytes()).is_err());
    }

    #[test]
    fn broker_rejects_claimed_privileged_identity() {
        let valid = request();

        for claim in [
            r#""uid":0"#,
            r#""pid":1"#,
            r#""broker":"trusted""#,
            r#""polkit":"approved""#,
        ] {
            let input = valid.replace(r#""version":1"#, &format!(r#""version":1,{claim}"#));
            assert!(parse_export_request(input.as_bytes()).is_err());
        }
    }

    #[test]
    fn pkexec_invoking_uid_accepts_canonical_decimal_values() {
        assert_eq!(validate_pkexec_invoking_uid(Some("1000")), Ok(1000));
        assert_eq!(validate_pkexec_invoking_uid(Some("0")), Ok(0));
        assert_eq!(
            validate_pkexec_invoking_uid(Some("4294967295")),
            Ok(u32::MAX)
        );
    }

    #[test]
    fn pkexec_invoking_uid_rejects_missing_and_malformed_values() {
        for value in [
            None,
            Some(""),
            Some("abc"),
            Some("-1"),
            Some("+1000"),
            Some(" 1000"),
            Some("1000 "),
            Some("4294967296"),
            Some("01000"),
        ] {
            assert!(
                validate_pkexec_invoking_uid(value).is_err(),
                "unexpectedly accepted {value:?}"
            );
        }
    }

    #[test]
    fn broker_activation_rejects_unprivileged_identity() {
        for uid in [1, 1000, 65534, u32::MAX] {
            assert!(validate_broker_effective_uid(uid).is_err());
        }
    }

    #[test]
    fn privileged_identity_does_not_grant_export_authorization() {
        assert!(validate_broker_effective_uid(0).is_ok());

        let requested = operation("export");
        assert!(authorize_export(&AuthorizationDecision::Denied, &requested, None).is_err());
    }

    #[test]
    fn broker_denies_without_an_authorization_provider() {
        assert!(
            authorize_export(&AuthorizationDecision::Denied, &operation("export"), None).is_err()
        );
    }

    #[test]
    fn privileged_pkexec_identity_cannot_supply_an_operation_grant() {
        let requested = operation("export");

        // Privileged execution and invoking-user attribution are prerequisites,
        // not independent evidence of authorization for this operation.
        assert!(validate_broker_effective_uid(0).is_ok());
        assert_eq!(validate_pkexec_invoking_uid(Some("1000")), Ok(1000));

        assert!(
            authorize_export(&AuthorizationDecision::Denied, &requested, None).is_err(),
            "privileged identity must not authorize an export"
        );
        assert!(
            validate_export_resources(&ExportResourceDecision::Unverified, &requested).is_err(),
            "privileged identity must not verify export resources"
        );
    }
}
