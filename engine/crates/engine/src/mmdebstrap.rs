//! mmdebstrap root filesystem bootstrapping.

use std::{
    error::Error,
    fmt::{self, Display, Formatter},
    io,
    process::{Command, ExitStatus},
};

use crate::{Bootstrapper, BuildContext};

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
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MmdebstrapBootstrapper;

impl MmdebstrapBootstrapper {
    /// Creates an mmdebstrap bootstrapper.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Constructs the mmdebstrap command for a build context.
    #[must_use]
    pub fn command(&self, context: &BuildContext) -> Command {
        let config = context.bootstrap();

        let mut command = Command::new("mmdebstrap");
        command
            .arg(format!("--variant={}", config.variant()))
            .arg(format!("--architectures={}", config.architecture()))
            .arg(format!("--components={}", config.components().join(",")))
            .arg(config.release())
            .arg(context.rootfs())
            .arg(config.mirror());

        command
    }
}

impl Bootstrapper for MmdebstrapBootstrapper {
    type Error = MmdebstrapError;

    fn bootstrap(&self, context: &BuildContext) -> Result<(), Self::Error> {
        let status = self
            .command(context)
            .status()
            .map_err(MmdebstrapError::Process)?;

        if status.success() {
            Ok(())
        } else {
            Err(MmdebstrapError::Unsuccessful(status))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;

    use super::MmdebstrapBootstrapper;
    use crate::{BootstrapConfig, BuildContext};

    #[test]
    fn constructs_mmdebstrap_command() {
        let config = BootstrapConfig::new(
            "bookworm",
            "amd64",
            "https://deb.debian.org/debian",
            vec!["main".to_owned(), "non-free-firmware".to_owned()],
            "minbase",
        );

        let context = BuildContext::new(
            "build/rootfs",
            "images/source.iso",
            "build/work",
            "build/output.iso",
            config,
        );

        let command = MmdebstrapBootstrapper::new().command(&context);
        let arguments = command.get_args().collect::<Vec<_>>();

        assert_eq!(command.get_program(), OsStr::new("mmdebstrap"));
        assert_eq!(
            arguments,
            vec![
                OsStr::new("--variant=minbase"),
                OsStr::new("--architectures=amd64"),
                OsStr::new("--components=main,non-free-firmware"),
                OsStr::new("bookworm"),
                OsStr::new("build/rootfs"),
                OsStr::new("https://deb.debian.org/debian"),
            ]
        );
    }
}
