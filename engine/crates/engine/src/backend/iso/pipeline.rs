//! ISO build pipeline.

use std::io;

use super::IsoContext;

/// Coordinates the ISO build process.
pub struct IsoPipeline;

impl IsoPipeline {
    /// Runs the ISO build pipeline.
    ///
    /// # Errors
    ///
    /// Returns an error if the workspace cannot be prepared.
    pub fn run(context: &IsoContext) -> io::Result<()> {
        context.config.layout.create()
    }
}
