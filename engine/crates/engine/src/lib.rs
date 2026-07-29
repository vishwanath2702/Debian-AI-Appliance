//! High-level orchestration for the DAIA engine.

mod bootstrap;
mod bootstrapper;
mod context;
mod mmdebstrap;
use std::path::PathBuf;

use executor::{ExecuteError, RootfsRunError};
use model::{Capability, Plan};
use planner::{PlanError, Planner};
use registry::{PackageRepository, Registry};
use resolver::Resolver;

pub use bootstrap::BootstrapConfig;
pub use bootstrapper::Bootstrapper;
pub use context::BuildContext;
pub use mmdebstrap::{MmdebstrapBootstrapper, MmdebstrapError};
/// Error returned when an appliance build cannot be completed.
#[derive(Debug)]
pub enum BuildError {
    /// An execution plan could not be produced.
    Plan(PlanError),

    /// The root filesystem could not be bootstrapped.
    Bootstrap(MmdebstrapError),

    /// The generated plan could not be executed inside the root filesystem.
    Rootfs(ExecuteError<RootfsRunError>),

    /// The bootable ISO image could not be generated.
    Iso(ExecuteError<std::io::Error>),
}

impl From<PlanError> for BuildError {
    fn from(error: PlanError) -> Self {
        Self::Plan(error)
    }
}

impl From<MmdebstrapError> for BuildError {
    fn from(error: MmdebstrapError) -> Self {
        Self::Bootstrap(error)
    }
}

impl From<ExecuteError<RootfsRunError>> for BuildError {
    fn from(error: ExecuteError<RootfsRunError>) -> Self {
        Self::Rootfs(error)
    }
}

impl From<ExecuteError<std::io::Error>> for BuildError {
    fn from(error: ExecuteError<std::io::Error>) -> Self {
        Self::Iso(error)
    }
}
mod backend;

pub use backend::{BuildBackend, IsoBackend, RootfsBackend, RunnerBackend};
/// High-level orchestration entry point.

#[derive(Clone, Debug)]
pub struct Engine {
    planner: Planner,
}

impl Engine {
    /// Creates an engine from a provider registry.
    #[must_use]
    pub const fn from_registry(registry: Registry) -> Self {
        let resolver = Resolver::new(registry);
        let planner = Planner::new(resolver);

        Self { planner }
    }

    /// Builds an execution plan for a capability.
    ///
    /// # Errors
    ///
    /// Returns a [`PlanError`] if no provider can satisfy the
    /// requested capability.
    pub fn plan(&self, capability: &Capability) -> Result<Plan, PlanError> {
        self.planner.build(capability)
    }

    /// Builds and executes an appliance plan using the supplied backend.
    ///
    /// # Errors
    ///
    /// Returns a [`BuildError`] if planning or backend execution fails.
    pub fn build_with_backend<B>(
        &self,
        capability: &Capability,
        mut backend: B,
    ) -> Result<Plan, BuildError>
    where
        B: BuildBackend,
        BuildError: From<ExecuteError<B::Error>>,
    {
        let plan = self.plan(capability)?;

        backend.build(&plan).map_err(BuildError::from)?;

        Ok(plan)
    }
    /// Builds and executes an appliance plan as a bootable ISO image.
    ///
    /// # Errors
    ///
    /// Returns a [`BuildError`] if planning, bootstrapping, root filesystem
    /// execution, or ISO generation fails.
    pub fn build_iso(
        &self,
        capability: &Capability,
        context: &BuildContext,
        package_repository: &PackageRepository,
    ) -> Result<Plan, BuildError> {
        let plan = self.plan(capability)?;

        MmdebstrapBootstrapper::new().bootstrap(context)?;

        let mut rootfs_backend =
            RootfsBackend::new(context.rootfs().to_path_buf(), package_repository.clone());

        rootfs_backend.build(&plan).map_err(BuildError::Rootfs)?;

        let mut iso_backend = IsoBackend::from_context(context);

        iso_backend.build(&plan).map_err(BuildError::Iso)?;

        Ok(plan)
    }

    /// Builds and executes an appliance plan inside a root filesystem.
    ///
    /// # Errors
    ///
    /// Returns a [`BuildError`] if planning or execution fails.
    pub fn build(
        &self,
        capability: &Capability,
        rootfs: PathBuf,
        package_repository: PackageRepository,
    ) -> Result<Plan, BuildError> {
        let backend = RootfsBackend::new(rootfs, package_repository);

        self.build_with_backend(capability, backend)
    }
}
#[cfg(test)]
mod tests {
    use model::{Action, Capability, CapabilityId, PlanStep, Provider, ProviderId};
    use registry::Registry;

    use super::{BuildError, Engine, RootfsRunError};
    fn desktop_registry() -> Registry {
        Registry::from_providers(vec![Provider {
            id: ProviderId::new("desktop"),
            capability: CapabilityId::new("desktop"),
            steps: vec![
                PlanStep::new(Action::InstallPackageManifest("desktop".to_owned())),
                PlanStep::new(Action::EnableService("display-manager".to_owned())),
            ],
        }])
        .expect("desktop test registry should be valid")
    }

    #[test]
    fn builds_desktop_plan() {
        let engine = Engine::from_registry(desktop_registry());

        let plan = engine
            .plan(&Capability::new("desktop"))
            .expect("desktop plan should build");

        assert_eq!(plan.capability, Capability::new("desktop"));
        assert_eq!(plan.provider, ProviderId::new("desktop"));

        assert_eq!(
            plan.steps,
            vec![
                PlanStep::new(Action::InstallPackageManifest("desktop".to_owned())),
                PlanStep::new(Action::EnableService("display-manager".to_owned())),
            ]
        );
    }

    #[test]
    fn builds_plan_from_custom_registry() {
        let provider = Provider {
            id: ProviderId::new("custom"),
            capability: CapabilityId::new("custom"),
            steps: vec![PlanStep::new(Action::EnableService("customd".to_owned()))],
        };

        let registry =
            Registry::from_providers(vec![provider]).expect("test registry should be valid");
        let engine = Engine::from_registry(registry);

        let plan = engine
            .plan(&Capability::new("custom"))
            .expect("custom plan should build");

        assert_eq!(plan.provider, ProviderId::new("custom"));
        assert_eq!(
            plan.steps,
            vec![PlanStep::new(Action::EnableService("customd".to_owned()))]
        );
    }

    #[test]
    fn returns_error_for_unknown_capability() {
        let engine = Engine::from_registry(desktop_registry());

        assert!(engine.plan(&Capability::new("does-not-exist")).is_err());
    }
    #[test]
    fn converts_plan_error_into_build_error() {
        let engine = Engine::from_registry(desktop_registry());

        let plan_error = engine
            .plan(&Capability::new("does-not-exist"))
            .expect_err("unknown capability should fail");

        let build_error = BuildError::from(plan_error);

        assert!(matches!(build_error, BuildError::Plan(_)));
    }

    #[test]
    fn converts_rootfs_execution_error_into_build_error() {
        let execution_error: executor::ExecuteError<RootfsRunError> =
            executor::ExecuteError::Environment(std::io::Error::other("prepare failed"));

        let build_error = BuildError::from(execution_error);

        assert!(matches!(build_error, BuildError::Rootfs(_)));
    }
}
