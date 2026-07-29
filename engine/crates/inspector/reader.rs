// engine/crates/inspector/src/reader.rs

//! Abstractions for reading files stored inside ISO images.

use crate::InspectError;

/// Reads files from an ISO filesystem.
pub trait IsoReader {
    /// Reads a file stored at `iso_path`.
    ///
    /// The path is interpreted relative to the root of the ISO filesystem.
    ///
    /// # Errors
    ///
    /// Returns an error when the ISO cannot be accessed, the reader process
    /// cannot be started, or the requested file cannot be extracted.
    fn read_file(&self, iso_path: &str) -> Result<Vec<u8>, InspectError>;
}
