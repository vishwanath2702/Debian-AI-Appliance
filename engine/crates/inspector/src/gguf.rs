//! GGUF model artifact inspection.

use std::{fs::File, io::Read, path::Path};

use crate::ModelInspectError;

const GGUF_MAGIC: [u8; 4] = *b"GGUF";

/// Metadata read from a GGUF model artifact.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GgufMetadata {
    version: u32,
    tensor_count: u64,
    metadata_kv_count: u64,
}

impl GgufMetadata {
    /// Returns the GGUF format version declared by the artifact.
    #[must_use]
    pub const fn version(self) -> u32 {
        self.version
    }

    /// Returns the number of tensors declared by the artifact.
    #[must_use]
    pub const fn tensor_count(self) -> u64 {
        self.tensor_count
    }

    /// Returns the number of metadata key/value pairs declared by the artifact.
    #[must_use]
    pub const fn metadata_kv_count(self) -> u64 {
        self.metadata_kv_count
    }
}

/// Inspects the structural header of a little-endian GGUF model artifact.
///
/// # Errors
///
/// Returns an error when the file cannot be read or does not begin with a
/// valid GGUF header.
pub fn inspect_gguf(path: impl AsRef<Path>) -> Result<GgufMetadata, ModelInspectError> {
    let mut file = File::open(path)?;
    let mut header = [0_u8; 24];

    file.read_exact(&mut header)?;

    if header[..4] != GGUF_MAGIC {
        return Err(ModelInspectError::InvalidGguf(
            "missing GGUF magic".to_owned(),
        ));
    }

    let version = u32::from_le_bytes(
        header[4..8]
            .try_into()
            .expect("GGUF version header is exactly four bytes"),
    );
    let tensor_count = u64::from_le_bytes(
        header[8..16]
            .try_into()
            .expect("GGUF tensor count is exactly eight bytes"),
    );
    let metadata_kv_count = u64::from_le_bytes(
        header[16..24]
            .try_into()
            .expect("GGUF metadata count is exactly eight bytes"),
    );

    Ok(GgufMetadata {
        version,
        tensor_count,
        metadata_kv_count,
    })
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::inspect_gguf;
    use crate::ModelInspectError;

    #[test]
    fn inspects_gguf_version() {
        let directory = tempfile::tempdir().expect("temporary directory should exist");
        let path = directory.path().join("model.bin");

        let mut bytes = Vec::from(*b"GGUF");
        bytes.extend_from_slice(&3_u32.to_le_bytes());
        bytes.extend_from_slice(&42_u64.to_le_bytes());
        bytes.extend_from_slice(&7_u64.to_le_bytes());

        fs::write(&path, bytes).expect("GGUF test artifact should be written");

        let metadata = inspect_gguf(&path).expect("GGUF header should be recognized");

        assert_eq!(metadata.version(), 3);
        assert_eq!(metadata.tensor_count(), 42);
        assert_eq!(metadata.metadata_kv_count(), 7);
    }

    #[test]
    fn rejects_non_gguf_file() {
        let directory = tempfile::tempdir().expect("temporary directory should exist");
        let path = directory.path().join("model.bin");

        let mut bytes = Vec::from(*b"NOPE");
        bytes.extend_from_slice(&3_u32.to_le_bytes());
        bytes.extend_from_slice(&0_u64.to_le_bytes());
        bytes.extend_from_slice(&0_u64.to_le_bytes());

        fs::write(&path, bytes).expect("non-GGUF test artifact should be written");

        let error = inspect_gguf(&path).expect_err("non-GGUF file should be rejected");

        assert!(matches!(error, ModelInspectError::InvalidGguf(_)));
    }

    #[test]
    fn reports_truncated_gguf_header_as_io_error() {
        let directory = tempfile::tempdir().expect("temporary directory should exist");
        let path = directory.path().join("model.bin");

        let mut bytes = Vec::from(*b"GGUF");
        bytes.extend_from_slice(&3_u32.to_le_bytes());

        fs::write(&path, bytes).expect("truncated GGUF test artifact should be written");

        let error = inspect_gguf(&path).expect_err("truncated GGUF header should fail");

        assert!(matches!(
            error,
            ModelInspectError::Io(error)
                if error.kind() == std::io::ErrorKind::UnexpectedEof
        ));
    }
}
