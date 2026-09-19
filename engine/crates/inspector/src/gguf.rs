//! GGUF model artifact inspection.

use std::{fs::File, io::Read, path::Path};

use crate::ModelInspectError;

const GGUF_MAGIC: [u8; 4] = *b"GGUF";
const GGUF_MAX_METADATA_KEY_LENGTH: u64 = 65_535;

const GGUF_TYPE_UINT8: u32 = 0;
const GGUF_TYPE_INT8: u32 = 1;
const GGUF_TYPE_UINT16: u32 = 2;
const GGUF_TYPE_INT16: u32 = 3;
const GGUF_TYPE_UINT32: u32 = 4;
const GGUF_TYPE_INT32: u32 = 5;
const GGUF_TYPE_FLOAT32: u32 = 6;
const GGUF_TYPE_BOOL: u32 = 7;
const GGUF_TYPE_STRING: u32 = 8;
const GGUF_TYPE_ARRAY: u32 = 9;
const GGUF_TYPE_UINT64: u32 = 10;
const GGUF_TYPE_INT64: u32 = 11;
const GGUF_TYPE_FLOAT64: u32 = 12;

/// Metadata read from a GGUF model artifact.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GgufMetadata {
    version: u32,
    tensor_count: u64,
    metadata_kv_count: u64,
    size_bytes: u64,
    architecture: String,
    name: Option<String>,
    tensor_types: Vec<u32>,
}

impl GgufMetadata {
    /// Returns the GGUF format version declared by the artifact.
    #[must_use]
    pub const fn version(&self) -> u32 {
        self.version
    }

    /// Returns the number of tensors declared by the artifact.
    #[must_use]
    pub const fn tensor_count(&self) -> u64 {
        self.tensor_count
    }

    /// Returns the number of metadata key/value pairs declared by the artifact.
    #[must_use]
    pub const fn metadata_kv_count(&self) -> u64 {
        self.metadata_kv_count
    }

    /// Returns the size of the model artifact in bytes.
    #[must_use]
    pub const fn size_bytes(&self) -> u64 {
        self.size_bytes
    }

    /// Returns the model architecture declared by the artifact.
    #[must_use]
    pub fn architecture(&self) -> &str {
        &self.architecture
    }

    /// Returns the model name declared by the artifact, when present.
    #[must_use]
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Returns the raw ggml types declared by the tensor table.
    #[must_use]
    pub fn tensor_types(&self) -> &[u32] {
        &self.tensor_types
    }
}

/// Inspects structural metadata from a little-endian GGUF model artifact.
///
/// # Errors
///
/// Returns an error when the file cannot be read or does not begin with a
/// valid GGUF header.
pub fn inspect_gguf(path: impl AsRef<Path>) -> Result<GgufMetadata, ModelInspectError> {
    let mut file = File::open(path)?;
    let size_bytes = file.metadata()?.len();
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

    let mut architecture = None;
    let mut name = None;

    for _ in 0..metadata_kv_count {
        let key = read_metadata_key(&mut file)?;
        let value_type = read_u32(&mut file)?;

        if key == "general.architecture" {
            if value_type != GGUF_TYPE_STRING {
                return Err(ModelInspectError::InvalidGguf(
                    "general.architecture is not a string".to_owned(),
                ));
            }

            architecture = Some(read_string(&mut file)?);
        } else if key == "general.name" {
            if value_type != GGUF_TYPE_STRING {
                return Err(ModelInspectError::InvalidGguf(
                    "general.name is not a string".to_owned(),
                ));
            }

            name = Some(read_string(&mut file)?);
        } else {
            skip_value(&mut file, value_type)?;
        }
    }

    let architecture = architecture.ok_or_else(|| {
        ModelInspectError::InvalidGguf("missing general.architecture metadata".to_owned())
    })?;

    let tensor_types = read_tensor_types(&mut file, tensor_count)?;

    Ok(GgufMetadata {
        version,
        tensor_count,
        metadata_kv_count,
        size_bytes,
        architecture,
        name,
        tensor_types,
    })
}

fn read_tensor_types(
    reader: &mut impl Read,
    tensor_count: u64,
) -> Result<Vec<u32>, ModelInspectError> {
    let tensor_count = usize::try_from(tensor_count).map_err(|_| {
        ModelInspectError::InvalidGguf("tensor count cannot be represented".to_owned())
    })?;

    let mut tensor_types = Vec::new();
    tensor_types
        .try_reserve_exact(tensor_count)
        .map_err(|_| ModelInspectError::InvalidGguf("tensor count is too large".to_owned()))?;

    for _ in 0..tensor_count {
        let name = read_string(reader)?;

        if name.len() >= 64 {
            return Err(ModelInspectError::InvalidGguf(
                "tensor name is too long".to_owned(),
            ));
        }

        let dimension_count = read_u32(reader)?;

        if dimension_count > 4 {
            return Err(ModelInspectError::InvalidGguf(
                "tensor has too many dimensions".to_owned(),
            ));
        }

        for _ in 0..dimension_count {
            read_u64(reader)?;
        }

        let tensor_type = read_u32(reader)?;

        if !is_supported_tensor_type(tensor_type) {
            return Err(ModelInspectError::InvalidGguf(
                "tensor has unsupported ggml type".to_owned(),
            ));
        }

        read_u64(reader)?;

        tensor_types.push(tensor_type);
    }

    Ok(tensor_types)
}

fn is_supported_tensor_type(tensor_type: u32) -> bool {
    matches!(
        tensor_type,
        0..=3 | 6..=30 | 34..=35 | 39..=42
    )
}

fn read_u32(reader: &mut impl Read) -> Result<u32, ModelInspectError> {
    let mut bytes = [0_u8; 4];
    reader.read_exact(&mut bytes)?;
    Ok(u32::from_le_bytes(bytes))
}

fn read_u64(reader: &mut impl Read) -> Result<u64, ModelInspectError> {
    let mut bytes = [0_u8; 8];
    reader.read_exact(&mut bytes)?;
    Ok(u64::from_le_bytes(bytes))
}

fn read_string(reader: &mut impl Read) -> Result<String, ModelInspectError> {
    let length = read_u64(reader)?;
    let length = usize::try_from(length)
        .map_err(|_| ModelInspectError::InvalidGguf("string length is too large".to_owned()))?;

    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(length)
        .map_err(|_| ModelInspectError::InvalidGguf("string length is too large".to_owned()))?;
    bytes.resize(length, 0);
    reader.read_exact(&mut bytes)?;

    String::from_utf8(bytes)
        .map_err(|_| ModelInspectError::InvalidGguf("string is not valid UTF-8".to_owned()))
}

fn read_metadata_key(reader: &mut impl Read) -> Result<String, ModelInspectError> {
    let length = read_u64(reader)?;

    if length > GGUF_MAX_METADATA_KEY_LENGTH {
        return Err(ModelInspectError::InvalidGguf(
            "metadata key exceeds maximum length".to_owned(),
        ));
    }

    let length = usize::try_from(length).map_err(|_| {
        ModelInspectError::InvalidGguf("metadata key length is too large".to_owned())
    })?;

    let mut bytes = vec![0_u8; length];
    reader.read_exact(&mut bytes)?;

    let key = String::from_utf8(bytes).map_err(|_| {
        ModelInspectError::InvalidGguf("metadata key is not valid UTF-8".to_owned())
    })?;

    let valid = !key.is_empty()
        && key.split('.').all(|segment| {
            !segment.is_empty()
                && segment
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
        });

    if !valid {
        return Err(ModelInspectError::InvalidGguf(
            "metadata key is invalid".to_owned(),
        ));
    }

    Ok(key)
}

fn skip_bytes(reader: &mut impl Read, mut length: u64) -> Result<(), ModelInspectError> {
    let mut buffer = [0_u8; 8192];

    while length != 0 {
        let chunk = usize::try_from(length.min(buffer.len() as u64))
            .expect("bounded GGUF skip chunk fits usize");
        reader.read_exact(&mut buffer[..chunk])?;
        length -= chunk as u64;
    }

    Ok(())
}

fn skip_value(reader: &mut impl Read, value_type: u32) -> Result<(), ModelInspectError> {
    match value_type {
        GGUF_TYPE_UINT8 | GGUF_TYPE_INT8 => skip_bytes(reader, 1),
        GGUF_TYPE_UINT16 | GGUF_TYPE_INT16 => skip_bytes(reader, 2),
        GGUF_TYPE_UINT32 | GGUF_TYPE_INT32 | GGUF_TYPE_FLOAT32 => skip_bytes(reader, 4),
        GGUF_TYPE_UINT64 | GGUF_TYPE_INT64 | GGUF_TYPE_FLOAT64 => skip_bytes(reader, 8),
        GGUF_TYPE_BOOL => {
            let mut value = [0_u8; 1];
            reader.read_exact(&mut value)?;

            if value[0] > 1 {
                return Err(ModelInspectError::InvalidGguf(
                    "boolean metadata value is invalid".to_owned(),
                ));
            }

            Ok(())
        }
        GGUF_TYPE_STRING => {
            let length = read_u64(reader)?;
            skip_bytes(reader, length)
        }
        GGUF_TYPE_ARRAY => {
            let element_type = read_u32(reader)?;

            if element_type == GGUF_TYPE_ARRAY {
                return Err(ModelInspectError::InvalidGguf(
                    "metadata arrays cannot contain arrays".to_owned(),
                ));
            }

            validate_value_type(element_type)?;

            let length = read_u64(reader)?;

            for _ in 0..length {
                skip_value(reader, element_type)?;
            }

            Ok(())
        }
        _ => Err(ModelInspectError::InvalidGguf(
            "unknown metadata value type".to_owned(),
        )),
    }
}

fn validate_value_type(value_type: u32) -> Result<(), ModelInspectError> {
    if value_type <= GGUF_TYPE_FLOAT64 {
        Ok(())
    } else {
        Err(ModelInspectError::InvalidGguf(
            "unknown metadata value type".to_owned(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::inspect_gguf;
    use crate::ModelInspectError;

    fn push_string(bytes: &mut Vec<u8>, value: &str) {
        bytes.extend_from_slice(&(value.len() as u64).to_le_bytes());
        bytes.extend_from_slice(value.as_bytes());
    }

    fn push_metadata_string(bytes: &mut Vec<u8>, key: &str, value: &str) {
        push_string(bytes, key);
        bytes.extend_from_slice(&super::GGUF_TYPE_STRING.to_le_bytes());
        push_string(bytes, value);
    }

    fn gguf_with_metadata_count(metadata_kv_count: u64) -> Vec<u8> {
        let mut bytes = Vec::from(*b"GGUF");
        bytes.extend_from_slice(&3_u32.to_le_bytes());
        bytes.extend_from_slice(&0_u64.to_le_bytes());
        bytes.extend_from_slice(&metadata_kv_count.to_le_bytes());
        bytes
    }

    #[test]
    fn inspects_gguf_version() {
        let directory = tempfile::tempdir().expect("temporary directory should exist");
        let path = directory.path().join("model.bin");

        let mut bytes = gguf_with_metadata_count(1);
        push_metadata_string(&mut bytes, "general.architecture", "llama");

        fs::write(&path, bytes).expect("GGUF test artifact should be written");

        let metadata = inspect_gguf(&path).expect("GGUF metadata should be recognized");

        assert_eq!(metadata.version(), 3);
        assert_eq!(metadata.tensor_count(), 0);
        assert_eq!(metadata.metadata_kv_count(), 1);
        assert_eq!(
            metadata.size_bytes(),
            fs::metadata(&path)
                .expect("GGUF test artifact metadata should be readable")
                .len()
        );
        assert_eq!(metadata.architecture(), "llama");
        assert!(metadata.tensor_types().is_empty());
    }

    #[test]
    fn inspects_optional_model_name() {
        let directory = tempfile::tempdir().expect("temporary directory should exist");
        let path = directory.path().join("model.bin");

        let mut bytes = gguf_with_metadata_count(2);
        push_metadata_string(&mut bytes, "general.architecture", "llama");
        push_metadata_string(&mut bytes, "general.name", "Tiny Llama");

        fs::write(&path, bytes).expect("GGUF test artifact should be written");

        let metadata = inspect_gguf(&path).expect("GGUF metadata should be recognized");

        assert_eq!(metadata.name(), Some("Tiny Llama"));
    }

    #[test]
    fn rejects_non_string_model_name() {
        let directory = tempfile::tempdir().expect("temporary directory should exist");
        let path = directory.path().join("model.bin");

        let mut bytes = gguf_with_metadata_count(2);
        push_metadata_string(&mut bytes, "general.architecture", "llama");
        push_string(&mut bytes, "general.name");
        bytes.extend_from_slice(&super::GGUF_TYPE_UINT32.to_le_bytes());
        bytes.extend_from_slice(&1_u32.to_le_bytes());

        fs::write(&path, bytes).expect("GGUF test artifact should be written");

        let error = inspect_gguf(&path).expect_err("non-string model name should be rejected");

        assert!(matches!(
            error,
            ModelInspectError::InvalidGguf(message)
                if message == "general.name is not a string"
        ));
    }

    #[test]
    fn model_name_is_optional() {
        let directory = tempfile::tempdir().expect("temporary directory should exist");
        let path = directory.path().join("model.bin");

        let mut bytes = gguf_with_metadata_count(1);
        push_metadata_string(&mut bytes, "general.architecture", "llama");

        fs::write(&path, bytes).expect("GGUF test artifact should be written");

        let metadata = inspect_gguf(&path).expect("GGUF metadata should be recognized");

        assert_eq!(metadata.name(), None);
    }

    #[test]
    fn inspects_all_tensor_types() {
        let directory = tempfile::tempdir().expect("temporary directory should exist");
        let path = directory.path().join("model.bin");

        let mut bytes = gguf_with_metadata_count(1);
        bytes[8..16].copy_from_slice(&2_u64.to_le_bytes());
        push_metadata_string(&mut bytes, "general.architecture", "llama");

        push_string(&mut bytes, "token_embd.weight");
        bytes.extend_from_slice(&2_u32.to_le_bytes());
        bytes.extend_from_slice(&32000_u64.to_le_bytes());
        bytes.extend_from_slice(&4096_u64.to_le_bytes());
        bytes.extend_from_slice(&7_u32.to_le_bytes());
        bytes.extend_from_slice(&0_u64.to_le_bytes());

        push_string(&mut bytes, "output.weight");
        bytes.extend_from_slice(&2_u32.to_le_bytes());
        bytes.extend_from_slice(&32000_u64.to_le_bytes());
        bytes.extend_from_slice(&4096_u64.to_le_bytes());
        bytes.extend_from_slice(&12_u32.to_le_bytes());
        bytes.extend_from_slice(&4096_u64.to_le_bytes());

        fs::write(&path, bytes).expect("GGUF test artifact should be written");

        let metadata = inspect_gguf(&path).expect("GGUF tensor descriptors should be recognized");

        assert_eq!(metadata.tensor_count(), 2);
        assert_eq!(metadata.tensor_types(), [7, 12]);
    }

    #[test]
    fn rejects_unsupported_tensor_types() {
        for tensor_type in [4_u32, 43_u32] {
            let directory = tempfile::tempdir().expect("temporary directory should exist");
            let path = directory.path().join("model.bin");

            let mut bytes = gguf_with_metadata_count(1);
            bytes[8..16].copy_from_slice(&1_u64.to_le_bytes());
            push_metadata_string(&mut bytes, "general.architecture", "llama");

            push_string(&mut bytes, "token_embd.weight");
            bytes.extend_from_slice(&2_u32.to_le_bytes());
            bytes.extend_from_slice(&32000_u64.to_le_bytes());
            bytes.extend_from_slice(&4096_u64.to_le_bytes());
            bytes.extend_from_slice(&tensor_type.to_le_bytes());
            bytes.extend_from_slice(&0_u64.to_le_bytes());

            fs::write(&path, bytes).expect("GGUF test artifact should be written");

            let error = inspect_gguf(&path).expect_err("unsupported tensor type should fail");

            assert!(matches!(error, ModelInspectError::InvalidGguf(_)));
        }
    }

    #[test]
    fn finds_architecture_after_scalar_metadata() {
        let directory = tempfile::tempdir().expect("temporary directory should exist");
        let path = directory.path().join("model.bin");

        let mut bytes = gguf_with_metadata_count(2);
        push_string(&mut bytes, "general.file_type");
        bytes.extend_from_slice(&super::GGUF_TYPE_UINT32.to_le_bytes());
        bytes.extend_from_slice(&7_u32.to_le_bytes());
        push_metadata_string(&mut bytes, "general.architecture", "llama");

        fs::write(&path, bytes).expect("GGUF test artifact should be written");

        let metadata = inspect_gguf(&path).expect("GGUF metadata should be recognized");

        assert_eq!(metadata.architecture(), "llama");
    }

    #[test]
    fn finds_architecture_after_array_metadata() {
        let directory = tempfile::tempdir().expect("temporary directory should exist");
        let path = directory.path().join("model.bin");

        let mut bytes = gguf_with_metadata_count(2);
        push_string(&mut bytes, "general.tags");
        bytes.extend_from_slice(&super::GGUF_TYPE_ARRAY.to_le_bytes());
        bytes.extend_from_slice(&super::GGUF_TYPE_STRING.to_le_bytes());
        bytes.extend_from_slice(&2_u64.to_le_bytes());
        push_string(&mut bytes, "chat");
        push_string(&mut bytes, "instruct");
        push_metadata_string(&mut bytes, "general.architecture", "llama");

        fs::write(&path, bytes).expect("GGUF test artifact should be written");

        let metadata = inspect_gguf(&path).expect("GGUF metadata should be recognized");

        assert_eq!(metadata.architecture(), "llama");
    }

    #[test]
    fn rejects_missing_architecture() {
        let directory = tempfile::tempdir().expect("temporary directory should exist");
        let path = directory.path().join("model.bin");

        let mut bytes = gguf_with_metadata_count(1);
        push_string(&mut bytes, "general.file_type");
        bytes.extend_from_slice(&super::GGUF_TYPE_UINT32.to_le_bytes());
        bytes.extend_from_slice(&7_u32.to_le_bytes());

        fs::write(&path, bytes).expect("GGUF test artifact should be written");

        let error = inspect_gguf(&path).expect_err("missing architecture should fail");

        assert!(matches!(error, ModelInspectError::InvalidGguf(_)));
    }

    #[test]
    fn rejects_non_string_architecture() {
        let directory = tempfile::tempdir().expect("temporary directory should exist");
        let path = directory.path().join("model.bin");

        let mut bytes = gguf_with_metadata_count(1);
        push_string(&mut bytes, "general.architecture");
        bytes.extend_from_slice(&super::GGUF_TYPE_UINT32.to_le_bytes());
        bytes.extend_from_slice(&7_u32.to_le_bytes());

        fs::write(&path, bytes).expect("GGUF test artifact should be written");

        let error = inspect_gguf(&path).expect_err("non-string architecture should fail");

        assert!(matches!(error, ModelInspectError::InvalidGguf(_)));
    }

    #[test]
    fn rejects_unknown_metadata_value_type() {
        let directory = tempfile::tempdir().expect("temporary directory should exist");
        let path = directory.path().join("model.bin");

        let mut bytes = gguf_with_metadata_count(2);
        push_string(&mut bytes, "custom.value");
        bytes.extend_from_slice(&13_u32.to_le_bytes());
        push_metadata_string(&mut bytes, "general.architecture", "llama");

        fs::write(&path, bytes).expect("GGUF test artifact should be written");

        let error = inspect_gguf(&path).expect_err("unknown metadata type should fail");

        assert!(matches!(error, ModelInspectError::InvalidGguf(_)));
    }

    #[test]
    fn rejects_invalid_boolean_metadata() {
        let directory = tempfile::tempdir().expect("temporary directory should exist");
        let path = directory.path().join("model.bin");

        let mut bytes = gguf_with_metadata_count(2);
        push_string(&mut bytes, "custom.flag");
        bytes.extend_from_slice(&super::GGUF_TYPE_BOOL.to_le_bytes());
        bytes.push(2);
        push_metadata_string(&mut bytes, "general.architecture", "llama");

        fs::write(&path, bytes).expect("GGUF test artifact should be written");

        let error = inspect_gguf(&path).expect_err("invalid boolean should fail");

        assert!(matches!(error, ModelInspectError::InvalidGguf(_)));
    }

    #[test]
    fn rejects_invalid_metadata_key() {
        let directory = tempfile::tempdir().expect("temporary directory should exist");
        let path = directory.path().join("model.bin");

        let mut bytes = gguf_with_metadata_count(2);
        push_string(&mut bytes, "Invalid.Key");
        bytes.extend_from_slice(&super::GGUF_TYPE_UINT8.to_le_bytes());
        bytes.push(1);
        push_metadata_string(&mut bytes, "general.architecture", "llama");

        fs::write(&path, bytes).expect("GGUF test artifact should be written");

        let error = inspect_gguf(&path).expect_err("invalid metadata key should fail");

        assert!(matches!(error, ModelInspectError::InvalidGguf(_)));
    }

    #[test]
    fn preserves_declared_architecture_value() {
        let directory = tempfile::tempdir().expect("temporary directory should exist");
        let path = directory.path().join("model.bin");

        let mut bytes = gguf_with_metadata_count(1);
        push_metadata_string(&mut bytes, "general.architecture", "Llama-3");

        fs::write(&path, bytes).expect("GGUF test artifact should be written");

        let metadata = inspect_gguf(&path).expect("GGUF metadata should be recognized");

        assert_eq!(metadata.architecture(), "Llama-3");
    }

    #[test]
    fn rejects_nested_metadata_arrays() {
        let directory = tempfile::tempdir().expect("temporary directory should exist");
        let path = directory.path().join("model.bin");

        let mut bytes = gguf_with_metadata_count(2);
        push_string(&mut bytes, "custom.nested");
        bytes.extend_from_slice(&super::GGUF_TYPE_ARRAY.to_le_bytes());
        bytes.extend_from_slice(&super::GGUF_TYPE_ARRAY.to_le_bytes());
        bytes.extend_from_slice(&1_u64.to_le_bytes());

        push_metadata_string(&mut bytes, "general.architecture", "llama");

        fs::write(&path, bytes).expect("GGUF test artifact should be written");

        let error = inspect_gguf(&path).expect_err("nested array should fail");

        assert!(matches!(error, ModelInspectError::InvalidGguf(_)));
    }

    #[test]
    fn rejects_truncated_metadata_value() {
        let directory = tempfile::tempdir().expect("temporary directory should exist");
        let path = directory.path().join("model.bin");

        let mut bytes = gguf_with_metadata_count(2);
        push_string(&mut bytes, "general.file_type");
        bytes.extend_from_slice(&super::GGUF_TYPE_UINT64.to_le_bytes());
        bytes.extend_from_slice(&[0_u8; 4]);

        fs::write(&path, bytes).expect("GGUF test artifact should be written");

        let error = inspect_gguf(&path).expect_err("truncated metadata should fail");

        assert!(matches!(
            error,
            ModelInspectError::Io(error)
                if error.kind() == std::io::ErrorKind::UnexpectedEof
        ));
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
