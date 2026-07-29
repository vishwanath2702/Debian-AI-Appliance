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
    /// Checks whether a path exists inside the ISO filesystem.
    ///
    /// # Errors
    ///
    /// Returns an error when the ISO cannot be accessed.
    fn path_exists(&self, iso_path: &str) -> Result<bool, InspectError>;
    /// Lists direct children of a directory inside the ISO filesystem.
    ///
    /// # Errors
    ///
    /// Returns an error when the ISO cannot be accessed or the requested path
    /// cannot be enumerated.
    fn list_files(&self, iso_path: &str) -> Result<Vec<String>, InspectError>;
}
