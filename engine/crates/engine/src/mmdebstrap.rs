//! mmdebstrap root filesystem bootstrapping.

use std::{
    error::Error,
    fmt::{self, Debug, Display, Formatter},
    io,
    process::{Command, ExitStatus},
    sync::Arc,
};

use crate::{BootstrapConfig, Bootstrapper, BuildContext};

const BOOTSTRAP_PACKAGES: &[&str] = &["ca-certificates", "gnupg", "grub-efi-amd64", "efibootmgr"];

trait CommandRunner: Send + Sync {
    fn status(&self, command: &mut Command) -> io::Result<ExitStatus>;
}

#[derive(Clone, Copy, Debug, Default)]
struct ProcessCommandRunner;

impl CommandRunner for ProcessCommandRunner {
    fn status(&self, command: &mut Command) -> io::Result<ExitStatus> {
        command.status()
    }
}

/// Error returned when mmdebstrap cannot create a root filesystem.
#[derive(Debug)]
pub enum MmdebstrapError {
    /// The mmdebstrap process could not be started or awaited.
    Process(io::Error),

    /// The mmdebstrap process exited unsuccessfully.
    Unsuccessful(ExitStatus),
}

impl Display for MmdebstrapError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Process(error) => write!(formatter, "failed to execute mmdebstrap: {error}"),
            Self::Unsuccessful(status) => {
                write!(formatter, "mmdebstrap exited unsuccessfully: {status}")
            }
        }
    }
}

impl Error for MmdebstrapError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Process(error) => Some(error),
            Self::Unsuccessful(_) => None,
        }
    }
}

/// Constructs commands for bootstrapping Debian root filesystems with mmdebstrap.
#[derive(Clone)]
pub struct MmdebstrapBootstrapper {
    runner: Arc<dyn CommandRunner>,
}

impl Debug for MmdebstrapBootstrapper {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MmdebstrapBootstrapper")
            .finish_non_exhaustive()
    }
}

impl Default for MmdebstrapBootstrapper {
    fn default() -> Self {
        Self::new()
    }
}

impl MmdebstrapBootstrapper {
    /// Creates an mmdebstrap bootstrapper.
    #[must_use]
    pub fn new() -> Self {
        Self {
            runner: Arc::new(ProcessCommandRunner),
        }
    }

    #[cfg(test)]
    fn with_runner(runner: impl CommandRunner + 'static) -> Self {
        Self {
            runner: Arc::new(runner),
        }
    }

    /// Constructs the mmdebstrap command for a target root and bootstrap configuration.
    #[must_use]
    pub fn command_for_root(&self, root: &std::path::Path, config: &BootstrapConfig) -> Command {
        let mut command = Command::new("mmdebstrap");

        command
            .arg("--mode=root")
            .arg(format!("--include={}", BOOTSTRAP_PACKAGES.join(",")))
            .arg(format!("--variant={}", config.variant()))
            .arg(format!("--architectures={}", config.architecture()))
            .arg(format!("--components={}", config.components().join(",")))
            .arg(config.release())
            .arg(root)
            .arg(config.mirror());

        command
    }

    /// Bootstraps a target root using the supplied bootstrap configuration.
    ///
    /// # Errors
    ///
    /// Returns [`MmdebstrapError`] if the command cannot be executed or exits
    /// unsuccessfully.
    pub fn bootstrap_root(
        &self,
        root: &std::path::Path,
        config: &BootstrapConfig,
    ) -> Result<(), MmdebstrapError> {
        let mut command = self.command_for_root(root, config);

        let status = self
            .runner
            .status(&mut command)
            .map_err(MmdebstrapError::Process)?;

        if status.success() {
            Ok(())
        } else {
            Err(MmdebstrapError::Unsuccessful(status))
        }
    }

    /// Constructs the mmdebstrap command for a build context.
    #[must_use]
    pub fn command(&self, context: &BuildContext) -> Command {
        self.command_for_root(context.rootfs(), context.bootstrap())
    }

    #[must_use]
    pub fn sudo_command(&self, context: &BuildContext) -> Command {
        let mut command = Command::new("sudo");

        command
            .arg("mmdebstrap")
            .arg("--mode=root")
            .arg(format!("--include={}", BOOTSTRAP_PACKAGES.join(",")))
            .arg(format!("--variant={}", context.bootstrap().variant()))
            .arg(format!(
                "--architectures={}",
                context.bootstrap().architecture()
            ))
            .arg(format!(
                "--components={}",
                context.bootstrap().components().join(",")
            ))
            .arg(context.bootstrap().release())
            .arg(context.rootfs())
            .arg(context.bootstrap().mirror());

        command
    }
}

impl Bootstrapper for MmdebstrapBootstrapper {
    type Error = MmdebstrapError;

    fn bootstrap(&self, context: &BuildContext) -> Result<(), Self::Error> {
        let mut command = self.command(context);

        let status = self
            .runner
            .status(&mut command)
            .map_err(MmdebstrapError::Process)?;

        if status.success() {
            return Ok(());
        }

        let mut sudo_command = self.sudo_command(context);

        let sudo_status = self
            .runner
            .status(&mut sudo_command)
            .map_err(MmdebstrapError::Process)?;

        if sudo_status.success() {
            restore_rootfs_ownership(context)?;
            Ok(())
        } else {
            Err(MmdebstrapError::Unsuccessful(sudo_status))
        }
    }
}
fn restore_rootfs_ownership(context: &BuildContext) -> Result<(), MmdebstrapError> {
    let user = std::env::var("USER").unwrap_or_else(|_| "root".to_owned());

    let status = Command::new("sudo")
        .arg("chown")
        .arg("-R")
        .arg(format!("{user}:{user}"))
        .arg(context.rootfs())
        .status()
        .map_err(MmdebstrapError::Process)?;

    if status.success() {
        Ok(())
    } else {
        Err(MmdebstrapError::Unsuccessful(status))
    }
}
#[cfg(test)]
mod tests {
    use std::{
        ffi::OsStr,
        io,
        os::unix::process::ExitStatusExt,
        process::{Command, ExitStatus},
        sync::Mutex,
    };

    use super::{CommandRunner, MmdebstrapBootstrapper, MmdebstrapError};
    use crate::{BootstrapConfig, Bootstrapper, BuildContext};

    struct RecordingCommandRunner {
        result: Mutex<Vec<io::Result<ExitStatus>>>,
    }

    impl RecordingCommandRunner {
        fn returning(results: Vec<io::Result<ExitStatus>>) -> Self {
            Self {
                result: Mutex::new(results),
            }
        }
    }

    impl CommandRunner for RecordingCommandRunner {
        fn status(&self, _command: &mut Command) -> io::Result<ExitStatus> {
            self.result.lock().unwrap().remove(0)
        }
    }
    fn test_context() -> BuildContext {
        let config = BootstrapConfig::new(
            "bookworm",
            "amd64",
            "https://deb.debian.org/debian",
            vec!["main".to_owned(), "non-free-firmware".to_owned()],
            "minbase",
        );

        BuildContext::new(
            "build/rootfs",
            "images/source.iso",
            "build/work",
            "build/output.iso",
            "registry/assets",
            config,
        )
    }

    #[test]
    fn bootstrap_root_succeeds_when_command_succeeds() {
        let runner = RecordingCommandRunner::returning(vec![Ok(ExitStatus::from_raw(0))]);

        let bootstrapper = MmdebstrapBootstrapper::with_runner(runner);

        let config = BootstrapConfig::new(
            "trixie",
            "amd64",
            "https://deb.debian.org/debian",
            vec!["main".to_owned()],
            "minbase",
        );

        bootstrapper
            .bootstrap_root(std::path::Path::new("/target"), &config)
            .expect("installation root bootstrap should succeed");
    }

    #[test]
    fn bootstrap_root_returns_error_when_command_fails() {
        let runner = RecordingCommandRunner::returning(vec![Ok(ExitStatus::from_raw(1 << 8))]);

        let bootstrapper = MmdebstrapBootstrapper::with_runner(runner);

        let config = BootstrapConfig::default();

        let error = bootstrapper
            .bootstrap_root(std::path::Path::new("/target"), &config)
            .expect_err("unsuccessful installation bootstrap should fail");

        assert!(matches!(error, MmdebstrapError::Unsuccessful(_)));
    }
    #[test]
    fn constructs_mmdebstrap_command_for_installation_root() {
        let config = BootstrapConfig::new(
            "trixie",
            "amd64",
            "https://deb.debian.org/debian",
            vec!["main".to_owned(), "non-free-firmware".to_owned()],
            "minbase",
        );

        let command = MmdebstrapBootstrapper::new()
            .command_for_root(std::path::Path::new("/target"), &config);

        let arguments = command.get_args().collect::<Vec<_>>();

        assert_eq!(command.get_program(), OsStr::new("mmdebstrap"));
        assert_eq!(
            arguments,
            vec![
                OsStr::new("--mode=root"),
                OsStr::new("--include=ca-certificates,gnupg,grub-efi-amd64,efibootmgr"),
                OsStr::new("--variant=minbase"),
                OsStr::new("--architectures=amd64"),
                OsStr::new("--components=main,non-free-firmware"),
                OsStr::new("trixie"),
                OsStr::new("/target"),
                OsStr::new("https://deb.debian.org/debian"),
            ]
        );
    }

    #[test]
    fn constructs_mmdebstrap_command() {
        let context = test_context();

        let command = MmdebstrapBootstrapper::new().command(&context);
        let arguments = command.get_args().collect::<Vec<_>>();

        assert_eq!(command.get_program(), OsStr::new("mmdebstrap"));
        assert_eq!(
            arguments,
            vec![
                OsStr::new("--mode=root"),
                OsStr::new("--include=ca-certificates,gnupg,grub-efi-amd64,efibootmgr"),
                OsStr::new("--variant=minbase"),
                OsStr::new("--architectures=amd64"),
                OsStr::new("--components=main,non-free-firmware"),
                OsStr::new("bookworm"),
                OsStr::new("build/rootfs"),
                OsStr::new("https://deb.debian.org/debian"),
            ]
        );
    }

    #[test]
    fn bootstrap_succeeds_when_command_succeeds() {
        let runner = RecordingCommandRunner::returning(vec![Ok(ExitStatus::from_raw(0))]);
        let bootstrapper = MmdebstrapBootstrapper::with_runner(runner);

        bootstrapper.bootstrap(&test_context()).unwrap();
    }

    #[test]
    fn bootstrap_returns_process_error_when_command_cannot_run() {
        let runner =
            RecordingCommandRunner::returning(vec![Err(io::Error::other("process unavailable"))]);
        let bootstrapper = MmdebstrapBootstrapper::with_runner(runner);

        let error = bootstrapper.bootstrap(&test_context()).unwrap_err();

        assert!(matches!(error, MmdebstrapError::Process(_)));
    }

    #[test]
    fn bootstrap_returns_error_when_command_exits_unsuccessfully() {
        let runner = RecordingCommandRunner::returning(vec![
            Ok(ExitStatus::from_raw(1 << 8)),
            Ok(ExitStatus::from_raw(1 << 8)),
        ]);

        let bootstrapper = MmdebstrapBootstrapper::with_runner(runner);

        let error = bootstrapper.bootstrap(&test_context()).unwrap_err();

        assert!(matches!(error, MmdebstrapError::Unsuccessful(_)));
    }
    #[test]
    fn constructs_sudo_mmdebstrap_command() {
        let context = test_context();

        let command = MmdebstrapBootstrapper::new().sudo_command(&context);

        assert_eq!(command.get_program(), OsStr::new("sudo"));
    }
}
