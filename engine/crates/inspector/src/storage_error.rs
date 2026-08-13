//! Errors produced while inspecting system storage.

use std::{error::Error, fmt, io};

/// Error produced while inspecting system storage.
#[derive(Debug)]
pub enum StorageInspectError {
    /// An operating-system operation failed.
    Io(io::Error),
}

impl fmt::Display for StorageInspectError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "storage inspection I/O failed: {error}"),
        }
    }
}

impl Error for StorageInspectError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
        }
    }
}

impl From<io::Error> for StorageInspectError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}
