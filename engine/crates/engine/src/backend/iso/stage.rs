//! ISO pipeline stages.

use std::io;

use super::IsoContext;

/// Creates the ISO workspace layout.
pub struct WorkspaceStage;

impl WorkspaceStage {
    /// Creates the ISO workspace directories.
    ///
    /// # Errors
    ///
    /// Returns an error if the workspace layout cannot be created.
    pub fn run(context: &IsoContext) -> io::Result<()> {
        context.config.layout.create()
    }
}
