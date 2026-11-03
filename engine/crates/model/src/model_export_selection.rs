//! Untrusted resource selections for DAIA Model Export.
//!
//! These values describe user intent only. They do not authorize filesystem
//! access, establish resource identity, or permit export execution.

use std::path::{Component, Path};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ModelExportSourceSelection {
    Gguf {
        directory: String,
        relative_file: String,
    },
    Ollama {
        directory: String,
        relative_manifest: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelExportDestinationSelection {
    directory: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelExportResourceSelection {
    source: ModelExportSourceSelection,
    destination: ModelExportDestinationSelection,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModelExportSelectionError {
    InvalidDirectory,
    InvalidRelativePath,
}

impl std::fmt::Display for ModelExportSelectionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for ModelExportSelectionError {}

fn valid_directory(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 4096
        && !value.chars().any(char::is_control)
        && !value.contains('\\')
        && value.starts_with('/')
        && value != "/"
        && value[1..]
            .split('/')
            .all(|component| !component.is_empty() && component != "." && component != "..")
        && Path::new(value).is_absolute()
        && Path::new(value)
            .components()
            .all(|component| matches!(component, Component::RootDir | Component::Normal(_)))
}

fn valid_relative_file(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 4096
        && !value.chars().any(char::is_control)
        && !value.contains('\\')
        && !value
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == ".." || part.starts_with('-'))
        && Path::new(value)
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

impl ModelExportSourceSelection {
    /// Describe a selected GGUF file without opening it.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid directory or relative-file syntax.
    pub fn gguf(
        directory: impl Into<String>,
        relative_file: impl Into<String>,
    ) -> Result<Self, ModelExportSelectionError> {
        let directory = directory.into();
        let relative_file = relative_file.into();

        if !valid_directory(&directory) {
            return Err(ModelExportSelectionError::InvalidDirectory);
        }
        if !valid_relative_file(&relative_file)
            || !relative_file.to_ascii_lowercase().ends_with(".gguf")
        {
            return Err(ModelExportSelectionError::InvalidRelativePath);
        }

        Ok(Self::Gguf {
            directory,
            relative_file,
        })
    }

    /// Describe a selected Ollama manifest relative to its manifests root.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid directory or relative-manifest syntax.
    pub fn ollama(
        directory: impl Into<String>,
        relative_manifest: impl Into<String>,
    ) -> Result<Self, ModelExportSelectionError> {
        let directory = directory.into();
        let relative_manifest = relative_manifest.into();

        if !valid_directory(&directory) {
            return Err(ModelExportSelectionError::InvalidDirectory);
        }
        if !valid_relative_file(&relative_manifest) || relative_manifest.split('/').count() < 4 {
            return Err(ModelExportSelectionError::InvalidRelativePath);
        }

        Ok(Self::Ollama {
            directory,
            relative_manifest,
        })
    }
}

impl ModelExportDestinationSelection {
    /// Describe a destination directory without authorizing writes.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid directory syntax.
    pub fn new(directory: impl Into<String>) -> Result<Self, ModelExportSelectionError> {
        let directory = directory.into();
        if !valid_directory(&directory) {
            return Err(ModelExportSelectionError::InvalidDirectory);
        }
        Ok(Self { directory })
    }

    #[must_use]
    pub fn directory(&self) -> &str {
        &self.directory
    }
}

impl ModelExportResourceSelection {
    #[must_use]
    pub fn new(
        source: ModelExportSourceSelection,
        destination: ModelExportDestinationSelection,
    ) -> Self {
        Self {
            source,
            destination,
        }
    }

    #[must_use]
    pub const fn source(&self) -> &ModelExportSourceSelection {
        &self.source
    }

    #[must_use]
    pub const fn destination(&self) -> &ModelExportDestinationSelection {
        &self.destination
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_gguf_and_ollama_selections() {
        let gguf = ModelExportSourceSelection::gguf("/media/models", "files/example.gguf").unwrap();
        let ollama = ModelExportSourceSelection::ollama(
            "/var/lib/ollama/models",
            "registry.ollama.ai/library/qwen/latest",
        )
        .unwrap();
        let destination = ModelExportDestinationSelection::new("/media/export").unwrap();

        let request = ModelExportResourceSelection::new(ollama.clone(), destination);
        assert_eq!(request.source(), &ollama);
        assert_eq!(request.destination().directory(), "/media/export");
        assert!(matches!(gguf, ModelExportSourceSelection::Gguf { .. }));
    }

    #[test]
    fn rejects_unsafe_directories() {
        for directory in [
            "",
            "relative/path",
            "/tmp/../escape",
            "/tmp/./folder",
            "/tmp//folder",
            "/tmp\nfolder",
        ] {
            assert!(
                ModelExportDestinationSelection::new(directory).is_err(),
                "accepted destination: {directory:?}"
            );
            assert!(
                ModelExportSourceSelection::ollama(directory, "library/model/latest").is_err(),
                "accepted source: {directory:?}"
            );
        }
    }

    #[test]
    fn rejects_unsafe_relative_files() {
        for file in [
            "",
            "../model.gguf",
            "models/../model.gguf",
            "/absolute.gguf",
            "models//model.gguf",
            "models/./model.gguf",
            "models\\model.gguf",
            "-model.gguf",
            "models/\nmodel.gguf",
        ] {
            assert!(
                ModelExportSourceSelection::gguf("/media/models", file).is_err(),
                "accepted GGUF path: {file:?}"
            );
        }
    }

    #[test]
    fn rejects_trailing_directory_separator_and_backslash() {
        for directory in ["/media/models/", "/media\\models", "/media/models//"] {
            assert!(
                ModelExportDestinationSelection::new(directory).is_err(),
                "accepted destination: {directory:?}"
            );
            assert!(
                ModelExportSourceSelection::gguf(directory, "example.gguf").is_err(),
                "accepted source: {directory:?}"
            );
        }
    }

    #[test]
    fn ollama_manifest_requires_registry_namespace_model_and_tag() {
        for manifest in [
            "latest",
            "library/model/latest",
            "registry.ollama.ai/library/model",
        ] {
            assert!(
                ModelExportSourceSelection::ollama("/var/lib/ollama/models", manifest).is_err(),
                "accepted incomplete manifest: {manifest:?}"
            );
        }

        assert!(
            ModelExportSourceSelection::ollama(
                "/var/lib/ollama/models",
                "custom.example.org/team/model/q4_k_m"
            )
            .is_ok()
        );
    }

    #[test]
    fn rejects_non_gguf_file_selection() {
        assert!(ModelExportSourceSelection::gguf("/media/models", "model.bin").is_err());
    }

    #[test]
    fn rejects_unsafe_ollama_manifest_selection() {
        for manifest in [
            "",
            "../latest",
            "/absolute/latest",
            "library/../latest",
            "library//latest",
            "library\\latest",
        ] {
            assert!(
                ModelExportSourceSelection::ollama("/var/lib/ollama/models", manifest).is_err()
            );
        }
    }
}
