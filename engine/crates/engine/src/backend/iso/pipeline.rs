//! ISO build pipeline.

use std::io;

use super::{InitramfsStage, IsoContext, KernelStage, SquashFsStage, WorkspaceStage};

/// Coordinates the ISO build process.
pub struct IsoPipeline;

impl IsoPipeline {
    /// Runs the ISO build pipeline.
    ///
    /// # Errors
    ///
    /// Returns an error if an ISO build stage fails.
    pub fn run(context: &IsoContext) -> io::Result<()> {
        WorkspaceStage::run(context)?;

        let _kernel = KernelStage::run(context)?;
        let _initramfs = InitramfsStage::run(context)?;
        let _squashfs = SquashFsStage::run(context);

        Ok(())
    }
}
