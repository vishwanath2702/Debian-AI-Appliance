//! Abstractions for reading files stored inside ISO images.

use crate::InspectError;

/// Reads files from an ISO filesystem.
pub trait IsoReader {
    /// Reads a file stored at `iso_path`.
    ///
    /// # Errors
    ///
    /// Returns an error when the ISO cannot be accessed or the requested file
    /// cannot be extracted.
    fn read_file(&self, iso_path: &str) -> Result<Vec<u8>, InspectError>;
}
