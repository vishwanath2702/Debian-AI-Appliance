//! Bootable ISO build workflow.

use model::Plan;

use super::WorkflowContext;
use crate::{
    Bootstrapper, BuildBackend, BuildError, IsoBackend, MmdebstrapBootstrapper, RootfsBackend,
};

/// Coordinates creation of a bootable ISO appliance.
pub struct IsoWorkflow;

impl IsoWorkflow {
    /// Runs the bootable ISO workflow.
    ///
    /// # Errors
    ///
    /// Returns a [`BuildError`] if planning, bootstrapping, root filesystem
    /// execution, or ISO generation fails.
    pub fn run(context: &WorkflowContext<'_>) -> Result<Plan, BuildError> {
        let plan = context.engine().plan(context.capability())?;

        MmdebstrapBootstrapper::new().bootstrap(context.build_context())?;

        let mut rootfs_backend = RootfsBackend::new(
            context.build_context().rootfs().to_path_buf(),
            context.package_repository().clone(),
        );

        rootfs_backend.build(&plan).map_err(BuildError::Rootfs)?;

        let mut iso_backend = IsoBackend::from_context(context.build_context());

        iso_backend.build(&plan).map_err(BuildError::Iso)?;

        Ok(plan)
    }
}
