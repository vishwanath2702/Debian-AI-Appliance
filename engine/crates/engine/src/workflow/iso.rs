//! Bootable ISO build workflow.

use crate::{
    Bootstrapper, BuildBackend, BuildContext, BuildError, IsoBackend, MmdebstrapBootstrapper,
    MmdebstrapError, RootfsBackend,
};
use executor::RootfsRunError;
use model::Plan;
use std::fs;

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
    fn prepare(build_context: &BuildContext) -> Result<(), BuildError> {
        clean_directory(build_context.rootfs())?;
        clean_directory(build_context.work_directory())?;

        if build_context.output_iso().exists() {
            fs::remove_file(build_context.output_iso()).map_err(BuildError::Workspace)?;
        }

        Ok(())
    }
    fn execute(mut self, build_context: &BuildContext, plan: &Plan) -> Result<(), BuildError> {
        Self::prepare(build_context)?;

        self.bootstrap(build_context)?;
        self.build_rootfs(plan)?;
        self.prepare_live_rootfs(build_context)?;
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

    fn prepare_live_rootfs(&self, build_context: &BuildContext) -> Result<(), BuildError> {
        let daia_directory = build_context.rootfs().join("usr/share/daia");

        let registry_directory = build_context.asset_directory().parent().ok_or_else(|| {
            BuildError::Workspace(std::io::Error::other("asset directory has no parent"))
        })?;

        let package_manifest_source = registry_directory.join("package-manifests");
        let provider_source = registry_directory.join("providers");
        let appliance_profile_source = registry_directory.join("appliance-profiles");

        copy_directory_contents(
            &package_manifest_source,
            &daia_directory.join("package-manifests"),
        )?;

        copy_directory_contents(&provider_source, &daia_directory.join("providers"))?;

        copy_directory_contents(
            &appliance_profile_source,
            &daia_directory.join("appliance-profiles"),
        )?;

        copy_directory_contents(
            build_context.asset_directory(),
            &daia_directory.join("assets"),
        )?;

        if let Some(daia_binary) = build_context.daia_binary() {
            let usr_bin = build_context.rootfs().join("usr/bin");

            fs::create_dir_all(&usr_bin).map_err(BuildError::Workspace)?;

            fs::copy(daia_binary, usr_bin.join("daia")).map_err(BuildError::Workspace)?;
        }

        clean_live_rootfs(build_context.rootfs())?;
        Ok(())
    }

    fn build_iso(&mut self, plan: &Plan) -> Result<(), BuildError> {
        self.iso_backend.build(plan).map_err(BuildError::Iso)
    }
}
fn clean_directory(path: &std::path::Path) -> Result<(), BuildError> {
    if path.exists() {
        fs::remove_dir_all(path).map_err(BuildError::Workspace)?;
    }

    fs::create_dir_all(path).map_err(BuildError::Workspace)?;

    Ok(())
}
fn clean_live_rootfs(rootfs: &std::path::Path) -> Result<(), BuildError> {
    let paths = [
        rootfs.join("var/cache/apt/archives"),
        rootfs.join("var/lib/apt/lists"),
    ];

    for path in paths {
        if path.exists() {
            fs::remove_dir_all(&path).map_err(BuildError::Workspace)?;
        }

        fs::create_dir_all(&path).map_err(BuildError::Workspace)?;
    }

    Ok(())
}

fn copy_directory_contents(
    source: &std::path::Path,
    destination: &std::path::Path,
) -> Result<(), BuildError> {
    fs::create_dir_all(destination).map_err(BuildError::Workspace)?;

    for entry in fs::read_dir(source).map_err(BuildError::Workspace)? {
        let entry = entry.map_err(BuildError::Workspace)?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());

        if source_path.is_dir() {
            copy_directory_contents(&source_path, &destination_path)?;
        } else {
            fs::copy(&source_path, &destination_path).map_err(BuildError::Workspace)?;
        }
    }

    Ok(())
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
                build_context.asset_directory().to_path_buf(),
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
        fs, io,
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
                std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../registry/assets"),
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
        let (engine, capability, _, _) = workflow_inputs();

        let temp = tempfile::tempdir().expect("temporary workflow directory should be created");

        let registry_directory = temp.path().join("registry");
        let asset_directory = registry_directory.join("assets");
        let package_manifest_directory = registry_directory.join("package-manifests");
        let provider_directory = registry_directory.join("providers");
        let appliance_profile_directory = registry_directory.join("appliance-profiles");

        std::fs::create_dir_all(&asset_directory).expect("asset directory should be created");

        std::fs::create_dir_all(&package_manifest_directory)
            .expect("package manifest directory should be created");

        std::fs::create_dir_all(&provider_directory).expect("provider directory should be created");

        std::fs::create_dir_all(&appliance_profile_directory)
            .expect("appliance profile directory should be created");

        let build_context = BuildContext::new(
            temp.path().join("rootfs"),
            temp.path().join("source.iso"),
            temp.path().join("work"),
            temp.path().join("output.iso"),
            asset_directory,
            BootstrapConfig::default(),
        );

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
    fn prepares_live_rootfs_daia_binary() {
        let temp = tempfile::tempdir().expect("temporary directory should be created");
        let rootfs = temp.path().join("rootfs");

        let registry_directory = temp.path().join("registry");
        let asset_directory = registry_directory.join("assets");
        let package_manifest_directory = registry_directory.join("package-manifests");
        let provider_directory = registry_directory.join("providers");
        let appliance_profile_directory = registry_directory.join("appliance-profiles");

        let daia_binary = temp.path().join("daia");

        fs::create_dir_all(&rootfs).expect("rootfs should be created");

        fs::create_dir_all(&asset_directory).expect("asset directory should be created");

        fs::create_dir_all(&package_manifest_directory)
            .expect("package manifest directory should be created");

        fs::create_dir_all(&provider_directory).expect("provider directory should be created");

        fs::create_dir_all(&appliance_profile_directory)
            .expect("appliance profile directory should be created");

        fs::write(&daia_binary, b"daia").expect("DAIA binary fixture should be written");

        let build_context = BuildContext::new(
            rootfs.clone(),
            temp.path().join("source.iso"),
            temp.path().join("work"),
            temp.path().join("output.iso"),
            asset_directory,
            BootstrapConfig::default(),
        )
        .with_daia_binary(&daia_binary);

        let pipeline = IsoPipeline {
            bootstrapper: RecordingBootstrapper {
                log: Arc::new(Mutex::new(Vec::new())),
                error: false,
            },
            rootfs_backend: RecordingRootfsBackend {
                log: Arc::new(Mutex::new(Vec::new())),
                error: false,
            },
            iso_backend: RecordingIsoBackend {
                log: Arc::new(Mutex::new(Vec::new())),
                error: false,
            },
        };

        pipeline
            .prepare_live_rootfs(&build_context)
            .expect("live rootfs preparation should succeed");

        assert_eq!(
            fs::read(rootfs.join("usr/bin/daia")).expect("DAIA binary should be copied"),
            b"daia"
        );
    }

    #[test]
    fn prepares_live_rootfs_runtime_directories() {
        let temp = tempfile::tempdir().expect("temporary directory should be created");
        let rootfs = temp.path().join("rootfs");

        let registry_directory = temp.path().join("registry");
        let asset_directory = registry_directory.join("assets");
        let package_manifest_directory = registry_directory.join("package-manifests");
        let provider_directory = registry_directory.join("providers");
        let appliance_profile_directory = registry_directory.join("appliance-profiles");

        fs::create_dir_all(&rootfs).expect("rootfs should be created");

        fs::create_dir_all(&asset_directory).expect("asset directory should be created");

        fs::create_dir_all(&package_manifest_directory)
            .expect("package manifest directory should be created");

        fs::create_dir_all(&provider_directory).expect("provider directory should be created");

        fs::create_dir_all(&appliance_profile_directory)
            .expect("appliance profile directory should be created");

        let build_context = BuildContext::new(
            rootfs.clone(),
            temp.path().join("source.iso"),
            temp.path().join("work"),
            temp.path().join("output.iso"),
            asset_directory,
            BootstrapConfig::default(),
        );

        let pipeline = IsoPipeline {
            bootstrapper: RecordingBootstrapper {
                log: Arc::new(Mutex::new(Vec::new())),
                error: false,
            },
            rootfs_backend: RecordingRootfsBackend {
                log: Arc::new(Mutex::new(Vec::new())),
                error: false,
            },
            iso_backend: RecordingIsoBackend {
                log: Arc::new(Mutex::new(Vec::new())),
                error: false,
            },
        };

        pipeline
            .prepare_live_rootfs(&build_context)
            .expect("live rootfs preparation should succeed");

        assert!(rootfs.join("usr/share/daia/package-manifests").is_dir());

        assert!(rootfs.join("usr/share/daia/providers").is_dir());

        assert!(rootfs.join("usr/share/daia/appliance-profiles").is_dir());

        assert!(rootfs.join("usr/share/daia/assets").is_dir());
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
