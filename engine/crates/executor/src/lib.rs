//! Execution of planned system actions.
mod apt_installer;

pub use apt_installer::{AptInstaller, AptInstallerError};
use std::fs;
use std::io;
use std::path::PathBuf;
use std::process::Command;

use model::{Action, Plan};

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

/// Manages the lifecycle of an execution environment.
pub trait ExecutionEnvironment {
    /// Prepares the execution environment.
    ///
    /// # Errors
    ///
    /// Returns any I/O error encountered while preparing the environment.
    fn prepare(&self) -> io::Result<()>;

    /// Bootstraps the execution environment.
    ///
    /// # Errors
    ///
    /// Returns any I/O error encountered while bootstrapping the environment.
    fn bootstrap(&self) -> io::Result<()>;
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

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RootfsRunner {
    rootfs: PathBuf,
    package_repository: registry::PackageRepository,
    package_installer: NoopPackageInstaller,
}

impl RootfsRunner {
    /// Creates a root filesystem action runner.
    #[must_use]
    pub const fn new(rootfs: PathBuf, package_repository: registry::PackageRepository) -> Self {
        Self {
            rootfs,
            package_repository,
            package_installer: NoopPackageInstaller,
        }
    }
}

impl ExecutionEnvironment for RootfsRunner {
    fn prepare(&self) -> io::Result<()> {
        fs::create_dir_all(&self.rootfs)?;
        Ok(())
    }

    fn bootstrap(&self) -> io::Result<()> {
        let status = Command::new("debootstrap")
            .arg("--variant=minbase")
            .arg("bookworm")
            .arg(&self.rootfs)
            .status()?;

        if status.success() {
            Ok(())
        } else {
            Err(io::Error::other(format!(
                "debootstrap exited with status {status}"
            )))
        }
    }
}
/// Error returned when a root filesystem action cannot be executed.
#[derive(Debug)]
pub enum RootfsRunError {
    /// The requested package manifest does not exist.
    PackageManifestNotFound(String),

    /// Package installation failed.
    PackageInstall(AptInstallerError),
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
        self.runner.bootstrap().map_err(ExecuteError::Environment)?;

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
    use model::{Action, Capability, PackageManifest, Plan, PlanStep, ProviderId};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};
    #[derive(Default)]
    struct RecordingRunner {
        actions: Vec<Action>,
    }

    impl ExecutionEnvironment for RecordingRunner {
        fn prepare(&self) -> std::io::Result<()> {
            Ok(())
        }

        fn bootstrap(&self) -> std::io::Result<()> {
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

    impl ExecutionEnvironment for FailingRunner {
        fn prepare(&self) -> std::io::Result<()> {
            Ok(())
        }

        fn bootstrap(&self) -> std::io::Result<()> {
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
        }
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
