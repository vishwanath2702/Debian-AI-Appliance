//! Errors produced while inspecting system storage.

use std::{error::Error, fmt, io, process::ExitStatus};

/// Error produced while inspecting system storage.
#[derive(Debug)]
pub enum StorageInspectError {
    /// An operating-system operation failed.
    Io(io::Error),

    /// An external storage-inspection command completed unsuccessfully.
    ProcessFailed {
        /// Name or path of the command.
        command: String,

        /// Exit status returned by the process.
        status: ExitStatus,

        /// Diagnostic output written to standard error.
        stderr: String,
    },
}

impl fmt::Display for StorageInspectError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "storage inspection I/O failed: {error}"),
            Self::ProcessFailed {
                command,
                status,
                stderr,
            } => {
                write!(formatter, "{command} failed with status {status}")?;

                let stderr = stderr.trim();

                if !stderr.is_empty() {
                    write!(formatter, ": {stderr}")?;
                }

                Ok(())
            }
        }
    }
}

impl Error for StorageInspectError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::ProcessFailed { .. } => None,
        }
    }
}

impl From<io::Error> for StorageInspectError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

#[cfg(test)]
mod tests {
    use std::process::Command;

    use super::StorageInspectError;

    #[test]
    fn process_failure_describes_failed_command() {
        let status = Command::new("false")
            .status()
            .expect("false command should execute");

        let error = StorageInspectError::ProcessFailed {
            command: "lsblk".to_owned(),
            status,
            stderr: "test failure".to_owned(),
        };

        assert!(error.to_string().contains("lsblk"));
        assert!(error.to_string().contains("test failure"));
    }
}
