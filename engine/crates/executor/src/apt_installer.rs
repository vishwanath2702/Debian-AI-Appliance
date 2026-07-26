//! APT-based package installation.

use std::io;
use std::path::Path;

use crate::PackageInstaller;
/// Error returned when APT package installation fails.
#[derive(Debug)]
pub enum AptInstallerError {
    /// The installer command could not be started or completed.
    Io(io::Error),

    /// The installer command exited unsuccessfully.
    CommandFailed(std::process::ExitStatus),
}
impl From<io::Error> for AptInstallerError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}
/// Installs packages using APT.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct AptInstaller;

impl AptInstaller {
    fn command_args(rootfs: &Path, packages: &[String]) -> Vec<String> {
        let mut args = vec![
            rootfs.display().to_string(),
            String::from("apt-get"),
            String::from("install"),
            String::from("--yes"),
        ];

        args.extend(packages.iter().cloned());

        args
    }

    fn command(rootfs: &Path, packages: &[String]) -> std::process::Command {
        let mut command = std::process::Command::new("chroot");
        command.args(Self::command_args(rootfs, packages));

        command
    }
}

impl PackageInstaller for AptInstaller {
    fn install(&self, rootfs: &Path, packages: &[String]) -> Result<(), AptInstallerError> {
        let status = Self::command(rootfs, packages).status()?;

        if !status.success() {
            return Err(AptInstallerError::CommandFailed(status));
        }

        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::AptInstaller;

    #[test]
    fn builds_expected_command_arguments() {
        let packages = vec![String::from("vim"), String::from("curl")];

        let args = AptInstaller::command_args(Path::new("/tmp/rootfs"), &packages);

        assert_eq!(
            args,
            vec![
                String::from("/tmp/rootfs"),
                String::from("apt-get"),
                String::from("install"),
                String::from("--yes"),
                String::from("vim"),
                String::from("curl"),
            ]
        );
    }
}
