//! ISO build pipeline.

use std::io;

use inspector::IsoInspector;

use super::{
    BootArtifactsStage, GrubConfigStage, InitramfsStage, InspectionStage, IsoContext,
    IsoImageStage, KernelStage, SourceIsoStage, SquashFsStage, ToolValidationStage, WorkspaceStage,
};
/// Coordinates the ISO build process.
pub struct IsoPipeline;

impl IsoPipeline {
    /// Runs the ISO build pipeline.
    ///
    /// # Errors
    ///
    /// Returns an error if an ISO build stage fails.
    pub fn run(context: &IsoContext, inspector: &dyn IsoInspector) -> io::Result<()> {
        SourceIsoStage::run(context)?;
        let _metadata = InspectionStage::run(context, inspector)?;
        ToolValidationStage::run(context)?;
        WorkspaceStage::run(context)?;

        let kernel = KernelStage::run(context)?;
        let initramfs = InitramfsStage::run(context)?;
        let _squashfs = SquashFsStage::run(context)?;

        BootArtifactsStage::run(context, &kernel, &initramfs)?;
        let _grub_config = GrubConfigStage::run(context)?;
        let _iso_image = IsoImageStage::run(context)?;

        Ok(())
    }
}
