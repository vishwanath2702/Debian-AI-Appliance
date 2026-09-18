//! Errors produced while inspecting AI model artifacts.

use std::{error::Error, fmt, io};

/// Error produced while inspecting an AI model artifact.
#[derive(Debug)]
pub enum ModelInspectError {
    /// An operating-system operation failed.
    Io(io::Error),

    /// A GGUF artifact contained an invalid structural header.
    InvalidGguf(String),
}

impl fmt::Display for ModelInspectError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "model inspection I/O failed: {error}"),
            Self::InvalidGguf(message) => {
                write!(formatter, "invalid GGUF artifact: {message}")
            }
        }
    }
}

impl Error for ModelInspectError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::InvalidGguf(_) => None,
        }
    }
}

impl From<io::Error> for ModelInspectError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}
