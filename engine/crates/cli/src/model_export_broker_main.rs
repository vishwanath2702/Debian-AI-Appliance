//! DAIA Model Export authorization broker.
//!
//! No caller identity, socket permission, or environment variable grants
//! authorization. An independently verified authorization mechanism must
//! be implemented before export operations are enabled.

use model::ModelExportOperation;

use model::{ModelExportArtifact, ModelExportOperationId, ModelExportSource};
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

/// Parse metadata only. No authorization or file access occurs here.
fn parse_export_request(input: &[u8]) -> Result<ModelExportOperation, &'static str> {
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

    let envelope = exact_object(&root.0, &["version", "operation"])?;
    if envelope.get("version") != Some(&Value::from(1)) {
        return Err("unsupported export request version");
    }

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

    ModelExportOperation::new(id, source, destination, artifacts)
        .map_err(|_| "invalid export operation")
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
) -> Result<(), &'static str> {
    match decision {
        AuthorizationDecision::VerifiedForOperation(grant)
            if grant.operation_bytes == operation.canonical_bytes() =>
        {
            Ok(())
        }
        _ => Err("DAIA Model Export authorization unavailable"),
    }
}

const MAX_EXPORT_REQUEST_BYTES: u64 = 1024 * 1024;

fn read_export_request<R: Read>(reader: R) -> Result<ModelExportOperation, &'static str> {
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
    if let Err(error) = authorize_export(&AuthorizationDecision::Denied, &operation) {
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

    fn request() -> String {
        format!(
            r#"{{"version":1,"operation":{{"id":"operation-1","source":"ollama","destination_name":"export","artifacts":[{{"relative_path":"model.gguf","sha256":"{}","size_bytes":12}}]}}}}"#,
            "a".repeat(64)
        )
    }

    #[test]
    fn request_intake_accepts_valid_metadata_without_authorizing() {
        let parsed = read_export_request(request().as_bytes()).unwrap();
        assert_eq!(parsed.destination_name(), "export");
        assert!(authorize_export(&AuthorizationDecision::Denied, &parsed).is_err());
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
        assert_eq!(parsed.destination_name(), "export");
        assert_eq!(parsed.artifacts().len(), 1);
        assert!(authorize_export(&AuthorizationDecision::Denied, &parsed).is_err());
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
    fn broker_denies_without_verified_authorization() {
        assert!(authorize_export(&AuthorizationDecision::Denied, &operation("export")).is_err());
    }

    #[test]
    fn broker_accepts_only_matching_operation() {
        let requested = operation("export");
        let decision = AuthorizationDecision::VerifiedForOperation(VerifiedExportGrant::for_test(
            requested.canonical_bytes(),
        ));

        assert!(authorize_export(&decision, &requested).is_ok());
        assert!(authorize_export(&decision, &operation("other-export")).is_err());
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
                authorize_export(&decision, &requested).is_err(),
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
            requested.canonical_bytes(),
        ));

        assert!(authorize_export(&decision, &requested).is_ok());
        assert!(authorize_export(&decision, &different).is_err());
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

            assert!(authorize_export(&decision, &requested).is_err());
        }
    }

    #[test]
    fn parsed_requests_never_create_authorization_grants() {
        let valid = request();
        let parsed = parse_export_request(valid.as_bytes()).unwrap();

        // Parsing validates metadata, but it cannot authorize an export.
        let decision = AuthorizationDecision::Denied;
        assert!(authorize_export(&decision, &parsed).is_err());

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
        assert!(authorize_export(&AuthorizationDecision::Denied, &requested).is_err());
    }

    #[test]
    fn broker_denies_without_an_authorization_provider() {
        assert!(authorize_export(&AuthorizationDecision::Denied, &operation("export")).is_err());
    }
}
