//! APT-based package installation.

use crate::PackageInstaller;
use std::{
    cell::Cell,
    error::Error,
    fmt::{self, Display, Formatter},
    io,
    path::Path,
};

/// Error returned when APT package installation fails.
#[derive(Debug)]
pub enum AptInstallerError {
    /// The installer command could not be started or completed.
    Io(io::Error),

    /// The installer command exited unsuccessfully.
    CommandFailed(std::process::ExitStatus),
}
impl Display for AptInstallerError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "failed to execute APT command: {error}"),
            Self::CommandFailed(status) => {
                write!(formatter, "APT command exited unsuccessfully: {status}")
            }
        }
    }
}

impl Error for AptInstallerError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::CommandFailed(_) => None,
        }
    }
}
impl From<io::Error> for AptInstallerError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// Installs packages using APT.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AptInstaller {
    updated: Cell<bool>,
}

impl AptInstaller {
    /// Creates an APT package installer.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            updated: Cell::new(false),
        }
    }

    fn update_command_args() -> Vec<String> {
        vec![String::from("apt-get"), String::from("update")]
    }

    fn install_command_args(packages: &[String]) -> Vec<String> {
        let mut args = vec![
            String::from("apt-get"),
            String::from("install"),
            String::from("--yes"),
        ];

        args.extend(packages.iter().cloned());

        args
    }

    fn run_in_rootfs(rootfs: &Path, args: &[String]) -> Result<(), AptInstallerError> {
        let status = std::process::Command::new("chroot")
            .arg(rootfs)
            .args(args)
            .status()?;

        if !status.success() {
            return Err(AptInstallerError::CommandFailed(status));
        }

        Ok(())
    }

    fn update_once(&self, rootfs: &Path) -> Result<(), AptInstallerError> {
        if self.updated.get() {
            return Ok(());
        }

        let args = Self::update_command_args();
        Self::run_in_rootfs(rootfs, &args)?;
        self.updated.set(true);

        Ok(())
    }
}

impl PackageInstaller for AptInstaller {
    fn install(&self, rootfs: &Path, packages: &[String]) -> Result<(), AptInstallerError> {
        self.update_once(rootfs)?;

        let args = Self::install_command_args(packages);
        Self::run_in_rootfs(rootfs, &args)
    }
}
#[cfg(test)]
mod tests {

    use super::AptInstaller;

    #[test]
    fn builds_expected_update_command_arguments() {
        let args = AptInstaller::update_command_args();

        assert_eq!(args, vec![String::from("apt-get"), String::from("update")]);
    }
    #[test]
    fn builds_expected_install_command_arguments() {
        let packages = vec![String::from("vim"), String::from("curl")];

        let args = AptInstaller::install_command_args(&packages);

        assert_eq!(
            args,
            vec![
                String::from("apt-get"),
                String::from("install"),
                String::from("--yes"),
                String::from("vim"),
                String::from("curl"),
            ]
        );
    }
}
