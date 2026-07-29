//! Bootable ISO build workflow.

use model::{Capability, Plan};
use registry::PackageRepository;

use crate::{
    Bootstrapper, BuildBackend, BuildContext, BuildError, Engine, IsoBackend,
    MmdebstrapBootstrapper, RootfsBackend,
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
    pub fn run(
        engine: &Engine,
        capability: &Capability,
        context: &BuildContext,
        package_repository: &PackageRepository,
    ) -> Result<Plan, BuildError> {
        let plan = engine.plan(capability)?;

        MmdebstrapBootstrapper::new().bootstrap(context)?;

        let mut rootfs_backend =
            RootfsBackend::new(context.rootfs().to_path_buf(), package_repository.clone());

        rootfs_backend.build(&plan).map_err(BuildError::Rootfs)?;

        let mut iso_backend = IsoBackend::from_context(context);

        iso_backend.build(&plan).map_err(BuildError::Iso)?;

        Ok(plan)
    }
}
