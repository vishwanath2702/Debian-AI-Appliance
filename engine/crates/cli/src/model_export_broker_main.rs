//! DAIA Model Export authorization broker.
//!
//! No caller identity, socket permission, or environment variable grants
//! authorization. An independently verified authorization mechanism must
//! be implemented before export operations are enabled.

use model::{ModelExportOperation, canonical_export_request_bytes};

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

    if let Some(selection) = selection.as_ref() {
        validate_gguf_artifact_binding(&operation, selection)?;
    }

    Ok(ParsedExportRequest {
        operation,
        selection,
    })
}

/// Validate GGUF request consistency without authorizing source access.
fn validate_gguf_artifact_binding(
    operation: &ModelExportOperation,
    selection: &ModelExportResourceSelection,
) -> Result<(), &'static str> {
    if let ModelExportSourceSelection::Gguf { relative_file, .. } = selection.source() {
        if !matches!(operation.source(), ModelExportSource::Gguf)
            || operation.artifacts().len() != 1
            || operation.artifacts()[0].relative_path() != relative_file
        {
            return Err("GGUF artifact does not match selected source");
        }
    }

    Ok(())
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
    invoking_uid: u32,
    operation_bytes: Vec<u8>,
}

#[cfg(test)]
impl VerifiedExportGrant {
    fn for_test(invoking_uid: u32, operation_bytes: Vec<u8>) -> Self {
        Self {
            invoking_uid,
            operation_bytes,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum AuthorizationDecision {
    Denied,
    VerifiedForOperation(VerifiedExportGrant),
}

fn authorize_export(
    decision: &AuthorizationDecision,
    invoking_uid: u32,
    operation: &ModelExportOperation,
    selection: Option<&ModelExportResourceSelection>,
) -> Result<(), &'static str> {
    let selection = selection.ok_or("DAIA Model Export resource selection required")?;

    match decision {
        AuthorizationDecision::VerifiedForOperation(grant)
            if invoking_uid != 0
                && grant.invoking_uid == invoking_uid
                && grant.operation_bytes
                    == canonical_export_request_bytes(operation, selection) =>
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
    selection: Option<&ModelExportResourceSelection>,
) -> Result<(), &'static str> {
    let selection = selection.ok_or("DAIA Model Export resource selection required")?;

    match decision {
        #[cfg(test)]
        ExportResourceDecision::VerifiedForOperation(bytes)
            if *bytes == canonical_export_request_bytes(operation, selection) =>
        {
            Ok(())
        }
        _ => Err("DAIA Model Export resources are not verified"),
    }
}

/// Validate a retained source-directory descriptor against the selected request.
///
/// This establishes directory identity only. It does not authorize access,
/// validate GGUF contents, validate Ollama manifests, or grant export execution.
#[cfg(target_os = "linux")]
fn validate_bound_export_source(
    selection: &ModelExportResourceSelection,
    source: std::os::fd::BorrowedFd<'_>,
) -> Result<std::os::fd::OwnedFd, &'static str> {
    let selected_directory = match selection.source() {
        ModelExportSourceSelection::Gguf { directory, .. }
        | ModelExportSourceSelection::Ollama { directory, .. } => directory,
    };

    let opened = engine::export_ipc::open_selected_export_directory(std::path::Path::new(
        selected_directory,
    ))
    .map_err(|_| "unable to resolve selected export source")?;

    let selected_metadata =
        rustix::fs::fstat(&opened).map_err(|_| "unable to inspect selected export source")?;

    let retained_metadata =
        rustix::fs::fstat(source).map_err(|_| "unable to inspect retained export source")?;

    if selected_metadata.st_dev != retained_metadata.st_dev
        || selected_metadata.st_ino != retained_metadata.st_ino
    {
        return Err("export source directory identity mismatch");
    }

    use rustix::fs::{FileType, Mode, OFlags, ResolveFlags};

    let relative_file = match selection.source() {
        ModelExportSourceSelection::Gguf { relative_file, .. } => {
            std::path::Path::new(relative_file)
        }
        ModelExportSourceSelection::Ollama {
            relative_manifest, ..
        } => {
            let manifest = std::path::Path::new(relative_manifest);

            if manifest.components().count() != 4 {
                return Err("invalid Ollama export manifest path");
            }

            manifest
        }
    };

    let relative_path = match selection.source() {
        ModelExportSourceSelection::Gguf { .. } => relative_file.to_path_buf(),
        ModelExportSourceSelection::Ollama { .. } => {
            std::path::Path::new("manifests").join(relative_file)
        }
    };

    let file = rustix::fs::openat2(
        source,
        &relative_path,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
        Mode::empty(),
        ResolveFlags::BENEATH | ResolveFlags::NO_SYMLINKS,
    )
    .map_err(|_| "unable to open selected export source file")?;

    let metadata =
        rustix::fs::fstat(&file).map_err(|_| "unable to inspect selected export source file")?;

    if FileType::from_raw_mode(metadata.st_mode) != FileType::RegularFile {
        return Err("selected export source is not a regular file");
    }

    Ok(file)
}

/// Validate a retained destination handle against the selected request.
///
/// The invoking UID must come from an independently authenticated identity.
/// This validates resource identity and basic ownership only; it does not
/// establish Polkit authorization, user write access, or same-UID isolation.
#[cfg(target_os = "linux")]
fn validate_bound_export_destination(
    selection: &ModelExportResourceSelection,
    destination: std::os::fd::BorrowedFd<'_>,
    invoking_uid: u32,
) -> Result<(), &'static str> {
    let selected_path = std::path::Path::new(selection.destination().directory());

    let opened = engine::export_ipc::open_selected_export_directory(selected_path)
        .map_err(|_| "unable to resolve selected export destination")?;

    let selected_metadata =
        rustix::fs::fstat(&opened).map_err(|_| "unable to inspect selected export destination")?;

    let retained_metadata = rustix::fs::fstat(destination)
        .map_err(|_| "unable to inspect retained export destination")?;

    if selected_metadata.st_dev != retained_metadata.st_dev
        || selected_metadata.st_ino != retained_metadata.st_ino
    {
        return Err("export destination identity mismatch");
    }

    engine::export_ipc::validate_selected_export_destination(destination, invoking_uid)
        .map_err(|_| "export destination ownership validation failed")
}

#[cfg(all(test, target_os = "linux"))]
mod bound_export_source_tests {
    use super::*;
    use std::os::fd::AsFd;

    fn selection(source: ModelExportSourceSelection) -> ModelExportResourceSelection {
        ModelExportResourceSelection::new(
            source,
            ModelExportDestinationSelection::new("/tmp/export").unwrap(),
        )
    }

    #[test]
    fn rejects_missing_gguf_file() {
        let root = tempfile::tempdir().unwrap();
        let retained = engine::export_ipc::open_selected_export_directory(root.path()).unwrap();
        let request = selection(
            ModelExportSourceSelection::gguf(root.path().to_str().unwrap(), "missing.gguf")
                .unwrap(),
        );

        assert!(validate_bound_export_source(&request, retained.as_fd()).is_err());
    }

    #[test]
    fn rejects_gguf_symlink() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("real.gguf"), b"GGUF").unwrap();
        symlink("real.gguf", root.path().join("linked.gguf")).unwrap();

        let retained = engine::export_ipc::open_selected_export_directory(root.path()).unwrap();
        let request = selection(
            ModelExportSourceSelection::gguf(root.path().to_str().unwrap(), "linked.gguf").unwrap(),
        );

        assert!(validate_bound_export_source(&request, retained.as_fd()).is_err());
    }

    #[test]
    fn rejects_gguf_directory_as_file() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("fake.gguf")).unwrap();

        let retained = engine::export_ipc::open_selected_export_directory(root.path()).unwrap();
        let request = selection(
            ModelExportSourceSelection::gguf(root.path().to_str().unwrap(), "fake.gguf").unwrap(),
        );

        assert!(validate_bound_export_source(&request, retained.as_fd()).is_err());
    }

    #[test]
    fn rejects_ollama_manifest_symlink() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().unwrap();
        let manifests = root
            .path()
            .join("manifests/registry.ollama.ai/library/model");
        std::fs::create_dir_all(&manifests).unwrap();
        std::fs::write(manifests.join("real"), b"{}").unwrap();
        symlink("real", manifests.join("latest")).unwrap();

        let retained = engine::export_ipc::open_selected_export_directory(root.path()).unwrap();
        let request = selection(
            ModelExportSourceSelection::ollama(
                root.path().to_str().unwrap(),
                "registry.ollama.ai/library/model/latest",
            )
            .unwrap(),
        );

        assert!(validate_bound_export_source(&request, retained.as_fd()).is_err());
    }

    #[test]
    fn rejects_ollama_manifest_directory() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(
            root.path()
                .join("manifests/registry.ollama.ai/library/model/latest"),
        )
        .unwrap();

        let retained = engine::export_ipc::open_selected_export_directory(root.path()).unwrap();
        let request = selection(
            ModelExportSourceSelection::ollama(
                root.path().to_str().unwrap(),
                "registry.ollama.ai/library/model/latest",
            )
            .unwrap(),
        );

        assert!(validate_bound_export_source(&request, retained.as_fd()).is_err());
    }

    #[test]
    fn rejects_replaced_source_directory_path() {
        let root = tempfile::tempdir().unwrap();
        let selected = root.path().join("selected");
        let moved = root.path().join("moved");

        std::fs::create_dir(&selected).unwrap();
        std::fs::write(selected.join("model.gguf"), b"GGUF").unwrap();

        let retained = engine::export_ipc::open_selected_export_directory(&selected).unwrap();

        std::fs::rename(&selected, &moved).unwrap();
        std::fs::create_dir(&selected).unwrap();
        std::fs::write(selected.join("model.gguf"), b"GGUF").unwrap();

        let request = selection(
            ModelExportSourceSelection::gguf(selected.to_str().unwrap(), "model.gguf").unwrap(),
        );

        assert_eq!(
            validate_bound_export_source(&request, retained.as_fd()).unwrap_err(),
            "export source directory identity mismatch"
        );
    }

    #[test]
    fn retained_gguf_descriptor_does_not_freeze_writable_contents() {
        use std::io::{Read, Seek, SeekFrom, Write};

        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("model.gguf");

        std::fs::write(&source, b"GGUF-original").unwrap();

        let retained_directory =
            engine::export_ipc::open_selected_export_directory(root.path()).unwrap();

        let request = selection(
            ModelExportSourceSelection::gguf(root.path().to_str().unwrap(), "model.gguf").unwrap(),
        );

        let retained_file =
            validate_bound_export_source(&request, retained_directory.as_fd()).unwrap();

        // Simulate another same-UID process retaining write access.
        let mut attacker = std::fs::OpenOptions::new()
            .write(true)
            .open(&source)
            .unwrap();

        attacker.seek(SeekFrom::Start(0)).unwrap();
        attacker.write_all(b"GGUF-modified").unwrap();
        attacker.sync_all().unwrap();

        let mut validated = std::fs::File::from(retained_file);
        validated.seek(SeekFrom::Start(0)).unwrap();

        let mut contents = Vec::new();
        validated.read_to_end(&mut contents).unwrap();

        assert_eq!(contents, b"GGUF-modified");
        assert_ne!(contents, b"GGUF-original");
    }

    #[test]
    fn retained_gguf_descriptor_survives_path_replacement() {
        use std::io::{Read, Seek, SeekFrom};

        let root = tempfile::tempdir().unwrap();
        let selected = root.path().join("model.gguf");
        let moved = root.path().join("original.gguf");

        std::fs::write(&selected, b"GGUF-original").unwrap();

        let retained_directory =
            engine::export_ipc::open_selected_export_directory(root.path()).unwrap();

        let request = selection(
            ModelExportSourceSelection::gguf(root.path().to_str().unwrap(), "model.gguf").unwrap(),
        );

        let retained_file =
            validate_bound_export_source(&request, retained_directory.as_fd()).unwrap();

        std::fs::rename(&selected, &moved).unwrap();
        std::fs::write(&selected, b"GGUF-replaced").unwrap();

        let mut file = std::fs::File::from(retained_file);
        file.seek(SeekFrom::Start(0)).unwrap();

        let mut contents = Vec::new();
        file.read_to_end(&mut contents).unwrap();

        assert_eq!(contents, b"GGUF-original");
        assert_eq!(std::fs::read(&selected).unwrap(), b"GGUF-replaced");
    }

    #[test]
    fn accepts_matching_gguf_source_directory() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("models");
        std::fs::create_dir(&source).unwrap();
        std::fs::write(source.join("model.gguf"), b"GGUF").unwrap();

        let retained = engine::export_ipc::open_selected_export_directory(&source).unwrap();
        let request = selection(
            ModelExportSourceSelection::gguf(source.to_str().unwrap(), "model.gguf").unwrap(),
        );

        assert!(validate_bound_export_source(&request, retained.as_fd()).is_ok());
    }

    #[test]
    fn accepts_matching_ollama_source_directory() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("ollama");
        std::fs::create_dir(&source).unwrap();
        let manifest = source
            .join("manifests")
            .join("registry.ollama.ai/library/model/latest");
        std::fs::create_dir_all(manifest.parent().unwrap()).unwrap();
        std::fs::write(&manifest, b"{}").unwrap();

        let retained = engine::export_ipc::open_selected_export_directory(&source).unwrap();
        let request = selection(
            ModelExportSourceSelection::ollama(
                source.to_str().unwrap(),
                "registry.ollama.ai/library/model/latest",
            )
            .unwrap(),
        );

        assert!(validate_bound_export_source(&request, retained.as_fd()).is_ok());
    }

    #[test]
    fn rejects_substituted_gguf_source_directory() {
        let root = tempfile::tempdir().unwrap();
        let selected = root.path().join("selected");
        let substituted = root.path().join("substituted");

        std::fs::create_dir(&selected).unwrap();
        std::fs::create_dir(&substituted).unwrap();

        let retained = engine::export_ipc::open_selected_export_directory(&substituted).unwrap();

        let request = selection(
            ModelExportSourceSelection::gguf(selected.to_str().unwrap(), "model.gguf").unwrap(),
        );

        assert_eq!(
            validate_bound_export_source(&request, retained.as_fd()).unwrap_err(),
            "export source directory identity mismatch"
        );
    }

    #[test]
    fn rejects_substituted_ollama_source_directory() {
        let root = tempfile::tempdir().unwrap();
        let selected = root.path().join("selected");
        let substituted = root.path().join("substituted");

        std::fs::create_dir(&selected).unwrap();
        std::fs::create_dir(&substituted).unwrap();

        let retained = engine::export_ipc::open_selected_export_directory(&substituted).unwrap();

        let request = selection(
            ModelExportSourceSelection::ollama(
                selected.to_str().unwrap(),
                "registry.ollama.ai/library/model/latest",
            )
            .unwrap(),
        );

        assert_eq!(
            validate_bound_export_source(&request, retained.as_fd()).unwrap_err(),
            "export source directory identity mismatch"
        );
    }
}

#[cfg(all(test, target_os = "linux"))]
mod bound_export_destination_tests {
    use super::*;
    use std::os::fd::AsFd;
    use std::os::unix::fs::PermissionsExt;

    fn selection(directory: &std::path::Path) -> ModelExportResourceSelection {
        ModelExportResourceSelection::new(
            ModelExportSourceSelection::gguf("/tmp", "example.gguf").unwrap(),
            ModelExportDestinationSelection::new(directory.to_str().unwrap()).unwrap(),
        )
    }

    #[test]
    fn accepts_matching_destination_identity() {
        let uid = rustix::process::geteuid().as_raw();
        if uid == 0 {
            return;
        }

        let temporary = tempfile::tempdir().unwrap();
        let destination = temporary.path().join("destination");
        std::fs::create_dir(&destination).unwrap();
        std::fs::set_permissions(&destination, std::fs::Permissions::from_mode(0o700)).unwrap();

        let opened = engine::export_ipc::open_selected_export_directory(&destination).unwrap();

        assert!(
            validate_bound_export_destination(&selection(&destination), opened.as_fd(), uid,)
                .is_ok()
        );
    }

    #[test]
    fn rejects_replaced_destination_identity() {
        let uid = rustix::process::geteuid().as_raw();
        if uid == 0 {
            return;
        }

        let temporary = tempfile::tempdir().unwrap();
        let destination = temporary.path().join("destination");
        let moved = temporary.path().join("moved");

        std::fs::create_dir(&destination).unwrap();

        let opened = engine::export_ipc::open_selected_export_directory(&destination).unwrap();

        std::fs::rename(&destination, &moved).unwrap();
        std::fs::create_dir(&destination).unwrap();

        assert!(
            validate_bound_export_destination(&selection(&destination), opened.as_fd(), uid,)
                .is_err()
        );
    }

    #[test]
    fn rejects_wrong_invoking_uid() {
        let uid = rustix::process::geteuid().as_raw();
        if uid == 0 || uid == u32::MAX {
            return;
        }

        let temporary = tempfile::tempdir().unwrap();
        let destination = temporary.path().join("destination");
        std::fs::create_dir(&destination).unwrap();

        let opened = engine::export_ipc::open_selected_export_directory(&destination).unwrap();

        assert!(
            validate_bound_export_destination(&selection(&destination), opened.as_fd(), uid + 1,)
                .is_err()
        );
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
        invoking_uid,
        &operation.operation,
        operation.selection.as_ref(),
    ) {
        eprintln!("DAIA Model Export broker: {error}");
        std::process::exit(1);
    }

    // Resource verification remains a separate fail-closed prerequisite.
    if let Err(error) = validate_export_resources(
        &ExportResourceDecision::Unverified,
        &operation.operation,
        operation.selection.as_ref(),
    ) {
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
    fn authorized_grant_rejects_substituted_invoker_uid() {
        let requested = operation("export");
        let selected = authorization_selection();

        let grant = VerifiedExportGrant::for_test(
            1001,
            canonical_export_request_bytes(&requested, &selected),
        );

        let decision = AuthorizationDecision::VerifiedForOperation(grant);

        assert!(authorize_export(&decision, 1001, &requested, Some(&selected)).is_ok());

        for substituted_uid in [0, 1002, u32::MAX] {
            assert!(
                authorize_export(&decision, substituted_uid, &requested, Some(&selected)).is_err()
            );
        }
    }

    #[test]
    fn authorized_grant_rejects_substituted_operation() {
        let requested = operation("export");
        let selected = authorization_selection();

        let grant = VerifiedExportGrant::for_test(
            1001,
            canonical_export_request_bytes(&requested, &selected),
        );

        let decision = AuthorizationDecision::VerifiedForOperation(grant);

        assert!(
            authorize_export(&decision, 1001, &operation("other-export"), Some(&selected)).is_err()
        );
    }

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
            validate_export_resources(
                &ExportResourceDecision::Unverified,
                &requested,
                Some(&authorization_selection()),
            )
            .is_err()
        );
    }

    #[test]
    fn resource_binding_rejects_different_operation_and_resources() {
        let requested = operation("export");
        let selected = authorization_selection();
        let decision = ExportResourceDecision::VerifiedForOperation(
            canonical_export_request_bytes(&requested, &selected),
        );

        assert!(validate_export_resources(&decision, &requested, Some(&selected)).is_ok());
        assert!(
            validate_export_resources(&decision, &operation("other-export"), Some(&selected),)
                .is_err()
        );

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

        assert!(validate_export_resources(&decision, &requested, Some(&changed_source),).is_err());

        assert!(
            validate_export_resources(&decision, &requested, Some(&changed_destination),).is_err()
        );

        assert!(validate_export_resources(&decision, &requested, None).is_err());
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
                1001,
                &parsed.operation,
                parsed.selection.as_ref()
            )
            .is_err()
        );
        assert!(
            validate_export_resources(
                &ExportResourceDecision::Unverified,
                &parsed.operation,
                parsed.selection.as_ref(),
            )
            .is_err()
        );
    }

    #[test]
    fn gguf_artifact_binding_accepts_matching_file() {
        let input = request()
            .replacen(r#""source":"ollama""#, r#""source":"gguf""#, 1)
            .replacen(
                r#""version":1"#,
                r#""version":2,"selection":{"source":{"kind":"gguf","directory":"/media/models","relative_file":"model.gguf"},"destination":{"directory":"/media/export"}}"#,
                1,
            );

        assert!(parse_export_request(input.as_bytes()).is_ok());
    }

    #[test]
    fn gguf_artifact_binding_rejects_mismatched_file() {
        let input = request()
            .replacen(r#""source":"ollama""#, r#""source":"gguf""#, 1)
            .replacen(
                r#""version":1"#,
                r#""version":2,"selection":{"source":{"kind":"gguf","directory":"/media/models","relative_file":"other.gguf"},"destination":{"directory":"/media/export"}}"#,
                1,
            );

        assert!(parse_export_request(input.as_bytes()).is_err());
    }

    #[test]
    fn gguf_artifact_binding_rejects_multiple_artifacts() {
        let input = request()
            .replacen(r#""source":"ollama""#, r#""source":"gguf""#, 1)
            .replacen(
                r#""version":1"#,
                r#""version":2,"selection":{"source":{"kind":"gguf","directory":"/media/models","relative_file":"model.gguf"},"destination":{"directory":"/media/export"}}"#,
                1,
            );

        let mut value: serde_json::Value = serde_json::from_str(&input).unwrap();

        let mut artifact = value["operation"]["artifacts"][0].clone();
        artifact["relative_path"] = serde_json::Value::String("extra.gguf".into());

        value["operation"]["artifacts"]
            .as_array_mut()
            .unwrap()
            .push(artifact);

        assert!(parse_export_request(value.to_string().as_bytes()).is_err());
    }

    #[test]
    fn parser_accepts_version_two_gguf_selection_without_authorizing() {
        let input = request()
            .replacen(r#""source":"ollama""#, r#""source":"gguf""#, 1)
            .replacen(
                r#""version":1"#,
                r#""version":2,"selection":{"source":{"kind":"gguf","directory":"/media/models","relative_file":"model.gguf"},"destination":{"directory":"/media/export"}}"#,
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
                && relative_file == "model.gguf"
        ));
        assert_eq!(selection.destination().directory(), "/media/export");
        assert!(
            authorize_export(
                &AuthorizationDecision::Denied,
                1001,
                &parsed.operation,
                parsed.selection.as_ref()
            )
            .is_err()
        );
        assert!(
            validate_export_resources(
                &ExportResourceDecision::Unverified,
                &parsed.operation,
                parsed.selection.as_ref(),
            )
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
                1001,
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
                1001,
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
            authorize_export(
                &AuthorizationDecision::Denied,
                1001,
                &operation("export"),
                None
            )
            .is_err()
        );
    }

    #[test]
    fn broker_accepts_only_matching_operation() {
        let requested = operation("export");
        let decision = AuthorizationDecision::VerifiedForOperation(VerifiedExportGrant::for_test(
            1001,
            canonical_export_request_bytes(&requested, &authorization_selection()),
        ));

        assert!(
            authorize_export(
                &decision,
                1001,
                &requested,
                Some(&authorization_selection())
            )
            .is_ok()
        );
        assert!(
            authorize_export(
                &decision,
                1001,
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
            1001,
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

        assert!(authorize_export(&decision, 1001, &requested, Some(&selected)).is_ok());
        assert!(authorize_export(&decision, 1001, &requested, Some(&changed_source)).is_err());
        assert!(authorize_export(&decision, 1001, &requested, Some(&changed_destination)).is_err());
        assert!(authorize_export(&decision, 1001, &requested, None).is_err());
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
                VerifiedExportGrant::for_test(1001, claim.as_bytes().to_vec()),
            );
            assert!(
                authorize_export(
                    &decision,
                    1001,
                    &requested,
                    Some(&authorization_selection())
                )
                .is_err(),
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
            1001,
            canonical_export_request_bytes(&requested, &authorization_selection()),
        ));

        assert!(
            authorize_export(
                &decision,
                1001,
                &requested,
                Some(&authorization_selection())
            )
            .is_ok()
        );
        assert!(
            authorize_export(
                &decision,
                1001,
                &different,
                Some(&authorization_selection())
            )
            .is_err()
        );
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
                VerifiedExportGrant::for_test(1001, claim.to_vec()),
            );

            assert!(
                authorize_export(
                    &decision,
                    1001,
                    &requested,
                    Some(&authorization_selection())
                )
                .is_err()
            );
        }
    }

    #[test]
    fn parsed_requests_never_create_authorization_grants() {
        let valid = request();
        let parsed = parse_export_request(valid.as_bytes()).unwrap();

        // Parsing validates metadata, but it cannot authorize an export.
        let decision = AuthorizationDecision::Denied;
        assert!(
            authorize_export(
                &decision,
                1001,
                &parsed.operation,
                parsed.selection.as_ref()
            )
            .is_err()
        );

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
        assert!(authorize_export(&AuthorizationDecision::Denied, 1001, &requested, None).is_err());
    }

    #[test]
    fn broker_denies_without_an_authorization_provider() {
        assert!(
            authorize_export(
                &AuthorizationDecision::Denied,
                1001,
                &operation("export"),
                None
            )
            .is_err()
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
            authorize_export(&AuthorizationDecision::Denied, 1001, &requested, None).is_err(),
            "privileged identity must not authorize an export"
        );
        assert!(
            validate_export_resources(&ExportResourceDecision::Unverified, &requested, None)
                .is_err(),
            "privileged identity must not verify export resources"
        );
    }
}
