//! Errors produced while reading or inspecting ISO images.

use std::{error::Error, fmt, io, process::ExitStatus};

/// Error produced while inspecting an ISO image.
#[derive(Debug)]
pub enum InspectError {
    /// An operating-system operation failed.
    Io(io::Error),

    /// ISO metadata contained invalid UTF-8.
    InvalidUtf8(std::str::Utf8Error),

    /// An external command completed unsuccessfully.
    ProcessFailed {
        /// Name or path of the command.
        command: String,

        /// Exit status returned by the process.
        status: ExitStatus,

        /// Diagnostic output written to standard error.
        stderr: String,
    },

    /// Debian `/.disk/info` metadata was malformed or unsupported.
    InvalidDiskInfo {
        /// Description of the parsing failure.
        reason: &'static str,
    },
}

impl fmt::Display for InspectError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "ISO inspection I/O failed: {error}"),
            Self::InvalidUtf8(error) => {
                write!(formatter, "ISO metadata is not valid UTF-8: {error}")
            }
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
            Self::InvalidDiskInfo { reason } => {
                write!(formatter, "invalid Debian .disk/info metadata: {reason}")
            }
        }
    }
}

impl Error for InspectError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::InvalidUtf8(error) => Some(error),
            Self::ProcessFailed { .. } | Self::InvalidDiskInfo { .. } => None,
        }
    }
}

impl From<std::str::Utf8Error> for InspectError {
    fn from(error: std::str::Utf8Error) -> Self {
        Self::InvalidUtf8(error)
    }
}
impl From<io::Error> for InspectError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}
