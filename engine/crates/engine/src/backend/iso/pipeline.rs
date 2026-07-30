//! ISO build pipeline.

use std::io;

use inspector::IsoInspector;

use super::{
    BootArtifactsStage, GrubConfigStage, InitramfsStage, InspectionStage, IsoContext,
    IsoImageStage, KernelStage, MetadataValidationStage, SourceIsoStage, SquashFsStage,
    ToolValidationStage, WorkspaceStage,
};
/// Coordinates the ISO build process.
pub struct IsoPipeline;

impl IsoPipeline {
    /// Runs the ISO build pipeline.
    ///
    /// # Errors
    ///
    /// Returns an error if an ISO build stage fails.
    pub fn run(context: &mut IsoContext, inspector: &dyn IsoInspector) -> io::Result<()> {
        SourceIsoStage::run(context)?;
        let metadata = InspectionStage::run(context, inspector)?;
        context.state.metadata = Some(metadata);
        MetadataValidationStage::run(context)?;
        ToolValidationStage::run(context)?;
        WorkspaceStage::run(context)?;

        let kernel = KernelStage::run(context)?;
        context.state.kernel = Some(kernel.clone());

        let initramfs = InitramfsStage::run(context)?;
        context.state.initramfs = Some(initramfs.clone());
        let squashfs = SquashFsStage::run(context)?;
        context.state.squashfs = Some(squashfs.clone());

        BootArtifactsStage::run(context, &kernel, &initramfs)?;
        let grub_config = GrubConfigStage::run(context)?;
        context.state.grub_config = Some(grub_config);
        let _iso_image = IsoImageStage::run(context)?;
        Ok(())
    }
}
