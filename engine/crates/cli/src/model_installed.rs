//! Read-only presentation of installed Ollama models.

use engine::ollama_inventory::{InstalledOllamaModel, installed_ollama_models_from_port};
use model::BenchmarkError;
use std::process::ExitCode;

fn format_inventory(models: &[InstalledOllamaModel]) -> String {
    if models.is_empty() {
        return "No Ollama models installed.\n".to_owned();
    }

    let mut output = format!("Installed Ollama models: {}\n", models.len());

    for model in models {
        output.push_str(&format!(
            "\nModel: {}\nSize: {} bytes\n",
            model.name, model.size_bytes
        ));

        if let Some(digest) = &model.digest {
            output.push_str(&format!("Digest: {digest}\n"));
        }

        if let Some(modified) = &model.modified_at {
            output.push_str(&format!("Modified: {modified}\n"));
        }

        if let Some(format) = &model.details.format {
            output.push_str(&format!("Format: {format}\n"));
        }

        if let Some(family) = &model.details.family {
            output.push_str(&format!("Family: {family}\n"));
        }

        if let Some(parameters) = &model.details.parameter_size {
            output.push_str(&format!("Parameters: {parameters}\n"));
        }

        if let Some(quantization) = &model.details.quantization_level {
            output.push_str(&format!("Quantization: {quantization}\n"));
        }
    }

    output
}

fn inventory_port(value: Option<&str>) -> Result<u16, BenchmarkError> {
    match value {
        None => Ok(11434),
        Some(value) => value
            .parse::<u16>()
            .ok()
            .filter(|port| *port != 0)
            .ok_or(BenchmarkError::InvalidConfiguration("ollama_port")),
    }
}

pub fn run(json_output: bool) -> ExitCode {
    let port = match inventory_port(std::env::var("DAIA_INVENTORY_OLLAMA_PORT").ok().as_deref()) {
        Ok(port) => port,
        Err(error) => {
            eprintln!("Invalid Ollama inventory port: {error:?}");
            return ExitCode::FAILURE;
        }
    };

    match installed_ollama_models_from_port(port) {
        Ok(models) => {
            if json_output {
                match serde_json::to_string(&models) {
                    Ok(json) => println!("{json}"),
                    Err(error) => {
                        eprintln!("Failed to serialize installed models: {error}");
                        return ExitCode::FAILURE;
                    }
                }
            } else {
                print!("{}", format_inventory(&models));
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("Failed to list installed Ollama models: {error:?}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::ollama_inventory::OllamaModelDetails;

    #[test]
    fn inventory_port_defaults_and_validates_override() {
        assert_eq!(inventory_port(None), Ok(11434));
        assert_eq!(inventory_port(Some("23456")), Ok(23456));
        assert_eq!(
            inventory_port(Some("0")),
            Err(BenchmarkError::InvalidConfiguration("ollama_port"))
        );
        assert_eq!(
            inventory_port(Some("invalid")),
            Err(BenchmarkError::InvalidConfiguration("ollama_port"))
        );
        assert_eq!(
            inventory_port(Some("65536")),
            Err(BenchmarkError::InvalidConfiguration("ollama_port"))
        );
    }

    #[test]
    fn serializes_installed_inventory_as_json() {
        let models = vec![InstalledOllamaModel {
            name: "qwen2.5:7b".to_owned(),
            model: Some("qwen2.5:7b".to_owned()),
            modified_at: Some("2026-10-30T10:20:30Z".to_owned()),
            size_bytes: 4683072000,
            digest: Some("sha256:abc".to_owned()),
            details: OllamaModelDetails {
                format: Some("gguf".to_owned()),
                family: Some("qwen2".to_owned()),
                parameter_size: Some("7.6B".to_owned()),
                quantization_level: Some("Q4_K_M".to_owned()),
                ..OllamaModelDetails::default()
            },
        }];

        let json = serde_json::to_value(&models).unwrap();

        assert_eq!(json[0]["name"], "qwen2.5:7b");
        assert_eq!(json[0]["model"], "qwen2.5:7b");
        assert_eq!(json[0]["size_bytes"], 4683072000_u64);
        assert_eq!(json[0]["digest"], "sha256:abc");
        assert_eq!(json[0]["details"]["family"], "qwen2");
        assert_eq!(json[0]["details"]["quantization_level"], "Q4_K_M");

        assert_eq!(
            serde_json::to_string(&Vec::<InstalledOllamaModel>::new()).unwrap(),
            "[]"
        );
    }

    #[test]
    fn formats_empty_inventory() {
        assert_eq!(format_inventory(&[]), "No Ollama models installed.\n");
    }

    #[test]
    fn formats_model_metadata() {
        let models = vec![InstalledOllamaModel {
            name: "qwen2.5:7b".to_owned(),
            model: Some("qwen2.5:7b".to_owned()),
            modified_at: Some("2026-10-30T10:20:30Z".to_owned()),
            size_bytes: 4683072000,
            digest: Some("sha256:abc".to_owned()),
            details: OllamaModelDetails {
                format: Some("gguf".to_owned()),
                family: Some("qwen2".to_owned()),
                parameter_size: Some("7.6B".to_owned()),
                quantization_level: Some("Q4_K_M".to_owned()),
                ..OllamaModelDetails::default()
            },
        }];

        let output = format_inventory(&models);
        assert!(output.contains("Model: qwen2.5:7b"));
        assert!(output.contains("Size: 4683072000 bytes"));
        assert!(output.contains("Digest: sha256:abc"));
        assert!(output.contains("Family: qwen2"));
        assert!(output.contains("Quantization: Q4_K_M"));
    }
}
