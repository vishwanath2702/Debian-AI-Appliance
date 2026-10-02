//! Model realization contracts for inference engines.

use std::{io, path::Path, process::Command};

use tempfile::TempDir;

use model::{InferenceEngineId, ModelRealization};

/// Executes a prepared model realization.
pub trait ModelRealizationExecutor {
    /// Executor-specific failure.
    type Error;

    /// Realizes one imported model for its target inference engine.
    fn execute(&mut self, realization: &ModelRealization) -> Result<(), Self::Error>;
}

/// Runs commands required by the Ollama model-realization adapter.
pub trait OllamaCommandRunner {
    /// Executes an Ollama command.
    fn status(&mut self, command: &mut Command) -> io::Result<()>;
}

/// Runs Ollama commands as operating-system processes.
#[derive(Clone, Copy, Debug, Default)]
pub struct ProcessOllamaCommandRunner;

impl OllamaCommandRunner for ProcessOllamaCommandRunner {
    fn status(&mut self, command: &mut Command) -> io::Result<()> {
        let status = command.status()?;

        if status.success() {
            Ok(())
        } else {
            Err(io::Error::other(format!(
                "Ollama command exited unsuccessfully: {status}"
            )))
        }
    }
}

/// Generates the minimal Ollama Modelfile for an imported GGUF artifact.
#[must_use]
pub fn ollama_modelfile(realization: &ModelRealization) -> String {
    format!("FROM \"{}\"\n", realization.content().path().display())
}

/// Constructs the Ollama command that realizes a model from a Modelfile.
#[must_use]
pub fn ollama_create_command(realization: &ModelRealization, modelfile_path: &Path) -> Command {
    let mut command = Command::new("ollama");

    command
        .arg("create")
        .arg("-f")
        .arg(modelfile_path)
        .arg(realization.id().as_str());

    command
}

/// Realizes imported GGUF models through Ollama.
pub struct OllamaModelRealizationExecutor<R> {
    runner: R,
}

impl<R> OllamaModelRealizationExecutor<R> {
    /// Creates an Ollama model-realization executor.
    #[must_use]
    pub const fn new(runner: R) -> Self {
        Self { runner }
    }
}

impl<R> ModelRealizationExecutor for OllamaModelRealizationExecutor<R>
where
    R: OllamaCommandRunner,
{
    type Error = io::Error;

    fn execute(&mut self, realization: &ModelRealization) -> Result<(), Self::Error> {
        if realization.engine() != &InferenceEngineId::ollama() {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                format!(
                    "Ollama executor cannot realize model for inference engine {}",
                    realization.engine()
                ),
            ));
        }

        let content_path = realization.content().path().to_string_lossy();

        if content_path.contains(['"', '\n', '\r']) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "Ollama model path contains unsupported Modelfile characters: {}",
                    realization.content().path().display()
                ),
            ));
        }

        let temporary_directory = TempDir::new()?;
        let modelfile_path = temporary_directory.path().join("Modelfile");

        std::fs::write(&modelfile_path, ollama_modelfile(realization))?;

        let mut command = ollama_create_command(realization, &modelfile_path);
        self.runner.status(&mut command)
    }
}

impl Default for OllamaModelRealizationExecutor<ProcessOllamaCommandRunner> {
    fn default() -> Self {
        Self::new(ProcessOllamaCommandRunner)
    }
}

#[cfg(test)]
mod tests {
    use super::{ModelRealizationExecutor, OllamaCommandRunner, OllamaModelRealizationExecutor};
    use model::{
        ExternalContentItemId, ImportedContentItem, InferenceEngineId, ModelRealization,
        ModelRealizationId,
    };
    use std::{io, process::Command};

    #[derive(Default)]
    struct RecordingOllamaCommandRunner {
        command: Option<Vec<String>>,
        modelfile: Option<String>,
    }

    impl OllamaCommandRunner for RecordingOllamaCommandRunner {
        fn status(&mut self, command: &mut Command) -> io::Result<()> {
            self.command = Some(
                std::iter::once(command.get_program())
                    .chain(command.get_args())
                    .map(|argument| argument.to_string_lossy().into_owned())
                    .collect(),
            );

            let args = command.get_args().collect::<Vec<_>>();
            let modelfile_path = args
                .get(2)
                .ok_or_else(|| io::Error::other("Ollama Modelfile argument is missing"))?;

            self.modelfile = Some(std::fs::read_to_string(std::path::Path::new(
                modelfile_path,
            ))?);

            Ok(())
        }
    }

    #[test]
    fn ollama_executor_realizes_imported_gguf() {
        let realization = ModelRealization::new(
            ModelRealizationId::new("model"),
            InferenceEngineId::ollama(),
            ImportedContentItem::new(
                ExternalContentItemId::new("source-model"),
                "/var/lib/daia/content/model.gguf",
            ),
        );

        let runner = RecordingOllamaCommandRunner::default();
        let mut executor = OllamaModelRealizationExecutor::new(runner);

        executor
            .execute(&realization)
            .expect("Ollama realization should succeed");

        assert_eq!(
            executor.runner.modelfile.as_deref(),
            Some("FROM \"/var/lib/daia/content/model.gguf\"\n")
        );

        let command = executor
            .runner
            .command
            .as_ref()
            .expect("Ollama command should be recorded");

        assert_eq!(command[0], "ollama");
        assert_eq!(command[1], "create");
        assert_eq!(command[2], "-f");
        assert_eq!(command[4], "model");
    }

    #[test]
    fn ollama_executor_can_reconcile_same_realization_repeatedly() {
        let realization = ModelRealization::new(
            ModelRealizationId::new("model"),
            InferenceEngineId::ollama(),
            ImportedContentItem::new(
                ExternalContentItemId::new("source-model"),
                "/var/lib/daia/content/model.gguf",
            ),
        );

        let runner = RecordingOllamaCommandRunner::default();
        let mut executor = OllamaModelRealizationExecutor::new(runner);

        executor
            .execute(&realization)
            .expect("first Ollama realization should succeed");

        let first_command = executor
            .runner
            .command
            .clone()
            .expect("first Ollama command should be recorded");

        executor
            .execute(&realization)
            .expect("repeated Ollama realization should succeed");

        let second_command = executor
            .runner
            .command
            .clone()
            .expect("repeated Ollama command should be recorded");

        assert_eq!(first_command[0], "ollama");
        assert_eq!(first_command[1], "create");
        assert_eq!(first_command[2], "-f");
        assert_eq!(first_command[4], "model");

        assert_eq!(second_command[0], "ollama");
        assert_eq!(second_command[1], "create");
        assert_eq!(second_command[2], "-f");
        assert_eq!(second_command[4], "model");

        assert_eq!(
            executor.runner.modelfile.as_deref(),
            Some("FROM \"/var/lib/daia/content/model.gguf\"\n")
        );
    }

    #[test]
    fn ollama_executor_rejects_path_with_double_quote() {
        let realization = ModelRealization::new(
            ModelRealizationId::new("model"),
            InferenceEngineId::ollama(),
            ImportedContentItem::new(
                ExternalContentItemId::new("source-model"),
                "/var/lib/daia/content/model\"quoted.gguf",
            ),
        );

        let runner = RecordingOllamaCommandRunner::default();
        let mut executor = OllamaModelRealizationExecutor::new(runner);

        let error = executor
            .execute(&realization)
            .expect_err("quoted GGUF path should be rejected");

        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
        assert!(executor.runner.command.is_none());
    }

    #[test]
    fn ollama_executor_rejects_path_with_newline() {
        let realization = ModelRealization::new(
            ModelRealizationId::new("model"),
            InferenceEngineId::ollama(),
            ImportedContentItem::new(
                ExternalContentItemId::new("source-model"),
                "/var/lib/daia/content/model\nnewline.gguf",
            ),
        );

        let runner = RecordingOllamaCommandRunner::default();
        let mut executor = OllamaModelRealizationExecutor::new(runner);

        let error = executor
            .execute(&realization)
            .expect_err("newline in GGUF path should be rejected");

        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
        assert!(executor.runner.command.is_none());
    }

    #[test]
    fn ollama_executor_rejects_other_inference_engines() {
        let realization = ModelRealization::new(
            ModelRealizationId::new("model"),
            InferenceEngineId::llama_cpp(),
            ImportedContentItem::new(
                ExternalContentItemId::new("source-model"),
                "/var/lib/daia/content/model.gguf",
            ),
        );

        let runner = RecordingOllamaCommandRunner::default();
        let mut executor = OllamaModelRealizationExecutor::new(runner);

        let error = executor
            .execute(&realization)
            .expect_err("non-Ollama realization should be rejected");

        assert_eq!(error.kind(), io::ErrorKind::Unsupported);
        assert!(executor.runner.command.is_none());
    }

    #[test]
    fn ollama_modelfile_references_imported_gguf() {
        let realization = ModelRealization::new(
            ModelRealizationId::new("model"),
            InferenceEngineId::ollama(),
            ImportedContentItem::new(
                ExternalContentItemId::new("source-model"),
                "/var/lib/daia/content/model.gguf",
            ),
        );

        assert_eq!(
            super::ollama_modelfile(&realization),
            "FROM \"/var/lib/daia/content/model.gguf\"\n"
        );
    }

    #[test]
    fn ollama_modelfile_quotes_gguf_path_with_spaces() {
        let realization = ModelRealization::new(
            ModelRealizationId::new("model"),
            InferenceEngineId::ollama(),
            ImportedContentItem::new(
                ExternalContentItemId::new("source-model"),
                "/var/lib/daia/content/My Model.gguf",
            ),
        );

        assert_eq!(
            super::ollama_modelfile(&realization),
            "FROM \"/var/lib/daia/content/My Model.gguf\"\n"
        );
    }

    #[test]
    fn ollama_create_command_uses_modelfile_and_realization_id() {
        let realization = ModelRealization::new(
            ModelRealizationId::new("model"),
            InferenceEngineId::ollama(),
            ImportedContentItem::new(
                ExternalContentItemId::new("source-model"),
                "/var/lib/daia/content/model.gguf",
            ),
        );

        let command =
            super::ollama_create_command(&realization, std::path::Path::new("/tmp/Modelfile"));

        assert_eq!(command.get_program(), "ollama");
        assert_eq!(
            command
                .get_args()
                .map(|argument| argument.to_string_lossy().into_owned())
                .collect::<Vec<_>>(),
            vec!["create", "-f", "/tmp/Modelfile", "model"]
        );
    }

    #[test]
    fn model_realization_exposes_contract() {
        let content = ImportedContentItem::new(
            ExternalContentItemId::new("model"),
            "/var/lib/daia/content/model.gguf",
        );

        let realization = ModelRealization::new(
            ModelRealizationId::new("model"),
            InferenceEngineId::ollama(),
            content.clone(),
        );

        assert_eq!(realization.id().as_str(), "model");
        assert_eq!(realization.engine(), &InferenceEngineId::ollama());
        assert_eq!(realization.content(), &content);
    }
}
