// engine/crates/inspector/src/error.rs

//! Errors produced while reading or inspecting ISO images.

use std::{
    error::Error,
    fmt,
    io,
    process::ExitStatus,
};

/// Error produced while inspecting an ISO image.
#[derive(Debug)]
pub enum InspectError {
    /// An operating-system operation failed.
    Io(io::Error),

    /// An external command completed unsuccessfully.
    ProcessFailed {
        /// Name or path of the command.
        command: String,

        /// Exit status returned by the process.
        status: ExitStatus,

        /// Diagnostic output written to standard error.
        stderr: String,
    },
}

impl fmt::Display for InspectError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "ISO inspection I/O failed: {error}"),
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

impl Error for InspectError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::ProcessFailed { .. } => None,
        }
    }
}

impl From<io::Error> for InspectError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}
