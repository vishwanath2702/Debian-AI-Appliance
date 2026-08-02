//! High-level orchestration for the DAIA engine.

mod bootstrap;
mod bootstrapper;
mod context;
mod mmdebstrap;
mod workflow;

use std::{
    error::Error,
    fmt::{self, Display, Formatter},
    path::PathBuf,
};

pub use bootstrap::BootstrapConfig;
pub use bootstrapper::Bootstrapper;
pub use context::BuildContext;
use executor::{ExecuteError, RootfsRunError};
pub use mmdebstrap::{MmdebstrapBootstrapper, MmdebstrapError};
use model::{Capability, Plan};
use planner::{PlanError, Planner};
use registry::{PackageRepository, Registry};
use resolver::Resolver;
use workflow::IsoWorkflow;
/// Error returned when an appliance build cannot be completed.
#[derive(Debug)]
pub enum BuildError {
    /// An execution plan could not be produced.
    Plan(PlanError),

    /// The build workspace could not be prepared.
    Workspace(std::io::Error),

    /// The root filesystem could not be bootstrapped.
    Bootstrap(MmdebstrapError),

    /// The generated plan could not be executed inside the root filesystem.
    Rootfs(ExecuteError<RootfsRunError>),

    /// The bootable ISO image could not be generated.
    Iso(ExecuteError<std::io::Error>),
}

impl Display for BuildError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Plan(error) => write!(formatter, "failed to build execution plan: {error}"),
            Self::Workspace(error) => {
                write!(formatter, "failed to prepare build workspace: {error}")
            }
            Self::Bootstrap(error) => {
                write!(formatter, "failed to bootstrap root filesystem: {error}")
            }
            Self::Rootfs(error) => {
                write!(
                    formatter,
                    "failed to execute root filesystem build: {error}"
                )
            }
            Self::Iso(error) => {
                write!(formatter, "failed to generate ISO image: {error}")
            }
        }
    }
}

impl Error for BuildError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Plan(error) => Some(error),
            Self::Workspace(error) => Some(error),
            Self::Bootstrap(error) => Some(error),
            Self::Rootfs(error) => Some(error),
            Self::Iso(error) => Some(error),
        }
    }
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

        IsoWorkflow::run(context, package_repository, plan)
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
        asset_directory: PathBuf,
        package_repository: PackageRepository,
    ) -> Result<Plan, BuildError> {
        let backend = RootfsBackend::new(rootfs, asset_directory, package_repository);

        self.build_with_backend(capability, backend)
    }
}
#[cfg(test)]
mod tests {
    use model::{Action, Capability, CapabilityId, PlanStep, Provider, ProviderId};
    use registry::{PackageRepository, Registry};

    use super::{BootstrapConfig, BuildContext, BuildError, Engine, RootfsRunError};
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

    #[test]
    fn build_iso_returns_plan_error_before_workflow_execution() {
        let engine = Engine::from_registry(desktop_registry());
        let build_context = BuildContext::new(
            "build/rootfs",
            "images/source.iso",
            "build/work",
            "build/output.iso",
            "registry/assets",
            BootstrapConfig::new(
                "bookworm",
                "amd64",
                "https://deb.debian.org/debian",
                vec!["main".to_owned()],
                "minbase",
            ),
        );
        let package_repository = PackageRepository::new();

        let result = engine.build_iso(
            &Capability::new("does-not-exist"),
            &build_context,
            &package_repository,
        );

        assert!(matches!(result, Err(BuildError::Plan(_))));
    }
}
