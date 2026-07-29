//! Bootable ISO build workflow.

use executor::RootfsRunError;
use model::Plan;

use super::WorkflowContext;
use crate::{
    Bootstrapper, BuildBackend, BuildError, IsoBackend, MmdebstrapBootstrapper, MmdebstrapError,
    RootfsBackend,
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
        let bootstrapper = MmdebstrapBootstrapper::new();
        let rootfs_backend = RootfsBackend::new(
            context.build_context().rootfs().to_path_buf(),
            context.package_repository().clone(),
        );
        let iso_backend = IsoBackend::from_context(context.build_context());

        Self::run_with(context, &bootstrapper, rootfs_backend, iso_backend)
    }

    fn run_with<B, R, I>(
        context: &WorkflowContext<'_>,
        bootstrapper: &B,
        mut rootfs_backend: R,
        mut iso_backend: I,
    ) -> Result<Plan, BuildError>
    where
        B: Bootstrapper<Error = MmdebstrapError>,
        R: BuildBackend<Error = RootfsRunError>,
        I: BuildBackend<Error = std::io::Error>,
    {
        let plan = context.engine().plan(context.capability())?;

        bootstrapper.bootstrap(context.build_context())?;

        rootfs_backend.build(&plan).map_err(BuildError::Rootfs)?;

        iso_backend.build(&plan).map_err(BuildError::Iso)?;

        Ok(plan)
    }
}
