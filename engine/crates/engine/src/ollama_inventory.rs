//! Decoding of installed Ollama model inventory.

use model::BenchmarkError;
use serde::Serialize;
use serde_json::Value;

/// Metadata returned by Ollama for an installed model.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct InstalledOllamaModel {
    pub name: String,
    pub model: Option<String>,
    pub modified_at: Option<String>,
    pub size_bytes: u64,
    pub digest: Option<String>,
    pub details: OllamaModelDetails,
}

/// Optional model characteristics reported by Ollama.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct OllamaModelDetails {
    pub parent_model: Option<String>,
    pub format: Option<String>,
    pub family: Option<String>,
    pub families: Vec<String>,
    pub parameter_size: Option<String>,
    pub quantization_level: Option<String>,
}

/// Decodes the JSON body returned by Ollama's `/api/tags`.
///
/// # Errors
///
/// Rejects malformed JSON and invalid required model fields.
pub fn decode_installed_models(body: &[u8]) -> Result<Vec<InstalledOllamaModel>, BenchmarkError> {
    let value: Value = serde_json::from_slice(body)
        .map_err(|_| BenchmarkError::Execution("invalid Ollama inventory JSON".into()))?;

    let models = value
        .get("models")
        .and_then(Value::as_array)
        .ok_or_else(|| BenchmarkError::Execution("missing Ollama models array".into()))?;

    models.iter().map(decode_model).collect()
}

fn optional_string(value: &Value, field: &str) -> Result<Option<String>, BenchmarkError> {
    match value.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(text)) => Ok(Some(text.clone())),
        _ => Err(BenchmarkError::Execution(format!(
            "invalid Ollama model field: {field}"
        ))),
    }
}

fn decode_model(value: &Value) -> Result<InstalledOllamaModel, BenchmarkError> {
    let name = value
        .get("name")
        .and_then(Value::as_str)
        .filter(|name| !name.is_empty())
        .ok_or_else(|| BenchmarkError::Execution("missing Ollama model name".into()))?
        .to_owned();

    let size_bytes = value
        .get("size")
        .and_then(Value::as_u64)
        .ok_or_else(|| BenchmarkError::Execution("invalid Ollama model size".into()))?;

    let details = match value.get("details") {
        None | Some(Value::Null) => OllamaModelDetails::default(),
        Some(value) if value.is_object() => {
            let families = match value.get("families") {
                None | Some(Value::Null) => Vec::new(),
                Some(Value::Array(items)) => items
                    .iter()
                    .map(|item| {
                        item.as_str().map(str::to_owned).ok_or_else(|| {
                            BenchmarkError::Execution("invalid Ollama model families".into())
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?,
                _ => {
                    return Err(BenchmarkError::Execution(
                        "invalid Ollama model families".into(),
                    ));
                }
            };

            OllamaModelDetails {
                parent_model: optional_string(value, "parent_model")?,
                format: optional_string(value, "format")?,
                family: optional_string(value, "family")?,
                families,
                parameter_size: optional_string(value, "parameter_size")?,
                quantization_level: optional_string(value, "quantization_level")?,
            }
        }
        _ => {
            return Err(BenchmarkError::Execution(
                "invalid Ollama model details".into(),
            ));
        }
    };

    Ok(InstalledOllamaModel {
        name,
        model: optional_string(value, "model")?,
        modified_at: optional_string(value, "modified_at")?,
        size_bytes,
        digest: optional_string(value, "digest")?,
        details,
    })
}

/// Retrieves and decodes models registered with the local Ollama service.
///
/// # Errors
///
/// Returns an error if Ollama is unavailable or its inventory is invalid.
pub fn installed_ollama_models() -> Result<Vec<InstalledOllamaModel>, BenchmarkError> {
    let body = crate::benchmark_ollama_transport::get_installed_models()?;
    decode_installed_models(&body)
}

/// Retrieves and decodes models from an Ollama service on a loopback port.
///
/// # Errors
///
/// Returns an error for an invalid port, transport failure, or invalid inventory.
pub fn installed_ollama_models_from_port(
    port: u16,
) -> Result<Vec<InstalledOllamaModel>, BenchmarkError> {
    let body = crate::benchmark_ollama_transport::get_installed_models_from_port(port)?;
    decode_installed_models(&body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retrieves_and_decodes_inventory_over_loopback() {
        use std::io::{Read, Write};
        use std::net::{Ipv4Addr, TcpListener};
        use std::thread;

        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = listener.local_addr().unwrap().port();

        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(2)))
                .unwrap();

            let mut request = [0_u8; 1024];
            let count = stream.read(&mut request).unwrap();
            assert!(request[..count].starts_with(b"GET /api/tags HTTP/1.1\r\n"));

            let body = br#"{"models":[{"name":"local:latest","size":42,"digest":"sha256:abc"}]}"#;
            let header = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );

            stream.write_all(header.as_bytes()).unwrap();
            stream.write_all(body).unwrap();
        });

        let result = installed_ollama_models_from_port(port);
        server.join().unwrap();

        let models = result.unwrap();
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].name, "local:latest");
        assert_eq!(models[0].size_bytes, 42);
        assert_eq!(models[0].digest.as_deref(), Some("sha256:abc"));
    }

    #[test]
    fn installed_inventory_rejects_zero_port() {
        assert!(installed_ollama_models_from_port(0).is_err());
    }

    #[test]
    fn decodes_empty_inventory() {
        assert_eq!(
            decode_installed_models(br#"{"models":[]}"#).unwrap(),
            vec![]
        );
    }

    #[test]
    fn preserves_name_tag_and_metadata() {
        let body = br#"{
            "models":[{
                "name":"qwen2.5:7b",
                "model":"qwen2.5:7b",
                "modified_at":"2026-10-30T10:20:30Z",
                "size":4683072000,
                "digest":"sha256:abc123",
                "details":{
                    "parent_model":"",
                    "format":"gguf",
                    "family":"qwen2",
                    "families":["qwen2"],
                    "parameter_size":"7.6B",
                    "quantization_level":"Q4_K_M"
                }
            }]
        }"#;

        let models = decode_installed_models(body).unwrap();
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].name, "qwen2.5:7b");
        assert_eq!(models[0].size_bytes, 4683072000);
        assert_eq!(models[0].digest.as_deref(), Some("sha256:abc123"));
        assert_eq!(models[0].details.family.as_deref(), Some("qwen2"));
        assert_eq!(
            models[0].details.quantization_level.as_deref(),
            Some("Q4_K_M")
        );
    }

    #[test]
    fn accepts_missing_optional_metadata() {
        let models =
            decode_installed_models(br#"{"models":[{"name":"local:latest","size":0}]}"#).unwrap();
        assert_eq!(models[0].name, "local:latest");
        assert_eq!(models[0].details, OllamaModelDetails::default());
    }

    #[test]
    fn rejects_invalid_inventory() {
        for body in [
            br#"{}"#.as_slice(),
            br#"{"models":{}}"#,
            br#"{"models":[{"size":1}]}"#,
            br#"{"models":[{"name":"x","size":-1}]}"#,
            br#"{"models":[{"name":"x","size":1,"details":{"families":[7]}}]}"#,
            b"not-json",
        ] {
            assert!(decode_installed_models(body).is_err());
        }
    }
}
