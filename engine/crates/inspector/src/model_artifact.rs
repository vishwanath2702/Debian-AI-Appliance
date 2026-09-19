//! AI model artifact recognition.

use std::{fs::File, io::Read, path::Path};

use crate::{GgufMetadata, ModelInspectError, inspect_gguf};

const GGUF_MAGIC: [u8; 4] = *b"GGUF";

/// Inspects a path when it contains a recognized AI model artifact.
///
/// Returns `Ok(None)` when the file is not a recognized model artifact.
///
/// # Errors
///
/// Returns an error when the artifact cannot be read or when a recognized
/// artifact is malformed.
pub fn inspect_model_artifact(path: &Path) -> Result<Option<GgufMetadata>, ModelInspectError> {
    let mut file = File::open(path)?;
    let mut magic = [0_u8; 4];

    let bytes_read = file.read(&mut magic)?;

    if bytes_read != magic.len() || magic != GGUF_MAGIC {
        return Ok(None);
    }

    inspect_gguf(path).map(Some)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::inspect_model_artifact;
    use crate::ModelInspectError;

    #[test]
    fn ignores_unrecognized_artifact() {
        let directory = tempfile::tempdir().expect("temporary directory should exist");
        let path = directory.path().join("manual.pdf");

        fs::write(&path, b"%PDF").expect("test artifact should be written");

        let metadata =
            inspect_model_artifact(&path).expect("unrecognized artifact should not fail");

        assert!(metadata.is_none());
    }

    #[test]
    fn recognizes_valid_gguf() {
        let directory = tempfile::tempdir().expect("temporary directory should exist");
        let path = directory.path().join("model.bin");

        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"GGUF");
        bytes.extend_from_slice(&3_u32.to_le_bytes());
        bytes.extend_from_slice(&0_u64.to_le_bytes());
        bytes.extend_from_slice(&1_u64.to_le_bytes());

        let key = b"general.architecture";
        bytes.extend_from_slice(&(key.len() as u64).to_le_bytes());
        bytes.extend_from_slice(key);
        bytes.extend_from_slice(&8_u32.to_le_bytes());

        let architecture = b"llama";
        bytes.extend_from_slice(&(architecture.len() as u64).to_le_bytes());
        bytes.extend_from_slice(architecture);

        fs::write(&path, bytes).expect("GGUF test artifact should be written");

        let metadata = inspect_model_artifact(&path)
            .expect("valid GGUF should be inspected")
            .expect("GGUF should be recognized");

        assert_eq!(metadata.version(), 3);
        assert_eq!(metadata.tensor_count(), 0);
        assert_eq!(metadata.metadata_kv_count(), 1);
        assert_eq!(metadata.architecture(), "llama");
        assert!(metadata.tensor_types().is_empty());
    }

    #[test]
    fn rejects_malformed_recognized_gguf() {
        let directory = tempfile::tempdir().expect("temporary directory should exist");
        let path = directory.path().join("model.bin");

        fs::write(&path, b"GGUF").expect("test artifact should be written");

        let error = inspect_model_artifact(&path).expect_err("malformed GGUF should fail");

        assert!(matches!(error, ModelInspectError::Io(_)));
    }
}
