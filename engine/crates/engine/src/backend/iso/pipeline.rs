//! ISO build pipeline.

use std::io;

use super::{IsoContext, WorkspaceStage};

/// Coordinates the ISO build process.
pub struct IsoPipeline;

impl IsoPipeline {
    /// Runs the ISO build pipeline.
    ///
    /// # Errors
    ///
    /// Returns an error if an ISO build stage fails.
    pub fn run(context: &IsoContext) -> io::Result<()> {
        WorkspaceStage::run(context)
    }
}
