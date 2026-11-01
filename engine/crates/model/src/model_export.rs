//! Validated descriptions of requested model exports.
//!
//! These types do not authorize exports, secure source files, or publish output.

use std::collections::HashSet;
use std::fmt;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ModelExportOperationId(String);

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ModelExportSource {
    Gguf,
    Ollama,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelExportArtifact {
    relative_path: String,
    sha256: String,
    size_bytes: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelExportOperation {
    id: ModelExportOperationId,
    source: ModelExportSource,
    destination_name: String,
    artifacts: Vec<ModelExportArtifact>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ModelExportValidationError {
    InvalidOperationId,
    InvalidDestinationName,
    InvalidArtifactPath,
    InvalidSha256,
    EmptyArtifacts,
    DuplicateArtifactPath,
    ConflictingArtifactPath,
    TooManyArtifacts,
    TotalSizeOverflow,
}

impl fmt::Display for ModelExportValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for ModelExportValidationError {}

fn valid_component(value: &str) -> bool {
    !value.is_empty()
        && value != "."
        && value != ".."
        && !value.starts_with('-')
        && !value
            .chars()
            .any(|c| c == '/' || c == '\\' || c.is_control())
}

fn valid_relative_path(value: &str) -> bool {
    !value.is_empty() && value.split('/').all(valid_component)
}

impl ModelExportOperationId {
    /// Creates a validated operation identifier.
    ///
    /// # Errors
    ///
    /// Returns `InvalidOperationId` for an invalid identifier.
    pub fn new(value: impl Into<String>) -> Result<Self, ModelExportValidationError> {
        let value = value.into();

        if value.len() > 128
            || !value
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
            || value.is_empty()
        {
            return Err(ModelExportValidationError::InvalidOperationId);
        }

        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl ModelExportArtifact {
    /// Creates a validated export artifact description.
    ///
    /// # Errors
    ///
    /// Returns `InvalidArtifactPath` or `InvalidSha256` when invalid.
    pub fn new(
        relative_path: impl Into<String>,
        sha256: impl Into<String>,
        size_bytes: u64,
    ) -> Result<Self, ModelExportValidationError> {
        let relative_path = relative_path.into();
        let sha256 = sha256.into();

        if relative_path.len() > 4096 || !valid_relative_path(&relative_path) {
            return Err(ModelExportValidationError::InvalidArtifactPath);
        }

        if sha256.len() != 64
            || !sha256.bytes().all(|b| b.is_ascii_hexdigit())
            || sha256.bytes().any(|b| b.is_ascii_uppercase())
        {
            return Err(ModelExportValidationError::InvalidSha256);
        }

        Ok(Self {
            relative_path,
            sha256,
            size_bytes,
        })
    }

    #[must_use]
    pub fn relative_path(&self) -> &str {
        &self.relative_path
    }

    #[must_use]
    pub fn sha256(&self) -> &str {
        &self.sha256
    }

    #[must_use]
    pub const fn size_bytes(&self) -> u64 {
        self.size_bytes
    }
}

// Bound metadata complexity, not the size of individual models.
const MAX_EXPORT_ARTIFACTS: usize = 100_000;

impl ModelExportOperation {
    /// Creates a validated export operation description.
    ///
    /// # Errors
    ///
    /// Returns a validation error for an invalid destination, artifact
    /// count, total declared size, or conflicting artifact paths.
    pub fn new(
        id: ModelExportOperationId,
        source: ModelExportSource,
        destination_name: impl Into<String>,
        artifacts: Vec<ModelExportArtifact>,
    ) -> Result<Self, ModelExportValidationError> {
        let destination_name = destination_name.into();

        if destination_name.len() > 255 || !valid_component(&destination_name) {
            return Err(ModelExportValidationError::InvalidDestinationName);
        }

        if artifacts.is_empty() {
            return Err(ModelExportValidationError::EmptyArtifacts);
        }

        if artifacts.len() > MAX_EXPORT_ARTIFACTS {
            return Err(ModelExportValidationError::TooManyArtifacts);
        }

        let mut total_size = 0_u64;
        for artifact in &artifacts {
            total_size = total_size
                .checked_add(artifact.size_bytes)
                .ok_or(ModelExportValidationError::TotalSizeOverflow)?;
        }

        let mut paths = HashSet::new();

        for artifact in &artifacts {
            if !paths.insert(artifact.relative_path.as_str()) {
                return Err(ModelExportValidationError::DuplicateArtifactPath);
            }
        }

        for path in &paths {
            for (index, byte) in path.bytes().enumerate() {
                if byte == b'/' && paths.contains(&path[..index]) {
                    return Err(ModelExportValidationError::ConflictingArtifactPath);
                }
            }
        }

        Ok(Self {
            id,
            source,
            destination_name,
            artifacts,
        })
    }

    /// Returns a versioned deterministic encoding of this operation.
    ///
    /// This encoding describes intent; it is not an authorization grant,
    /// a source snapshot, or a destination-device identity.
    #[must_use]
    pub fn canonical_bytes(&self) -> Vec<u8> {
        fn field(bytes: &mut Vec<u8>, tag: u8, value: &[u8]) {
            bytes.push(tag);
            bytes.extend_from_slice(&(value.len() as u64).to_be_bytes());
            bytes.extend_from_slice(value);
        }

        let mut bytes = b"DAIA-MODEL-EXPORT\0".to_vec();
        bytes.extend_from_slice(&1_u32.to_be_bytes());

        field(&mut bytes, 1, self.id.as_str().as_bytes());
        field(
            &mut bytes,
            2,
            match self.source {
                ModelExportSource::Gguf => b"gguf",
                ModelExportSource::Ollama => b"ollama",
            },
        );
        field(&mut bytes, 3, self.destination_name.as_bytes());

        let mut artifacts: Vec<_> = self.artifacts.iter().collect();
        artifacts.sort_unstable_by(|a, b| a.relative_path.cmp(&b.relative_path));

        bytes.extend_from_slice(&(artifacts.len() as u64).to_be_bytes());

        for artifact in artifacts {
            field(&mut bytes, 4, artifact.relative_path.as_bytes());
            field(&mut bytes, 5, artifact.sha256.as_bytes());
            field(&mut bytes, 6, &artifact.size_bytes.to_be_bytes());
        }

        bytes
    }

    #[must_use]
    pub const fn id(&self) -> &ModelExportOperationId {
        &self.id
    }

    #[must_use]
    pub const fn source(&self) -> &ModelExportSource {
        &self.source
    }

    #[must_use]
    pub fn destination_name(&self) -> &str {
        &self.destination_name
    }

    #[must_use]
    pub fn artifacts(&self) -> &[ModelExportArtifact] {
        &self.artifacts
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn artifact(path: &str) -> ModelExportArtifact {
        ModelExportArtifact::new(path, "a".repeat(64), 12).unwrap()
    }

    fn operation(
        artifacts: Vec<ModelExportArtifact>,
    ) -> Result<ModelExportOperation, ModelExportValidationError> {
        ModelExportOperation::new(
            ModelExportOperationId::new("operation-1").unwrap(),
            ModelExportSource::Ollama,
            "offline-model",
            artifacts,
        )
    }

    #[test]
    fn accepts_valid_gguf_and_ollama_descriptions() {
        let id = ModelExportOperationId::new("export_123").unwrap();

        let gguf = ModelExportOperation::new(
            id,
            ModelExportSource::Gguf,
            "model-export",
            vec![artifact("models/example.gguf")],
        )
        .unwrap();

        assert_eq!(gguf.artifacts().len(), 1);
        assert_eq!(gguf.destination_name(), "model-export");
        assert_eq!(gguf.source(), &ModelExportSource::Gguf);

        assert!(
            operation(vec![
                artifact("manifests/registry.ollama.ai/library/model/latest"),
                artifact(&format!("blobs/sha256-{}", "b".repeat(64))),
            ])
            .is_ok()
        );
    }

    #[test]
    fn rejects_invalid_operation_ids() {
        for value in ["", "../escape", "a b", "a/b", "é"] {
            assert!(ModelExportOperationId::new(value).is_err());
        }
    }

    #[test]
    fn rejects_unsafe_artifact_paths() {
        for value in [
            "",
            "/absolute",
            "../escape",
            "models/../escape",
            "models//file",
            "models/./file",
            "models\\file",
            "models/\nfile",
        ] {
            assert!(ModelExportArtifact::new(value, "a".repeat(64), 1).is_err());
        }
    }

    #[test]
    fn rejects_invalid_sha256() {
        for value in ["", "abc", "g".repeat(64).as_str(), "A".repeat(64).as_str()] {
            assert!(ModelExportArtifact::new("model.gguf", value, 1).is_err());
        }
    }

    #[test]
    fn rejects_invalid_destinations() {
        for name in ["", ".", "..", "../escape", "a/b", "-option"] {
            assert!(
                ModelExportOperation::new(
                    ModelExportOperationId::new("id").unwrap(),
                    ModelExportSource::Gguf,
                    name,
                    vec![artifact("model.gguf")],
                )
                .is_err()
            );
        }
    }

    #[test]
    fn rejects_declared_total_size_overflow() {
        assert_eq!(
            operation(vec![
                ModelExportArtifact::new("a.gguf", "a".repeat(64), u64::MAX).unwrap(),
                ModelExportArtifact::new("b.gguf", "a".repeat(64), 1).unwrap(),
            ])
            .unwrap_err(),
            ModelExportValidationError::TotalSizeOverflow
        );

        assert!(
            operation(vec![
                ModelExportArtifact::new("a.gguf", "a".repeat(64), u64::MAX).unwrap(),
            ])
            .is_ok()
        );
    }

    #[test]
    fn rejects_excessive_artifact_count() {
        let artifact = ModelExportArtifact::new("same.gguf", "a".repeat(64), 0).unwrap();

        let artifacts = vec![artifact; 100_001];

        assert_eq!(
            operation(artifacts).unwrap_err(),
            ModelExportValidationError::TooManyArtifacts
        );
    }

    #[test]
    fn canonical_encoding_matches_fixed_byte_fixture() {
        fn field(output: &mut Vec<u8>, tag: u8, value: &[u8]) {
            output.push(tag);
            output.extend_from_slice(&(value.len() as u64).to_be_bytes());
            output.extend_from_slice(value);
        }

        let operation = ModelExportOperation::new(
            ModelExportOperationId::new("id").unwrap(),
            ModelExportSource::Gguf,
            "out",
            vec![ModelExportArtifact::new("a.gguf", "a".repeat(64), 12).unwrap()],
        )
        .unwrap();

        let mut expected = b"DAIA-MODEL-EXPORT\0".to_vec();
        expected.extend_from_slice(&1_u32.to_be_bytes());

        field(&mut expected, 1, b"id");
        field(&mut expected, 2, b"gguf");
        field(&mut expected, 3, b"out");
        expected.extend_from_slice(&1_u64.to_be_bytes());
        field(&mut expected, 4, b"a.gguf");
        field(&mut expected, 5, "a".repeat(64).as_bytes());
        field(&mut expected, 6, &12_u64.to_be_bytes());

        assert_eq!(operation.canonical_bytes(), expected);
        assert_eq!(&operation.canonical_bytes()[..18], b"DAIA-MODEL-EXPORT\0");
    }

    #[test]
    fn canonical_encoding_matches_independent_golden_digest() {
        let operation = ModelExportOperation::new(
            ModelExportOperationId::new("id").unwrap(),
            ModelExportSource::Gguf,
            "out",
            vec![ModelExportArtifact::new("a.gguf", "a".repeat(64), 12).unwrap()],
        )
        .unwrap();

        // SHA-256 calculated independently from the version-1 byte specification.
        // This is a regression fixture, not an authorization signature.
        let actual = operation.canonical_bytes();
        assert_eq!(actual.len(), 171);
        assert_eq!(
            actual,
            include_bytes!("model_export_golden_v1.bin").as_slice()
        );
    }

    #[test]
    fn canonical_encoding_ignores_artifact_input_order() {
        let first = operation(vec![artifact("models/a.gguf"), artifact("models/b.gguf")]).unwrap();

        let second = operation(vec![artifact("models/b.gguf"), artifact("models/a.gguf")]).unwrap();

        assert_eq!(first.canonical_bytes(), second.canonical_bytes());
    }

    #[test]
    fn canonical_encoding_binds_operation_fields() {
        let baseline = operation(vec![artifact("model.gguf")]).unwrap();
        let baseline_bytes = baseline.canonical_bytes();

        let different_id = ModelExportOperation::new(
            ModelExportOperationId::new("different-id").unwrap(),
            ModelExportSource::Ollama,
            "offline-model",
            vec![artifact("model.gguf")],
        )
        .unwrap();

        let different_source = ModelExportOperation::new(
            ModelExportOperationId::new("operation-1").unwrap(),
            ModelExportSource::Gguf,
            "offline-model",
            vec![artifact("model.gguf")],
        )
        .unwrap();

        let different_destination = ModelExportOperation::new(
            ModelExportOperationId::new("operation-1").unwrap(),
            ModelExportSource::Ollama,
            "different-destination",
            vec![artifact("model.gguf")],
        )
        .unwrap();

        let different_digest = operation(vec![
            ModelExportArtifact::new("model.gguf", "b".repeat(64), 12).unwrap(),
        ])
        .unwrap();

        let different_size = operation(vec![
            ModelExportArtifact::new("model.gguf", "a".repeat(64), 13).unwrap(),
        ])
        .unwrap();

        let different_path = operation(vec![artifact("other.gguf")]).unwrap();

        for changed in [
            different_id,
            different_source,
            different_destination,
            different_digest,
            different_size,
            different_path,
        ] {
            assert_ne!(baseline_bytes, changed.canonical_bytes());
        }
    }

    #[test]
    fn canonical_encoding_is_length_delimited() {
        let first = operation(vec![artifact("a/b.gguf")]).unwrap();
        let second = operation(vec![artifact("a-b.gguf")]).unwrap();

        assert_ne!(first.canonical_bytes(), second.canonical_bytes());
        assert!(first.canonical_bytes().starts_with(b"DAIA-MODEL-EXPORT"));
    }

    #[test]
    fn rejects_artifact_ancestor_conflicts_in_either_order() {
        for paths in [
            ["models/a", "models/a/file.gguf"],
            ["models/a/file.gguf", "models/a"],
        ] {
            assert_eq!(
                operation(paths.into_iter().map(artifact).collect()).unwrap_err(),
                ModelExportValidationError::ConflictingArtifactPath
            );
        }

        assert!(
            operation(vec![
                artifact("models/a.gguf"),
                artifact("models/a/file.gguf"),
            ])
            .is_ok()
        );
    }

    #[test]
    fn rejects_empty_and_duplicate_artifacts() {
        assert_eq!(
            operation(vec![]).unwrap_err(),
            ModelExportValidationError::EmptyArtifacts
        );

        assert_eq!(
            operation(vec![artifact("a.gguf"), artifact("a.gguf")]).unwrap_err(),
            ModelExportValidationError::DuplicateArtifactPath
        );
    }
}
