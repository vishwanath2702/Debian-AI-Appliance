//! Canonical identity of a requested model export.
//!
//! Request encoding describes intent only. It does not authorize access,
//! establish filesystem identity, or protect against hostile same-UID peers.

use crate::{ModelExportOperation, ModelExportResourceSelection};

/// Deterministic versioned encoding of an operation and its selected resources.
///
/// This preserves the existing broker request encoding exactly.
#[must_use]
pub fn canonical_export_request_bytes(
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ModelExportArtifact, ModelExportDestinationSelection, ModelExportOperationId,
        ModelExportSource, ModelExportSourceSelection,
    };

    #[test]
    fn request_encoding_preserves_broker_wire_format() {
        let operation = ModelExportOperation::new(
            ModelExportOperationId::new("operation-1").unwrap(),
            ModelExportSource::Ollama,
            "export",
            vec![ModelExportArtifact::new("model.gguf", "a".repeat(64), 12).unwrap()],
        )
        .unwrap();

        let selection = ModelExportResourceSelection::new(
            ModelExportSourceSelection::ollama(
                "/media/models",
                "registry.ollama.ai/library/qwen/latest",
            )
            .unwrap(),
            ModelExportDestinationSelection::new("/media/export").unwrap(),
        );

        let operation_bytes = operation.canonical_bytes();
        let selection_bytes = selection.canonical_bytes();

        let mut expected = b"DAIA-MODEL-EXPORT-REQUEST\0".to_vec();
        expected.extend_from_slice(&1_u32.to_be_bytes());
        expected.extend_from_slice(&(operation_bytes.len() as u64).to_be_bytes());
        expected.extend_from_slice(&operation_bytes);
        expected.extend_from_slice(&(selection_bytes.len() as u64).to_be_bytes());
        expected.extend_from_slice(&selection_bytes);

        assert_eq!(
            canonical_export_request_bytes(&operation, &selection),
            expected,
        );
    }
}
