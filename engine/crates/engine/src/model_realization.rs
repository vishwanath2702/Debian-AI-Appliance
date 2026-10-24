//! Model realization contracts for inference engines.

use std::{io, path::Path, process::Command};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use tempfile::Builder;

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
    /// Prepares an engine-created temporary Modelfile for execution.
    ///
    /// The default implementation preserves existing runner behavior.
    fn prepare_modelfile(
        &mut self,
        _directory: &tempfile::TempDir,
        _modelfile: &Path,
    ) -> io::Result<()> {
        Ok(())
    }

    /// Executes an Ollama command.
    fn status(&mut self, command: &mut Command) -> io::Result<()>;
}

/// Runs Ollama commands as operating-system processes.
#[derive(Clone, Copy, Debug, Default)]
pub struct ProcessOllamaCommandRunner;

impl OllamaCommandRunner for ProcessOllamaCommandRunner {
    fn status(&mut self, command: &mut Command) -> io::Result<()> {
        let output = command.output()?;

        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let detail = stderr.trim();

            if detail.is_empty() {
                Err(io::Error::other(format!(
                    "Ollama command exited unsuccessfully: {}",
                    output.status
                )))
            } else {
                Err(io::Error::other(format!(
                    "Ollama command exited unsuccessfully: {}: {detail}",
                    output.status
                )))
            }
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

        let temporary_directory = Builder::new()
            .permissions(std::fs::Permissions::from_mode(0o700))
            .tempdir_in("/tmp")?;
        let modelfile_path = temporary_directory.path().join("Modelfile");

        std::fs::write(&modelfile_path, ollama_modelfile(realization))?;

        // The temporary directory must remain at its original location
        // before ownership is handed to the Ollama service account.
        let directory_metadata = std::fs::symlink_metadata(temporary_directory.path())?;
        if !directory_metadata.file_type().is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "Ollama temporary directory was replaced",
            ));
        }

        self.runner
            .prepare_modelfile(&temporary_directory, &modelfile_path)?;

        let mut command = ollama_create_command(realization, &modelfile_path);
        self.runner.status(&mut command)
    }
}

#[allow(dead_code)]
fn validate_ollama_create_command(command: &std::process::Command) -> Result<(), &'static str> {
    use std::ffi::OsStr;

    if command.get_program() != OsStr::new("ollama") {
        return Err("Unexpected Ollama executable");
    }

    let arguments: Vec<_> = command.get_args().collect();

    if arguments.len() != 4
        || arguments[0] != OsStr::new("create")
        || arguments[1] != OsStr::new("-f")
        || arguments[2].is_empty()
        || arguments[3].is_empty()
        || arguments[3].to_string_lossy().starts_with('-')
    {
        return Err("Unexpected Ollama command arguments");
    }

    Ok(())
}

#[allow(dead_code)]
fn restricted_ollama_command(original: &std::process::Command) -> std::process::Command {
    let mut command = std::process::Command::new("/usr/local/bin/ollama");

    command.args(original.get_args());
    command.env_clear();
    command.env("PATH", "/usr/local/bin:/usr/bin:/bin");
    command.env("HOME", "/var/lib/ollama");
    command.env("OLLAMA_MODELS", "/var/lib/ollama/models");
    command.env("OLLAMA_HOST", "127.0.0.1:11434");

    command
}

// Resolve the Ollama service account rather than assuming a fixed UID/GID.
fn ollama_service_identity() -> std::io::Result<(u32, u32)> {
    let passwd = std::fs::read_to_string("/etc/passwd")?;
    parse_ollama_service_identity(&passwd)
}

fn parse_ollama_service_identity(passwd: &str) -> std::io::Result<(u32, u32)> {
    use std::io::{Error, ErrorKind};

    let identities: Vec<_> = passwd
        .lines()
        .filter_map(|line| {
            let fields: Vec<_> = line.split(':').collect();

            if fields.len() != 7 || fields[0] != "ollama" {
                return None;
            }

            Some((fields[2], fields[3]))
        })
        .collect();

    let [(uid, gid)] = identities.as_slice() else {
        return Err(Error::new(
            ErrorKind::NotFound,
            "Expected exactly one Ollama service account",
        ));
    };

    let uid = uid
        .parse::<u32>()
        .map_err(|_| Error::new(ErrorKind::InvalidData, "Invalid Ollama UID"))?;

    let gid = gid
        .parse::<u32>()
        .map_err(|_| Error::new(ErrorKind::InvalidData, "Invalid Ollama GID"))?;

    if uid == 0 || gid == 0 {
        return Err(Error::new(
            ErrorKind::PermissionDenied,
            "Ollama service account must not use root UID or GID",
        ));
    }

    Ok((uid, gid))
}

fn restricted_ollama_service_command(
    command: &std::process::Command,
    uid: u32,
    gid: u32,
) -> std::process::Command {
    let original = restricted_ollama_command(command);
    let mut restricted = std::process::Command::new("/usr/bin/setpriv");

    restricted
        .arg("--reuid")
        .arg(uid.to_string())
        .arg("--regid")
        .arg(gid.to_string())
        .arg("--clear-groups")
        .arg("--no-new-privs")
        .arg(original.get_program())
        .args(original.get_args());

    restricted.env_clear();
    restricted.env("PATH", "/usr/local/bin:/usr/bin:/bin");
    restricted.env("HOME", "/var/lib/ollama");
    restricted.env("OLLAMA_MODELS", "/var/lib/ollama/models");
    restricted.env("OLLAMA_HOST", "127.0.0.1:11434");

    restricted
}

fn open_private_ollama_modelfile(
    directory: &std::path::Path,
    modelfile: &std::path::Path,
) -> std::io::Result<(std::os::fd::OwnedFd, std::os::fd::OwnedFd)> {
    use rustix::fs::{Mode, OFlags, fstat, openat};
    use std::io::{Error, ErrorKind};

    if modelfile.parent() != Some(directory)
        || modelfile.file_name() != Some(std::ffi::OsStr::new("Modelfile"))
    {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            "Unexpected temporary Ollama Modelfile path",
        ));
    }

    let directory_fd = openat(
        rustix::fs::CWD,
        directory,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )?;

    let directory_stat = fstat(&directory_fd)?;

    if directory_stat.st_uid != 0 || directory_stat.st_mode & 0o777 != 0o700 {
        return Err(Error::new(
            ErrorKind::PermissionDenied,
            "Temporary Ollama directory must be root-owned and private",
        ));
    }

    let file_fd = openat(
        &directory_fd,
        "Modelfile",
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )?;

    let file_stat = fstat(&file_fd)?;

    if file_stat.st_uid != 0
        || file_stat.st_mode & 0o170000 != 0o100000
        || file_stat.st_nlink != 1
        || file_stat.st_mode & 0o022 != 0
    {
        return Err(Error::new(
            ErrorKind::PermissionDenied,
            "Temporary Ollama Modelfile has unsafe ownership or permissions",
        ));
    }

    Ok((directory_fd, file_fd))
}

#[derive(Default)]
pub struct RestrictedOllamaRunner {
    prepared_modelfile: Option<std::path::PathBuf>,
}

impl OllamaCommandRunner for RestrictedOllamaRunner {
    fn prepare_modelfile(
        &mut self,
        directory: &tempfile::TempDir,
        modelfile: &std::path::Path,
    ) -> std::io::Result<()> {
        use rustix::fs::{Mode, fchmod, fchown};
        use rustix::process::{Gid, Uid};

        let (_, gid) = ollama_service_identity()?;
        let (directory_fd, file_fd) = open_private_ollama_modelfile(directory.path(), modelfile)?;

        let group = Some(Gid::from_raw(gid));

        // Keep both objects root-owned. Ollama may traverse the directory
        // and read the Modelfile, but may modify neither.
        fchown(&file_fd, Some(Uid::from_raw(0)), group)?;
        fchmod(&file_fd, Mode::from_raw_mode(0o640))?;
        fchown(&directory_fd, Some(Uid::from_raw(0)), group)?;
        fchmod(&directory_fd, Mode::from_raw_mode(0o710))?;

        self.prepared_modelfile = Some(modelfile.to_path_buf());

        Ok(())
    }

    fn status(&mut self, command: &mut std::process::Command) -> std::io::Result<()> {
        validate_ollama_create_command(command)
            .map_err(|message| std::io::Error::new(std::io::ErrorKind::InvalidInput, message))?;

        let prepared = self.prepared_modelfile.take().ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "Ollama Modelfile was not prepared",
            )
        })?;

        let arguments: Vec<_> = command.get_args().collect();

        if std::path::Path::new(arguments[2]) != prepared {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "Ollama command does not reference the prepared Modelfile",
            ));
        }

        let (uid, gid) = ollama_service_identity()?;

        let mut restricted = restricted_ollama_service_command(command, uid, gid);

        OllamaCommandRunner::status(&mut ProcessOllamaCommandRunner, &mut restricted)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn ollama_identity_resolves_only_valid_named_service_account() {
        let parse = super::parse_ollama_service_identity;

        let valid = "root:x:0:0:root:/root:/bin/bash\\n\\
ollama:x:989:989::/var/lib/ollama:/usr/sbin/nologin\\n";

        assert_eq!(parse(valid).unwrap(), (989, 989));

        for invalid in [
            "root:x:0:0:root:/root:/bin/bash\\n",
            "ollama:x:0:989::/var/lib/ollama:/usr/sbin/nologin\\n",
            "ollama:x:989:0::/var/lib/ollama:/usr/sbin/nologin\\n",
            "ollama:x:invalid:989::/var/lib/ollama:/usr/sbin/nologin\\n",
            "ollama:x:989:989::/var/lib/ollama:/usr/sbin/nologin\\n\\
ollama:x:990:990::/var/lib/ollama:/usr/sbin/nologin\\n",
        ] {
            assert!(parse(invalid).is_err(), "{invalid:?}");
        }
    }

    #[test]
    fn private_ollama_modelfile_rejects_unexpected_path() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let unexpected = directory.path().join("unexpected");

        let error = super::open_private_ollama_modelfile(directory.path(), &unexpected)
            .expect_err("unexpected Modelfile path must be rejected");

        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
    }

    #[cfg(unix)]
    #[test]
    fn private_ollama_modelfile_rejects_symlink() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().expect("temporary directory");
        let target = directory.path().join("target");
        let modelfile = directory.path().join("Modelfile");

        std::fs::write(&target, "FROM /tmp/example.gguf\\n").expect("write target");
        symlink(&target, &modelfile).expect("create symlink");

        assert!(
            super::open_private_ollama_modelfile(directory.path(), &modelfile,).is_err(),
            "symbolic-link Modelfile must be rejected"
        );
    }

    #[cfg(unix)]
    #[test]
    fn private_ollama_modelfile_rejects_permissive_directory() {
        use std::os::unix::fs::PermissionsExt;

        let directory = tempfile::tempdir().expect("temporary directory");
        let modelfile = directory.path().join("Modelfile");

        std::fs::write(&modelfile, "FROM /tmp/example.gguf\\n").expect("write Modelfile");

        std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o777))
            .expect("set permissive directory permissions");

        assert!(
            super::open_private_ollama_modelfile(directory.path(), &modelfile,).is_err(),
            "permissive temporary directory must be rejected"
        );
    }

    #[cfg(unix)]
    #[test]
    fn private_ollama_modelfile_rejects_hard_link() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let modelfile = directory.path().join("Modelfile");
        let second_link = directory.path().join("second-link");

        std::fs::write(&modelfile, "FROM /tmp/example.gguf\\n").expect("write Modelfile");

        std::fs::hard_link(&modelfile, &second_link).expect("create hard link");

        assert!(
            super::open_private_ollama_modelfile(directory.path(), &modelfile,).is_err(),
            "hard-linked Modelfile must be rejected"
        );
    }

    #[test]
    #[ignore = "requires root and the local Ollama service account"]
    fn restricted_runner_prepares_readonly_modelfile_for_ollama() {
        use super::OllamaCommandRunner;
        use std::os::unix::fs::{MetadataExt, PermissionsExt};

        assert_eq!(rustix::process::geteuid().as_raw(), 0);

        let (uid, gid) = super::ollama_service_identity().expect("resolve Ollama identity");

        let directory = tempfile::Builder::new()
            .permissions(std::fs::Permissions::from_mode(0o700))
            .tempdir_in("/tmp")
            .expect("create private temporary directory");

        let modelfile = directory.path().join("Modelfile");
        let content = "FROM \"/tmp/example.gguf\"\n";

        std::fs::write(&modelfile, content).expect("write Modelfile");

        let mut runner = super::RestrictedOllamaRunner::default();

        runner
            .prepare_modelfile(&directory, &modelfile)
            .expect("prepare Modelfile");

        let directory_metadata = std::fs::metadata(directory.path()).expect("directory metadata");
        let file_metadata = std::fs::metadata(&modelfile).expect("Modelfile metadata");

        assert_eq!(directory_metadata.uid(), 0);
        assert_eq!(directory_metadata.gid(), gid);
        assert_eq!(directory_metadata.permissions().mode() & 0o777, 0o710);

        assert_eq!(file_metadata.uid(), 0);
        assert_eq!(file_metadata.gid(), gid);
        assert_eq!(file_metadata.permissions().mode() & 0o777, 0o640);

        assert_eq!(
            runner.prepared_modelfile.as_deref(),
            Some(modelfile.as_path())
        );

        let output = std::process::Command::new("/usr/bin/setpriv")
            .args([
                "--reuid",
                &uid.to_string(),
                "--regid",
                &gid.to_string(),
                "--clear-groups",
                "--no-new-privs",
                "/usr/bin/cat",
            ])
            .arg(&modelfile)
            .output()
            .expect("execute service-account read");

        assert!(
            output.status.success(),
            "Ollama read failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        assert_eq!(output.stdout, content.as_bytes());

        let temporary_path = directory.path().to_path_buf();
        drop(directory);

        assert!(
            !temporary_path.exists(),
            "temporary Modelfile directory must be removed"
        );
    }

    #[test]
    fn restricted_runner_rejects_different_prepared_modelfile() {
        use super::OllamaCommandRunner;

        let mut runner = super::RestrictedOllamaRunner {
            prepared_modelfile: Some(std::path::PathBuf::from("/tmp/prepared/Modelfile")),
        };

        let mut command = std::process::Command::new("ollama");
        command.args(["create", "-f", "/tmp/different/Modelfile", "qwen2.5-local"]);

        let error = runner
            .status(&mut command)
            .expect_err("different Modelfile must not execute");

        assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
        assert!(
            error
                .to_string()
                .contains("does not reference the prepared Modelfile"),
            "{error}"
        );
        assert!(
            runner.prepared_modelfile.is_none(),
            "prepared state must be consumed even on mismatch"
        );
    }

    #[test]
    fn restricted_runner_rejects_unprepared_modelfile() {
        use super::OllamaCommandRunner;

        let mut runner = super::RestrictedOllamaRunner::default();
        let mut command = std::process::Command::new("ollama");

        command.args(["create", "-f", "/tmp/unprepared/Modelfile", "qwen2.5-local"]);

        let error = runner
            .status(&mut command)
            .expect_err("unprepared Modelfile must not execute");

        assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
    }

    #[test]
    fn restricted_ollama_service_command_drops_privileges() {
        let mut original = std::process::Command::new("ollama");
        original.args(["create", "-f", "/tmp/example/Modelfile", "qwen2.5-local"]);

        let restricted = super::restricted_ollama_service_command(&original, 997, 986);

        assert_eq!(
            restricted.get_program(),
            std::ffi::OsStr::new("/usr/bin/setpriv")
        );

        let arguments: Vec<_> = restricted
            .get_args()
            .map(|argument| argument.to_string_lossy().into_owned())
            .collect();

        assert_eq!(
            arguments,
            vec![
                "--reuid",
                "997",
                "--regid",
                "986",
                "--clear-groups",
                "--no-new-privs",
                "/usr/local/bin/ollama",
                "create",
                "-f",
                "/tmp/example/Modelfile",
                "qwen2.5-local",
            ]
        );
    }

    #[test]
    fn restricted_ollama_command_uses_fixed_executable() {
        use std::ffi::OsStr;
        use std::process::Command;

        let mut original = Command::new("ollama");
        original.args(["create", "-f", "/tmp/Modelfile", "test-model"]);

        let restricted = super::restricted_ollama_command(&original);

        assert_eq!(
            restricted.get_program(),
            OsStr::new("/usr/local/bin/ollama")
        );
        assert_eq!(
            restricted.get_args().collect::<Vec<_>>(),
            original.get_args().collect::<Vec<_>>()
        );

        let environment = restricted.get_envs().collect::<Vec<_>>();

        assert_eq!(environment.len(), 4);
        assert!(environment.iter().any(|(key, value)| {
            *key == "HOME" && *value == Some(OsStr::new("/var/lib/ollama"))
        }));
        assert!(environment.iter().any(|(key, value)| {
            *key == "OLLAMA_MODELS" && *value == Some(OsStr::new("/var/lib/ollama/models"))
        }));
        assert!(environment.iter().any(|(key, value)| {
            *key == "PATH" && *value == Some(OsStr::new("/usr/local/bin:/usr/bin:/bin"))
        }));
        assert!(environment.iter().any(|(key, value)| {
            *key == "OLLAMA_HOST" && *value == Some(OsStr::new("127.0.0.1:11434"))
        }));
    }

    #[test]
    fn restricted_runner_rejects_unexpected_commands() {
        use std::process::Command;

        let mut valid = Command::new("ollama");
        valid.args(["create", "-f", "/tmp/Modelfile", "test-model"]);
        assert!(super::validate_ollama_create_command(&valid).is_ok());

        for arguments in [
            vec!["rm", "test-model"],
            vec!["pull", "test-model"],
            vec!["create", "-f", "/tmp/Modelfile", "-unsafe"],
            vec!["create", "-f", "/tmp/Modelfile"],
        ] {
            let mut command = Command::new("ollama");
            command.args(arguments);
            assert!(super::validate_ollama_create_command(&command).is_err());
        }

        let mut unexpected = Command::new("/bin/sh");
        unexpected.args(["-c", "true"]);
        assert!(super::validate_ollama_create_command(&unexpected).is_err());
    }

    #[test]
    fn restricted_runner_refuses_unexpected_command_without_execution() {
        use super::OllamaCommandRunner;
        use std::io::ErrorKind;
        use std::process::Command;

        let mut runner = super::RestrictedOllamaRunner::default();
        let mut command = Command::new("/bin/sh");
        command.args(["-c", "echo should-not-run"]);

        let error = runner
            .status(&mut command)
            .expect_err("unexpected executable must be rejected");

        assert_eq!(error.kind(), ErrorKind::InvalidInput);
    }

    use super::{ModelRealizationExecutor, OllamaCommandRunner, OllamaModelRealizationExecutor};
    use model::{
        ExternalContentItemId, ImportedContentItem, InferenceEngineId, ModelRealization,
        ModelRealizationId,
    };
    use std::{io, process::Command};

    #[cfg(unix)]
    #[test]
    fn ollama_modelfile_directory_is_private() {
        use std::os::unix::fs::PermissionsExt;

        let directory = tempfile::Builder::new()
            .permissions(std::fs::Permissions::from_mode(0o700))
            .tempdir()
            .expect("temporary directory");
        let permissions = std::fs::metadata(directory.path())
            .expect("directory metadata")
            .permissions();

        assert_eq!(
            permissions.mode() & 0o777,
            0o700,
            "Ollama temporary directory must remain private"
        );
    }

    #[cfg(unix)]
    #[test]
    fn process_ollama_runner_preserves_failure_stderr() {
        use std::os::unix::fs::PermissionsExt;

        let directory = tempfile::tempdir().expect("temporary directory");
        let script = directory.path().join("failing-ollama");

        std::fs::write(
            &script,
            "#!/bin/sh\nprintf 'model registration failed\n' >&2\nexit 17\n",
        )
        .expect("write failing command");

        let mut permissions = std::fs::metadata(&script)
            .expect("script metadata")
            .permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(&script, permissions).expect("make script executable");

        let mut command = std::process::Command::new(&script);
        let mut runner = super::ProcessOllamaCommandRunner;

        let error = runner
            .status(&mut command)
            .expect_err("failing command must return an error");

        let message = error.to_string();

        assert!(message.contains("exit status: 17"), "{message}");
        assert!(message.contains("model registration failed"), "{message}");
    }

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
    fn ollama_executor_cleans_up_temporary_modelfile() {
        use model::{
            ExternalContentItemId, ImportedContentItem, InferenceEngineId, ModelRealization,
            ModelRealizationId,
        };
        use std::path::PathBuf;

        struct RecordingRunner {
            temporary_directory: Option<PathBuf>,
        }

        impl OllamaCommandRunner for RecordingRunner {
            fn prepare_modelfile(
                &mut self,
                directory: &tempfile::TempDir,
                modelfile: &std::path::Path,
            ) -> io::Result<()> {
                assert!(modelfile.is_file());
                self.temporary_directory = Some(directory.path().to_path_buf());
                Ok(())
            }

            fn status(&mut self, _command: &mut std::process::Command) -> io::Result<()> {
                Ok(())
            }
        }

        let realization = ModelRealization::new(
            ModelRealizationId::new("cleanup-test"),
            InferenceEngineId::ollama(),
            ImportedContentItem::new(
                ExternalContentItemId::new("cleanup-source"),
                "/tmp/cleanup-test.gguf",
            ),
        );

        let mut executor = OllamaModelRealizationExecutor::new(RecordingRunner {
            temporary_directory: None,
        });

        executor
            .execute(&realization)
            .expect("realization succeeds");

        let directory = executor
            .runner
            .temporary_directory
            .as_ref()
            .expect("runner recorded temporary directory");

        assert!(
            !directory.exists(),
            "temporary Modelfile directory must be removed after execution"
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
