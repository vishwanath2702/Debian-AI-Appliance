//! Bootable ISO build workflow.

use executor::RootfsRunError;
use model::Plan;

use crate::{
    Bootstrapper, BuildBackend, BuildContext, BuildError, IsoBackend, MmdebstrapBootstrapper,
    MmdebstrapError, RootfsBackend,
};

use registry::PackageRepository;

/// Coordinates creation of a bootable ISO appliance.
pub struct IsoWorkflow;

struct IsoPipeline<B, R, I> {
    bootstrapper: B,
    rootfs_backend: R,
    iso_backend: I,
}

impl<B, R, I> IsoPipeline<B, R, I>
where
    B: Bootstrapper<Error = MmdebstrapError>,
    R: BuildBackend<Error = RootfsRunError>,
    I: BuildBackend<Error = std::io::Error>,
{
    fn execute(mut self, build_context: &BuildContext, plan: &Plan) -> Result<(), BuildError> {
        self.bootstrap(build_context)?;
        self.build_rootfs(plan)?;
        self.build_iso(plan)?;

        Ok(())
    }

    fn bootstrap(&self, build_context: &BuildContext) -> Result<(), BuildError> {
        self.bootstrapper.bootstrap(build_context)?;

        Ok(())
    }

    fn build_rootfs(&mut self, plan: &Plan) -> Result<(), BuildError> {
        self.rootfs_backend.build(plan).map_err(BuildError::Rootfs)
    }

    fn build_iso(&mut self, plan: &Plan) -> Result<(), BuildError> {
        self.iso_backend.build(plan).map_err(BuildError::Iso)
    }
}
impl IsoWorkflow {
    /// Runs the bootable ISO workflow.
    ///
    /// # Errors
    ///
    /// Returns a [`BuildError`] if bootstrapping, root filesystem execution,
    /// or ISO generation fails.
    pub fn run(
        build_context: &BuildContext,
        package_repository: &PackageRepository,
        plan: Plan,
    ) -> Result<Plan, BuildError> {
        let pipeline = Self::production_pipeline(build_context, package_repository);

        Self::run_with(build_context, plan, pipeline)
    }

    fn production_pipeline(
        build_context: &BuildContext,
        package_repository: &PackageRepository,
    ) -> IsoPipeline<MmdebstrapBootstrapper, RootfsBackend, IsoBackend> {
        IsoPipeline {
            bootstrapper: MmdebstrapBootstrapper::new(),
            rootfs_backend: RootfsBackend::new(
                build_context.rootfs().to_path_buf(),
                package_repository.clone(),
            ),
            iso_backend: IsoBackend::from_context(build_context),
        }
    }

    fn run_with<B, R, I>(
        build_context: &BuildContext,
        plan: Plan,
        pipeline: IsoPipeline<B, R, I>,
    ) -> Result<Plan, BuildError>
    where
        B: Bootstrapper<Error = MmdebstrapError>,
        R: BuildBackend<Error = RootfsRunError>,
        I: BuildBackend<Error = std::io::Error>,
    {
        pipeline.execute(build_context, &plan)?;

        Ok(plan)
    }
}
#[cfg(test)]
mod tests {
    use std::{
        io,
        sync::{Arc, Mutex},
    };

    use executor::{ExecuteError, RootfsRunError};
    use model::{Action, Capability, CapabilityId, Plan, PlanStep, Provider, ProviderId};
    use registry::{PackageRepository, Registry};

    use super::{IsoPipeline, IsoWorkflow};
    use crate::{
        BootstrapConfig, Bootstrapper, BuildBackend, BuildContext, BuildError, Engine,
        MmdebstrapError,
    };

    type ExecutionLog = Arc<Mutex<Vec<&'static str>>>;

    struct RecordingBootstrapper {
        log: ExecutionLog,
        error: bool,
    }

    impl Bootstrapper for RecordingBootstrapper {
        type Error = MmdebstrapError;

        fn bootstrap(&self, _context: &BuildContext) -> Result<(), Self::Error> {
            self.log
                .lock()
                .expect("execution log should not be poisoned")
                .push("bootstrap");

            if self.error {
                Err(MmdebstrapError::Process(io::Error::other(
                    "bootstrap failed",
                )))
            } else {
                Ok(())
            }
        }
    }

    struct RecordingRootfsBackend {
        log: ExecutionLog,
        error: bool,
    }

    impl BuildBackend for RecordingRootfsBackend {
        type Error = RootfsRunError;

        fn build(&mut self, _plan: &Plan) -> Result<(), ExecuteError<Self::Error>> {
            self.log
                .lock()
                .expect("execution log should not be poisoned")
                .push("rootfs");

            if self.error {
                Err(ExecuteError::Environment(io::Error::other("rootfs failed")))
            } else {
                Ok(())
            }
        }
    }

    struct RecordingIsoBackend {
        log: ExecutionLog,
        error: bool,
    }

    impl BuildBackend for RecordingIsoBackend {
        type Error = io::Error;

        fn build(&mut self, _plan: &Plan) -> Result<(), ExecuteError<Self::Error>> {
            self.log
                .lock()
                .expect("execution log should not be poisoned")
                .push("iso");

            if self.error {
                Err(ExecuteError::Environment(io::Error::other("iso failed")))
            } else {
                Ok(())
            }
        }
    }

    fn workflow_inputs() -> (Engine, Capability, BuildContext, PackageRepository) {
        let registry = Registry::from_providers(vec![Provider {
            id: ProviderId::new("desktop"),
            capability: CapabilityId::new("desktop"),
            steps: vec![
                PlanStep::new(Action::InstallPackageManifest("desktop".to_owned())),
                PlanStep::new(Action::EnableService("display-manager".to_owned())),
            ],
        }])
        .expect("test registry should be valid");

        let bootstrap = BootstrapConfig::new(
            "bookworm",
            "amd64",
            "https://deb.debian.org/debian",
            vec!["main".to_owned()],
            "minbase",
        );

        (
            Engine::from_registry(registry),
            Capability::new("desktop"),
            BuildContext::new(
                "build/rootfs",
                "images/source.iso",
                "build/work",
                "build/output.iso",
                "registry/assets",
                bootstrap,
            ),
            PackageRepository::new(),
        )
    }

    fn run_workflow(
        bootstrap_error: bool,
        rootfs_error: bool,
        iso_error: bool,
    ) -> (Result<Plan, BuildError>, ExecutionLog) {
        let (engine, capability, build_context, _) = workflow_inputs();
        let plan = engine
            .plan(&capability)
            .expect("test workflow plan should build");
        let log = Arc::new(Mutex::new(Vec::new()));
        let result = IsoWorkflow::run_with(
            &build_context,
            plan,
            IsoPipeline {
                bootstrapper: RecordingBootstrapper {
                    log: Arc::clone(&log),
                    error: bootstrap_error,
                },
                rootfs_backend: RecordingRootfsBackend {
                    log: Arc::clone(&log),
                    error: rootfs_error,
                },
                iso_backend: RecordingIsoBackend {
                    log: Arc::clone(&log),
                    error: iso_error,
                },
            },
        );
        (result, log)
    }
    #[test]
    fn executes_workflow_stages_in_order() {
        let (result, log) = run_workflow(false, false, false);

        let plan = result.expect("workflow should succeed");

        assert_eq!(plan.capability, Capability::new("desktop"));
        assert_eq!(plan.provider, ProviderId::new("desktop"));
        assert_eq!(
            *log.lock().expect("execution log should not be poisoned"),
            vec!["bootstrap", "rootfs", "iso"]
        );
    }

    #[test]
    fn returns_bootstrap_error_without_running_backends() {
        let (result, log) = run_workflow(true, false, false);

        assert!(matches!(result, Err(BuildError::Bootstrap(_))));
        assert_eq!(
            *log.lock().expect("execution log should not be poisoned"),
            vec!["bootstrap"]
        );
    }

    #[test]
    fn returns_rootfs_error_without_running_iso_backend() {
        let (result, log) = run_workflow(false, true, false);

        assert!(matches!(result, Err(BuildError::Rootfs(_))));
        assert_eq!(
            *log.lock().expect("execution log should not be poisoned"),
            vec!["bootstrap", "rootfs"]
        );
    }

    #[test]
    fn returns_iso_error_after_rootfs_execution() {
        let (result, log) = run_workflow(false, false, true);

        assert!(matches!(result, Err(BuildError::Iso(_))));
        assert_eq!(
            *log.lock().expect("execution log should not be poisoned"),
            vec!["bootstrap", "rootfs", "iso"]
        );
    }
}
