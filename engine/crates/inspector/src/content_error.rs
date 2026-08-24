//! Errors produced while inspecting external content.

use std::{error::Error, fmt, io};

/// Error produced while inspecting external content.
#[derive(Debug)]
pub enum ContentInspectError {
    /// An operating-system operation failed.
    Io(io::Error),
}

impl fmt::Display for ContentInspectError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "content inspection I/O failed: {error}"),
        }
    }
}

impl Error for ContentInspectError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
        }
    }
}

impl From<io::Error> for ContentInspectError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}
