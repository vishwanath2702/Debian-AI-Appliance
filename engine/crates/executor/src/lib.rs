//! Execution of planned system actions.
mod apt_installer;

pub use apt_installer::{AptInstaller, AptInstallerError};
use assets::AssetStore;
use model::{Action, Plan};
use std::{
    error::Error,
    fmt::{self, Display, Formatter},
    fs, io,
    path::PathBuf,
};
/// Error returned when plan execution fails.
#[derive(Debug)]
pub enum ExecuteError<E> {
    /// The execution environment could not be prepared.
    Environment(io::Error),

    /// An action could not be executed.
    Action {
        /// Zero-based index of the failed step.
        step: usize,

        /// Action that failed.
        action: Action,

        /// Error returned by the action runner.
        source: E,
    },
}
impl<E> Display for ExecuteError<E>
where
    E: Display,
{
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Environment(error) => {
                write!(
                    formatter,
                    "failed to prepare execution environment: {error}"
                )
            }
            Self::Action {
                step,
                action,
                source,
            } => write!(
                formatter,
                "failed to execute step {step} ({action}): {source}"
            ),
        }
    }
}

impl<E> Error for ExecuteError<E>
where
    E: Error + 'static,
{
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Environment(error) => Some(error),
            Self::Action { source, .. } => Some(source),
        }
    }
}
/// Manages the lifecycle of an execution environment.
pub trait ExecutionEnvironment {
    /// Prepares the execution environment.
    ///
    /// # Errors
    ///
    /// Returns any I/O error encountered while preparing the environment.
    fn prepare(&self) -> io::Result<()>;
}
/// Installs packages into a root filesystem.
pub trait PackageInstaller {
    /// Installs the requested packages into the supplied root filesystem.
    ///
    /// # Errors
    ///
    /// Returns an error when package installation cannot be completed.
    fn install(
        &self,
        rootfs: &std::path::Path,
        packages: &[String],
    ) -> Result<(), AptInstallerError>;
}

/// Package installer that performs no work.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct NoopPackageInstaller;

impl PackageInstaller for NoopPackageInstaller {
    fn install(
        &self,
        _rootfs: &std::path::Path,
        _packages: &[String],
    ) -> Result<(), AptInstallerError> {
        Ok(())
    }
}
/// Executes one action from a plan.
pub trait ActionRunner {
    /// Error returned when an action cannot be executed.
    type Error;

    /// Executes one action.
    ///
    /// # Errors
    ///
    /// Returns an error when the action cannot be executed.
    fn run(&mut self, action: &Action) -> Result<(), Self::Error>;
}

struct UnavailableAssetStore;

impl AssetStore for UnavailableAssetStore {
    fn read(&self, path: &std::path::Path) -> io::Result<Vec<u8>> {
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("asset store is unavailable for `{}`", path.display()),
        ))
    }
}
pub struct RootfsRunner {
    rootfs: PathBuf,
    package_repository: registry::PackageRepository,
    package_installer: Box<dyn PackageInstaller>,
    asset_store: Box<dyn AssetStore>,
}

impl RootfsRunner {
    /// Creates a root filesystem action runner using default dependencies.
    #[must_use]
    pub fn new(rootfs: PathBuf, package_repository: registry::PackageRepository) -> Self {
        Self::with_dependencies(
            rootfs,
            package_repository,
            Box::new(NoopPackageInstaller),
            Box::new(UnavailableAssetStore),
        )
    }

    /// Creates a root filesystem action runner using the supplied package installer.
    #[must_use]
    pub fn with_installer(
        rootfs: PathBuf,
        package_repository: registry::PackageRepository,
        package_installer: Box<dyn PackageInstaller>,
    ) -> Self {
        Self::with_dependencies(
            rootfs,
            package_repository,
            package_installer,
            Box::new(UnavailableAssetStore),
        )
    }

    /// Creates a root filesystem action runner using supplied dependencies.
    #[must_use]
    pub fn with_dependencies(
        rootfs: PathBuf,
        package_repository: registry::PackageRepository,
        package_installer: Box<dyn PackageInstaller>,
        asset_store: Box<dyn AssetStore>,
    ) -> Self {
        Self {
            rootfs,
            package_repository,
            package_installer,
            asset_store,
        }
    }
}
impl Display for RootfsRunError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::PackageManifestNotFound(name) => {
                write!(formatter, "package manifest `{name}` was not found")
            }
            Self::PackageInstall(error) => {
                write!(formatter, "package installation failed: {error}")
            }
            Self::Asset(error) => {
                write!(formatter, "asset operation failed: {error}")
            }
        }
    }
}

impl Error for RootfsRunError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::PackageManifestNotFound(_) => None,
            Self::PackageInstall(error) => Some(error),
            Self::Asset(error) => Some(error),
        }
    }
}
impl ExecutionEnvironment for RootfsRunner {
    fn prepare(&self) -> io::Result<()> {
        fs::create_dir_all(&self.rootfs)?;
        Ok(())
    }
}

/// Error returned when a root filesystem action cannot be executed.
#[derive(Debug)]
pub enum RootfsRunError {
    /// The requested package manifest does not exist.
    PackageManifestNotFound(String),

    /// Package installation failed.
    PackageInstall(AptInstallerError),

    /// Asset operations failed.
    Asset(io::Error),
}
impl ActionRunner for RootfsRunner {
    type Error = RootfsRunError;

    fn run(&mut self, action: &Action) -> Result<(), Self::Error> {
        match action {
            Action::InstallPackageManifest(name) => {
                let manifest = self
                    .package_repository
                    .manifest(name)
                    .ok_or_else(|| RootfsRunError::PackageManifestNotFound(name.clone()))?;
                self.package_installer
                    .install(&self.rootfs, manifest.packages())
                    .map_err(RootfsRunError::PackageInstall)?;
            }
            Action::CopyAsset { asset, destination } => {
                let contents = self
                    .asset_store
                    .read(std::path::Path::new(asset.as_str()))
                    .map_err(RootfsRunError::Asset)?;

                let destination = destination
                    .strip_prefix(std::path::Path::new("/"))
                    .unwrap_or(destination);
                let destination = self.rootfs.join(destination);

                if let Some(parent) = destination.parent() {
                    fs::create_dir_all(parent).map_err(RootfsRunError::Asset)?;
                }

                fs::write(destination, contents).map_err(RootfsRunError::Asset)?;
            }
            Action::EnableService(service) => {
                println!(
                    "enabling service in rootfs {}: {service}",
                    self.rootfs.display()
                );
            }
        }

        Ok(())
    }
}
/// Executes every action in a plan in its declared order.
pub struct Executor<R> {
    runner: R,
}

impl<R> Executor<R> {
    /// Creates an executor using the supplied action runner.
    #[must_use]
    pub const fn new(runner: R) -> Self {
        Self { runner }
    }

    /// Returns a shared reference to the underlying runner.
    #[must_use]
    pub const fn runner(&self) -> &R {
        &self.runner
    }
}
impl<R> Executor<R>
where
    R: ActionRunner + ExecutionEnvironment,
{
    /// Executes every step in the supplied plan.
    ///
    /// Execution stops on the first error.
    ///
    /// # Errors
    ///
    /// Returns the first error produced by the runner.
    pub fn execute(&mut self, plan: &Plan) -> Result<(), ExecuteError<R::Error>> {
        self.runner.prepare().map_err(ExecuteError::Environment)?;

        for (step, plan_step) in plan.steps.iter().enumerate() {
            self.runner
                .run(&plan_step.action)
                .map_err(|source| ExecuteError::Action {
                    step,
                    action: plan_step.action.clone(),
                    source,
                })?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{ActionRunner, Executor, RootfsRunner};
    use crate::ExecutionEnvironment;
    use assets::AssetStore;
    use model::{Action, Capability, PackageManifest, Plan, PlanStep, ProviderId};
    use std::fs;
    use std::io;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};
    #[derive(Default)]
    struct RecordingRunner {
        actions: Vec<Action>,
    }

    impl ExecutionEnvironment for RecordingRunner {
        fn prepare(&self) -> std::io::Result<()> {
            Ok(())
        }
    }

    impl ActionRunner for RecordingRunner {
        type Error = ();

        fn run(&mut self, action: &Action) -> Result<(), Self::Error> {
            self.actions.push(action.clone());
            Ok(())
        }
    }

    struct FailingRunner;
    struct TestAssetStore {
        contents: Vec<u8>,
    }

    impl AssetStore for TestAssetStore {
        fn read(&self, _: &Path) -> io::Result<Vec<u8>> {
            Ok(self.contents.clone())
        }
    }

    struct FailingAssetStore;

    impl AssetStore for FailingAssetStore {
        fn read(&self, _: &Path) -> io::Result<Vec<u8>> {
            Err(io::Error::new(io::ErrorKind::NotFound, "missing asset"))
        }
    }
    impl ExecutionEnvironment for FailingRunner {
        fn prepare(&self) -> std::io::Result<()> {
            Ok(())
        }
    }

    impl ActionRunner for FailingRunner {
        type Error = &'static str;

        fn run(&mut self, _: &Action) -> Result<(), Self::Error> {
            Err("boom")
        }
    }

    fn plan_with_steps(steps: Vec<PlanStep>) -> Plan {
        Plan {
            capability: Capability::new("desktop"),
            provider: ProviderId::new("desktop"),
            steps,
        }
    }

    #[test]
    fn executes_plan_actions_in_order() {
        let plan = plan_with_steps(vec![
            PlanStep::new(Action::InstallPackageManifest("desktop".to_owned())),
            PlanStep::new(Action::EnableService("display-manager".to_owned())),
        ]);

        let mut executor = Executor::new(RecordingRunner::default());

        executor.execute(&plan).expect("plan should execute");

        assert_eq!(
            executor.runner().actions,
            vec![
                Action::InstallPackageManifest("desktop".to_owned()),
                Action::EnableService("display-manager".to_owned()),
            ]
        );
    }

    #[test]
    fn empty_plan_succeeds() {
        let plan = plan_with_steps(Vec::new());
        let mut executor = Executor::new(RecordingRunner::default());

        executor.execute(&plan).expect("empty plan should execute");

        assert!(executor.runner().actions.is_empty());
    }

    #[test]
    fn returns_execution_context_when_runner_fails() {
        let plan = plan_with_steps(vec![PlanStep::new(Action::InstallPackageManifest(
            "desktop".to_owned(),
        ))]);

        let mut executor = Executor::new(FailingRunner);

        let error = executor.execute(&plan).expect_err("execution should fail");

        match error {
            crate::ExecuteError::Action {
                step,
                action,
                source,
            } => {
                assert_eq!(step, 0);
                assert_eq!(action, Action::InstallPackageManifest("desktop".to_owned()));
                assert_eq!(source, "boom");
            }
            crate::ExecuteError::Environment(_) => {
                panic!("expected action execution error");
            }
        }
    }

    #[test]
    fn rootfs_runner_accepts_actions() {
        let repository = registry::PackageRepository::from_manifests(vec![PackageManifest::new(
            "desktop",
            vec!["vim".to_owned(), "curl".to_owned()],
        )])
        .expect("package repository should be valid");

        let mut runner = RootfsRunner::new(PathBuf::from("/tmp/daia-rootfs"), repository);
        runner
            .run(&Action::InstallPackageManifest("desktop".to_owned()))
            .expect("rootfs runner should succeed");
    }
    #[test]
    fn rootfs_runner_returns_error_for_unknown_manifest() {
        let mut runner = RootfsRunner::new(
            PathBuf::from("/tmp/daia-rootfs"),
            registry::PackageRepository::new(),
        );

        let error = runner
            .run(&Action::InstallPackageManifest("desktop".to_owned()))
            .expect_err("unknown package manifest should fail");

        match error {
            super::RootfsRunError::PackageManifestNotFound(name) => {
                assert_eq!(name, "desktop");
            }
            other @ super::RootfsRunError::PackageInstall(_) => {
                panic!("expected PackageManifestNotFound, got {other:?}");
            }
            super::RootfsRunError::Asset(error) => {
                panic!("expected PackageManifestNotFound, got Asset({error})");
            }
        }
    }
    #[test]
    fn rootfs_runner_copies_asset() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();

        let rootfs = std::env::temp_dir().join(format!("daia-rootfs-{unique}"));

        let mut runner = RootfsRunner::with_dependencies(
            rootfs.clone(),
            registry::PackageRepository::new(),
            Box::new(crate::AptInstaller::new()),
            Box::new(TestAssetStore {
                contents: b"hello world".to_vec(),
            }),
        );

        runner
            .run(&Action::CopyAsset {
                asset: model::AssetId::new("test.txt"),
                destination: PathBuf::from("/etc/test.txt"),
            })
            .expect("copy should succeed");

        assert_eq!(
            fs::read(rootfs.join("etc/test.txt")).unwrap(),
            b"hello world"
        );

        fs::remove_dir_all(rootfs).unwrap();
    }

    #[test]
    fn rootfs_runner_returns_asset_error_when_asset_is_missing() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();

        let rootfs = std::env::temp_dir().join(format!("daia-rootfs-{unique}"));

        let mut runner = RootfsRunner::with_dependencies(
            rootfs,
            registry::PackageRepository::new(),
            Box::new(crate::AptInstaller::new()),
            Box::new(FailingAssetStore),
        );

        let error = runner
            .run(&Action::CopyAsset {
                asset: model::AssetId::new("missing"),
                destination: PathBuf::from("/etc/test.txt"),
            })
            .expect_err("missing asset should fail");

        assert!(matches!(error, super::RootfsRunError::Asset(_)));
    }
    #[test]
    fn prepare_creates_rootfs_directory() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();

        let path = std::env::temp_dir().join(format!("daia-rootfs-{unique}"));

        let runner = RootfsRunner::new(path.clone(), registry::PackageRepository::new());

        runner.prepare().expect("prepare should succeed");

        assert!(path.is_dir());

        fs::remove_dir_all(path).unwrap();
    }
}
