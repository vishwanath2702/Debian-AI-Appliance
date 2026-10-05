//! Bootable ISO build workflow.

use crate::{
    Bootstrapper, BuildBackend, BuildContext, BuildError, IsoBackend, MmdebstrapBootstrapper,
    MmdebstrapError, RootfsBackend,
};
use executor::RootfsRunError;
use model::Plan;
use registry::PackageRepository;
use std::fs;

/// Coordinates creation of a bootable ISO appliance.
pub struct IsoWorkflow;

trait LiveRootfsPreparer {
    fn prepare(&mut self, build_context: &BuildContext) -> Result<(), BuildError>;
}

#[derive(Clone, Copy, Debug, Default)]
struct SystemLiveRootfsPreparer;

impl LiveRootfsPreparer for SystemLiveRootfsPreparer {
    fn prepare(&mut self, build_context: &BuildContext) -> Result<(), BuildError> {
        let daia_directory = build_context.rootfs().join("usr/share/daia");

        let registry_directory = build_context.asset_directory().parent().ok_or_else(|| {
            BuildError::Workspace(std::io::Error::other("asset directory has no parent"))
        })?;

        let package_manifest_source = registry_directory.join("package-manifests");
        let provider_source = registry_directory.join("providers");
        let appliance_profile_source = registry_directory.join("appliance-profiles");
        let content_repository_source = registry_directory.join("content-repositories");

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
            &content_repository_source,
            &daia_directory.join("content-repositories"),
        )?;

        copy_directory_contents(
            build_context.asset_directory(),
            &daia_directory.join("assets"),
        )?;

        if let Some(daia_binary) = build_context.daia_binary() {
            let usr_bin = build_context.rootfs().join("usr/bin");

            create_directory(&usr_bin)?;

            let destination = usr_bin.join("daia");

            copy_file(daia_binary, &destination)?;
        }

        if let Some(daia_tui_binary) = build_context.daia_tui_binary() {
            let usr_bin = build_context.rootfs().join("usr/bin");

            create_directory(&usr_bin)?;

            let destination = usr_bin.join("daia-tui");

            copy_file(daia_tui_binary, &destination)?;
        }

        let systemd_directory = build_context.rootfs().join("etc/systemd/system");

        let multi_user_wants = systemd_directory.join("multi-user.target.wants");

        create_directory(&systemd_directory)?;
        create_directory(&multi_user_wants)?;

        let default_target = systemd_directory.join("default.target");

        if std::fs::symlink_metadata(&default_target).is_ok() {
            remove_file(&default_target)?;
        }

        create_symlink(
            std::path::Path::new("/usr/lib/systemd/system/multi-user.target"),
            &default_target,
        )?;

        let installer_service = systemd_directory.join("daia-installer.service");

        write_file(
            &installer_service,
            r#"[Unit]
Description=DAIA Rust Installer
After=systemd-user-sessions.service
Before=graphical.target
Conflicts=graphical.target

[Service]
Type=simple
ExecStart=/usr/bin/daia-tui
StandardInput=tty
StandardOutput=tty
StandardError=tty
TTYPath=/dev/tty1
TTYReset=yes
TTYVHangup=yes
TTYVTDisallocate=no
Restart=no

[Install]
WantedBy=multi-user.target
"#,
        )?;

        let installer_link = multi_user_wants.join("daia-installer.service");

        if installer_link.exists() {
            remove_file(&installer_link)?;
        }

        create_symlink(
            std::path::Path::new("../daia-installer.service"),
            &installer_link,
        )?;

        let tty1_getty_link = build_context
            .rootfs()
            .join("etc/systemd/system/getty.target.wants/getty@tty1.service");

        if tty1_getty_link.exists() {
            remove_file(&tty1_getty_link)?;
        }

        clean_live_rootfs(build_context.rootfs())?;

        Ok(())
    }
}

struct IsoPipeline<B, R, L, I> {
    bootstrapper: B,
    rootfs_backend: R,
    live_rootfs_preparer: L,
    iso_backend: I,
}

impl<B, R, L, I> IsoPipeline<B, R, L, I>
where
    B: Bootstrapper<Error = MmdebstrapError>,
    R: BuildBackend<Error = RootfsRunError>,
    L: LiveRootfsPreparer,
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
        self.live_rootfs_preparer.prepare(build_context)?;
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

fn create_directory(path: &std::path::Path) -> Result<(), BuildError> {
    match fs::create_dir_all(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
            let status = std::process::Command::new("sudo")
                .arg("mkdir")
                .arg("-p")
                .arg("--")
                .arg(path)
                .status()
                .map_err(BuildError::Workspace)?;

            if status.success() {
                Ok(())
            } else {
                Err(BuildError::Workspace(std::io::Error::other(format!(
                    "failed to create privileged directory `{}`",
                    path.display()
                ))))
            }
        }
        Err(error) => Err(BuildError::Workspace(error)),
    }
}

fn clean_directory(path: &std::path::Path) -> Result<(), BuildError> {
    if path.exists() {
        match fs::remove_dir_all(path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
                let status = std::process::Command::new("sudo")
                    .arg("rm")
                    .arg("-rf")
                    .arg("--")
                    .arg(path)
                    .status()
                    .map_err(BuildError::Workspace)?;

                if !status.success() {
                    return Err(BuildError::Workspace(std::io::Error::other(format!(
                        "failed to remove privileged build directory `{}`",
                        path.display()
                    ))));
                }
            }
            Err(error) => return Err(BuildError::Workspace(error)),
        }
    }

    create_directory(path)?;

    Ok(())
}

fn clean_live_rootfs(rootfs: &std::path::Path) -> Result<(), BuildError> {
    let paths = [
        rootfs.join("var/cache/apt/archives"),
        rootfs.join("var/lib/apt/lists"),
    ];

    for path in paths {
        clean_directory(&path)?;
    }

    Ok(())
}

fn remove_file(path: &std::path::Path) -> Result<(), BuildError> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
            let status = std::process::Command::new("sudo")
                .arg("rm")
                .arg("-f")
                .arg("--")
                .arg(path)
                .status()
                .map_err(BuildError::Workspace)?;

            if status.success() {
                Ok(())
            } else {
                Err(BuildError::Workspace(std::io::Error::other(format!(
                    "failed to remove privileged build file `{}`",
                    path.display()
                ))))
            }
        }
        Err(error) => Err(BuildError::Workspace(error)),
    }
}

fn create_symlink(target: &std::path::Path, link: &std::path::Path) -> Result<(), BuildError> {
    match std::os::unix::fs::symlink(target, link) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
            let status = std::process::Command::new("sudo")
                .arg("ln")
                .arg("-s")
                .arg("--")
                .arg(target)
                .arg(link)
                .status()
                .map_err(BuildError::Workspace)?;

            if status.success() {
                Ok(())
            } else {
                Err(BuildError::Workspace(std::io::Error::other(format!(
                    "failed to create privileged build symlink `{}`",
                    link.display()
                ))))
            }
        }
        Err(error) => Err(BuildError::Workspace(error)),
    }
}

fn write_file(path: &std::path::Path, contents: &str) -> Result<(), BuildError> {
    match fs::write(path, contents) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
            let mut child = std::process::Command::new("sudo")
                .arg("tee")
                .arg(path)
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::null())
                .spawn()
                .map_err(BuildError::Workspace)?;

            use std::io::Write;

            child
                .stdin
                .as_mut()
                .ok_or_else(|| {
                    BuildError::Workspace(std::io::Error::other(
                        "failed to open privileged file writer stdin",
                    ))
                })?
                .write_all(contents.as_bytes())
                .map_err(BuildError::Workspace)?;

            let status = child.wait().map_err(BuildError::Workspace)?;

            if status.success() {
                Ok(())
            } else {
                Err(BuildError::Workspace(std::io::Error::other(format!(
                    "failed to write privileged build file `{}`",
                    path.display()
                ))))
            }
        }
        Err(error) => Err(BuildError::Workspace(error)),
    }
}

fn copy_file(source: &std::path::Path, destination: &std::path::Path) -> Result<(), BuildError> {
    match fs::copy(source, destination) {
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
            let status = std::process::Command::new("sudo")
                .arg("cp")
                .arg("--")
                .arg(source)
                .arg(destination)
                .status()
                .map_err(BuildError::Workspace)?;

            if status.success() {
                Ok(())
            } else {
                Err(BuildError::Workspace(std::io::Error::other(format!(
                    "failed to copy privileged file `{}` to `{}`",
                    source.display(),
                    destination.display()
                ))))
            }
        }
        Err(error) => Err(BuildError::Workspace(std::io::Error::new(
            error.kind(),
            format!(
                "failed to copy `{}` to `{}`: {error}",
                source.display(),
                destination.display()
            ),
        ))),
    }
}

fn copy_directory_contents(
    source: &std::path::Path,
    destination: &std::path::Path,
) -> Result<(), BuildError> {
    create_directory(destination)?;

    for entry in fs::read_dir(source).map_err(|error| {
        BuildError::Workspace(std::io::Error::new(
            error.kind(),
            format!("failed to read directory `{}`: {error}", source.display()),
        ))
    })? {
        let entry = entry.map_err(BuildError::Workspace)?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());

        if source_path.is_dir() {
            copy_directory_contents(&source_path, &destination_path)?;
        } else {
            copy_file(&source_path, &destination_path)?;
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
    ) -> IsoPipeline<MmdebstrapBootstrapper, RootfsBackend, SystemLiveRootfsPreparer, IsoBackend>
    {
        IsoPipeline {
            bootstrapper: MmdebstrapBootstrapper::new(),
            rootfs_backend: RootfsBackend::new(
                build_context.rootfs().to_path_buf(),
                build_context.asset_directory().to_path_buf(),
                package_repository.clone(),
            ),
            live_rootfs_preparer: SystemLiveRootfsPreparer,
            iso_backend: IsoBackend::from_context(build_context),
        }
    }
    fn run_with<B, R, L, I>(
        build_context: &BuildContext,
        plan: Plan,
        pipeline: IsoPipeline<B, R, L, I>,
    ) -> Result<Plan, BuildError>
    where
        B: Bootstrapper<Error = MmdebstrapError>,
        R: BuildBackend<Error = RootfsRunError>,
        L: LiveRootfsPreparer,
        I: BuildBackend<Error = std::io::Error>,
    {
        pipeline.execute(build_context, &plan)?;

        Ok(plan)
    }
}
#[cfg(test)]
mod tests {

    use executor::{ExecuteError, RootfsRunError};
    use model::{Action, Capability, CapabilityId, Plan, PlanStep, Provider, ProviderId};
    use registry::{PackageRepository, Registry};
    use std::{
        fs, io,
        sync::{Arc, Mutex},
    };

    use super::{IsoPipeline, IsoWorkflow, LiveRootfsPreparer, SystemLiveRootfsPreparer};
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
        let content_repository_directory = registry_directory.join("content-repositories");

        std::fs::create_dir_all(&asset_directory).expect("asset directory should be created");

        std::fs::create_dir_all(&package_manifest_directory)
            .expect("package manifest directory should be created");

        std::fs::create_dir_all(&provider_directory).expect("provider directory should be created");

        std::fs::create_dir_all(&appliance_profile_directory)
            .expect("appliance profile directory should be created");

        std::fs::create_dir_all(&content_repository_directory)
            .expect("content repository directory should be created");

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
                live_rootfs_preparer: SystemLiveRootfsPreparer,
                iso_backend: RecordingIsoBackend {
                    log: Arc::clone(&log),
                    error: iso_error,
                },
            },
        );

        (result, log)
    }

    #[test]
    fn system_live_rootfs_preparer_can_be_created() {
        let _preparer = SystemLiveRootfsPreparer;
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
        let content_repository_directory = registry_directory.join("content-repositories");

        let daia_binary = temp.path().join("daia");
        let daia_tui_binary = temp.path().join("daia-tui");

        fs::create_dir_all(&rootfs).expect("rootfs should be created");

        fs::create_dir_all(&asset_directory).expect("asset directory should be created");

        fs::create_dir_all(&package_manifest_directory)
            .expect("package manifest directory should be created");

        fs::create_dir_all(&provider_directory).expect("provider directory should be created");

        fs::create_dir_all(&appliance_profile_directory)
            .expect("appliance profile directory should be created");

        fs::create_dir_all(&content_repository_directory)
            .expect("content repository directory should be created");

        fs::write(
            content_repository_directory.join("local-models.yaml"),
            b"content repository",
        )
        .expect("content repository fixture should be written");

        fs::write(&daia_binary, b"daia").expect("DAIA binary fixture should be written");
        fs::write(&daia_tui_binary, b"daia-tui")
            .expect("DAIA TUI binary fixture should be written");

        let getty_wants = rootfs.join("etc/systemd/system/getty.target.wants");
        fs::create_dir_all(&getty_wants).expect("getty wants directory should be created");

        let tty1_getty_link = getty_wants.join("getty@tty1.service");
        std::os::unix::fs::symlink("/usr/lib/systemd/system/getty@.service", &tty1_getty_link)
            .expect("tty1 getty fixture should be created");

        let build_context = BuildContext::new(
            rootfs.clone(),
            temp.path().join("source.iso"),
            temp.path().join("work"),
            temp.path().join("output.iso"),
            asset_directory,
            BootstrapConfig::default(),
        )
        .with_daia_binary(&daia_binary)
        .with_daia_tui_binary(&daia_tui_binary);

        let mut preparer = SystemLiveRootfsPreparer;

        preparer
            .prepare(&build_context)
            .expect("live rootfs preparation should succeed");

        assert_eq!(
            fs::read(rootfs.join("usr/bin/daia")).expect("DAIA binary should be copied"),
            b"daia"
        );

        assert_eq!(
            fs::read(rootfs.join("usr/bin/daia-tui")).expect("DAIA TUI binary should be copied"),
            b"daia-tui"
        );

        let installer_service =
            fs::read_to_string(rootfs.join("etc/systemd/system/daia-installer.service"))
                .expect("DAIA installer service should be readable");

        assert!(
            installer_service.contains("ExecStart=/usr/bin/daia-tui"),
            "live installer service must launch the DAIA terminal installer"
        );
        assert!(
            !installer_service.contains("ExecStart=/usr/bin/daia install"),
            "live installer service must not launch the legacy CLI installer presentation"
        );

        assert_eq!(
            fs::read(rootfs.join("usr/share/daia/content-repositories/local-models.yaml"))
                .expect("content repository should be copied"),
            b"content repository"
        );

        assert!(
            rootfs
                .join("etc/systemd/system/daia-installer.service")
                .is_file()
        );

        assert_eq!(
            fs::read_link(rootfs.join("etc/systemd/system/default.target"))
                .expect("live rootfs default target should be a symlink"),
            std::path::PathBuf::from("/usr/lib/systemd/system/multi-user.target")
        );

        assert!(
            rootfs
                .join("etc/systemd/system/multi-user.target.wants/daia-installer.service")
                .is_symlink()
        );

        assert!(
            !tty1_getty_link.exists(),
            "tty1 getty must be removed before the DAIA installer owns tty1"
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
        let content_repository_directory = registry_directory.join("content-repositories");

        fs::create_dir_all(&rootfs).expect("rootfs should be created");

        fs::create_dir_all(&asset_directory).expect("asset directory should be created");

        fs::create_dir_all(&package_manifest_directory)
            .expect("package manifest directory should be created");

        fs::create_dir_all(&provider_directory).expect("provider directory should be created");

        fs::create_dir_all(&appliance_profile_directory)
            .expect("appliance profile directory should be created");

        fs::create_dir_all(&content_repository_directory)
            .expect("content repository directory should be created");

        let build_context = BuildContext::new(
            rootfs.clone(),
            temp.path().join("source.iso"),
            temp.path().join("work"),
            temp.path().join("output.iso"),
            asset_directory,
            BootstrapConfig::default(),
        );

        let mut preparer = SystemLiveRootfsPreparer;

        preparer
            .prepare(&build_context)
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
