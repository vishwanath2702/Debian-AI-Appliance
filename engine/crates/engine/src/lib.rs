//! High-level orchestration for the DAIA engine.

mod bootstrap;
mod bootstrapper;
mod context;
mod installation;
mod mmdebstrap;
mod workflow;

pub use installation::{
    DryRunInstallationExecutor, InstallationCommandRunner, InstallationExecutor,
    InstallationOperation, InstallationOperationExecutor, InstallationPlan, PreparedInstallation,
    ProcessInstallationCommandRunner, SystemInstallationOperationExecutor,
};

use std::{
    error::Error,
    fmt::{self, Display, Formatter},
    path::PathBuf,
};

pub use bootstrap::BootstrapConfig;
pub use bootstrapper::Bootstrapper;
pub use context::BuildContext;
use executor::{ExecuteError, RootfsRunError};
use inspector::{StorageInspectError, StorageInspector};
pub use mmdebstrap::{MmdebstrapBootstrapper, MmdebstrapError};
use model::{
    ApplianceProfile, Capability, DiscoveredStorage, InstallationIntent, Plan, StorageKind,
};
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
    /// Builds execution plans for an appliance profile.
    ///
    /// # Errors
    ///
    /// Returns a [`PlanError`] if any capability in the profile cannot
    /// be satisfied by a provider.
    pub fn plan_profile(&self, profile: &ApplianceProfile) -> Result<Vec<Plan>, PlanError> {
        self.planner.build_profile(profile)
    }

    /// Plans an installation intent against the supplied appliance profile.
    ///
    /// # Errors
    ///
    /// Returns a [`PlanError`] if the selected appliance profile cannot be planned.
    pub fn plan_installation(
        &self,
        intent: &InstallationIntent,
        profile: &ApplianceProfile,
    ) -> Result<Vec<Plan>, PlanError> {
        debug_assert_eq!(intent.profile_name(), profile.name());

        self.plan_profile(profile)
    }

    /// Discovers storage devices using the supplied storage inspector.
    ///
    /// # Errors
    ///
    /// Returns a [`StorageInspectError`] if storage discovery fails.
    pub fn discover_storage<I>(
        &self,
        inspector: &I,
    ) -> Result<Vec<DiscoveredStorage>, StorageInspectError>
    where
        I: StorageInspector,
    {
        inspector.inspect()
    }

    /// Validates and resolves storage selected for an installation.
    ///
    /// # Errors
    ///
    /// Returns an error if the selected storage is unavailable or is the
    /// currently running system disk.
    pub fn validate_installation_storage<'a>(
        &self,
        intent: &InstallationIntent,
        storage: &'a [DiscoveredStorage],
    ) -> Result<&'a DiscoveredStorage, String> {
        let selected = storage
            .iter()
            .find(|storage| storage.id() == intent.storage_id())
            .ok_or_else(|| {
                format!(
                    "selected storage {} is no longer available",
                    intent.storage_id()
                )
            })?;

        if selected.kind() == StorageKind::System {
            return Err(format!(
                "selected storage {} is the system disk",
                selected.id()
            ));
        }

        Ok(selected)
    }

    /// Validates and prepares an installation for later execution.
    ///
    /// # Errors
    ///
    /// Returns a [`PlanError`] if the appliance profile cannot be planned.
    /// Returns a storage-validation error if the selected storage is unavailable
    /// or is the currently running system disk.
    pub fn prepare_installation(
        &self,
        intent: InstallationIntent,
        profile: &ApplianceProfile,
        storage: &[DiscoveredStorage],
    ) -> Result<PreparedInstallation, String> {
        let selected = self
            .validate_installation_storage(&intent, storage)?
            .clone();

        let plans = self
            .plan_installation(&intent, profile)
            .map_err(|error| error.to_string())?;

        Ok(PreparedInstallation::new(intent, selected, plans))
    }

    /// Executes a prepared installation using the supplied executor.
    ///
    /// # Errors
    ///
    /// Returns the executor error if installation execution fails.
    pub fn execute_installation<E>(
        &self,
        installation: &PreparedInstallation,
        executor: &mut E,
    ) -> Result<(), E::Error>
    where
        E: InstallationExecutor,
    {
        executor.execute(installation)
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

    use inspector::{StorageInspectError, StorageInspector};
    use model::{
        Action, ApplianceProfile, Capability, CapabilityId, DiscoveredStorage, DiscoveredStorageId,
        InstallationIntent, PlanStep, Provider, ProviderId, StorageKind,
    };
    use registry::{PackageRepository, Registry};

    use super::{
        BootstrapConfig, BuildContext, BuildError, DryRunInstallationExecutor, Engine,
        InstallationExecutor, InstallationOperation, InstallationOperationExecutor,
        InstallationPlan, PreparedInstallation, RootfsRunError,
    };
    struct TestStorageInspector;

    impl StorageInspector for TestStorageInspector {
        fn inspect(&self) -> Result<Vec<DiscoveredStorage>, StorageInspectError> {
            Ok(vec![
                DiscoveredStorage::new("wwn:system-disk", StorageKind::System, "/dev/sda"),
                DiscoveredStorage::new("serial:usb-disk", StorageKind::Removable, "/dev/sdb"),
            ])
        }
    }

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

    struct RecordingInstallationExecutor {
        executed: bool,
    }

    impl InstallationExecutor for RecordingInstallationExecutor {
        type Error = std::convert::Infallible;

        fn execute(&mut self, _installation: &PreparedInstallation) -> Result<(), Self::Error> {
            self.executed = true;
            Ok(())
        }
    }

    struct RecordingOperationExecutor {
        operations: Vec<InstallationOperation>,
    }

    impl InstallationOperationExecutor for RecordingOperationExecutor {
        type Error = std::convert::Infallible;

        fn execute_operation(
            &mut self,
            operation: &InstallationOperation,
        ) -> Result<(), Self::Error> {
            self.operations.push(operation.clone());
            Ok(())
        }
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum RecordingOperationError {
        Failed,
    }

    struct FailingOperationExecutor {
        operations: Vec<InstallationOperation>,
        fail_at: usize,
    }

    impl InstallationOperationExecutor for FailingOperationExecutor {
        type Error = RecordingOperationError;

        fn execute_operation(
            &mut self,
            operation: &InstallationOperation,
        ) -> Result<(), Self::Error> {
            if self.operations.len() == self.fail_at {
                return Err(RecordingOperationError::Failed);
            }

            self.operations.push(operation.clone());
            Ok(())
        }
    }

    #[test]
    fn installation_plan_stops_after_operation_failure() {
        let plan = InstallationPlan::new(vec![
            InstallationOperation::PrepareDisk {
                storage_id: DiscoveredStorageId::new("serial:usb-disk"),
                device_path: "/dev/sdb".into(),
            },
            InstallationOperation::CreateFilesystems {
                filesystem: "ext4".to_owned(),
            },
            InstallationOperation::MountFilesystems {
                mount_point: "/target".to_owned(),
            },
            InstallationOperation::BootstrapSystem {
                root: "/target".to_owned(),
            },
            InstallationOperation::ApplyPlans { count: 1 },
        ]);

        let mut executor = FailingOperationExecutor {
            operations: Vec::new(),
            fail_at: 2,
        };

        let error = plan
            .execute(&mut executor)
            .expect_err("installation plan should stop on operation failure");

        assert_eq!(error, RecordingOperationError::Failed);
        assert_eq!(
            executor.operations,
            vec![
                InstallationOperation::PrepareDisk {
                    storage_id: DiscoveredStorageId::new("serial:usb-disk"),
                    device_path: "/dev/sdb".into(),
                },
                InstallationOperation::CreateFilesystems {
                    filesystem: "ext4".to_owned(),
                },
            ]
        );
    }
    #[test]
    fn dry_run_executor_records_executed_operations_in_order() {
        let engine = Engine::from_registry(desktop_registry());

        let intent =
            InstallationIntent::new("desktop", DiscoveredStorageId::new("serial:usb-disk"));

        let storage = DiscoveredStorage::new("serial:usb-disk", StorageKind::Removable, "/dev/sdb");

        let prepared = PreparedInstallation::new(intent, storage, Vec::new());

        let mut executor = DryRunInstallationExecutor::default();

        engine
            .execute_installation(&prepared, &mut executor)
            .expect("dry-run installation should execute");

        let expected = prepared.installation_plan();

        assert_eq!(executor.executed_operations(), expected.operations());
    }

    #[test]
    fn installation_plan_executes_operations_in_order() {
        let plan = InstallationPlan::new(vec![
            InstallationOperation::PrepareDisk {
                storage_id: DiscoveredStorageId::new("serial:usb-disk"),
                device_path: "/dev/sdb".into(),
            },
            InstallationOperation::CreateFilesystems {
                filesystem: "ext4".to_owned(),
            },
            InstallationOperation::MountFilesystems {
                mount_point: "/target".to_owned(),
            },
            InstallationOperation::BootstrapSystem {
                root: "/target".to_owned(),
            },
            InstallationOperation::ApplyPlans { count: 1 },
        ]);

        let expected = plan.operations().to_vec();

        let mut executor = RecordingOperationExecutor {
            operations: Vec::new(),
        };

        plan.execute(&mut executor)
            .expect("installation plan should execute");

        assert_eq!(executor.operations, expected);
    }

    #[test]
    fn records_installation_operations_through_executor() {
        let mut executor = RecordingOperationExecutor {
            operations: Vec::new(),
        };

        let operation = InstallationOperation::PrepareDisk {
            storage_id: DiscoveredStorageId::new("serial:usb-disk"),
            device_path: "/dev/sdb".into(),
        };
        executor
            .execute_operation(&operation)
            .expect("recording executor should accept operation");

        assert_eq!(executor.operations, vec![operation]);
    }

    #[test]
    fn dry_run_executor_records_installation_operations() {
        let engine = Engine::from_registry(desktop_registry());

        let intent =
            InstallationIntent::new("desktop", DiscoveredStorageId::new("serial:usb-disk"));

        let storage = DiscoveredStorage::new("serial:usb-disk", StorageKind::Removable, "/dev/sdb");

        let prepared = PreparedInstallation::new(intent, storage, Vec::new());

        let mut executor = DryRunInstallationExecutor::default();

        engine
            .execute_installation(&prepared, &mut executor)
            .expect("dry-run installation should execute");

        let plan = executor
            .plan()
            .expect("dry-run executor should record installation plan");

        assert_eq!(
            plan.operations(),
            &[
                InstallationOperation::PrepareDisk {
                    storage_id: DiscoveredStorageId::new("serial:usb-disk"),
                    device_path: "/dev/sdb".into(),
                },
                InstallationOperation::CreateFilesystems {
                    filesystem: "ext4".to_owned(),
                },
                InstallationOperation::MountFilesystems {
                    mount_point: "/target".to_owned(),
                },
                InstallationOperation::BootstrapSystem {
                    root: "/target".to_owned(),
                },
                InstallationOperation::ApplyPlans { count: 0 }
            ]
        );
    }

    #[test]
    fn installation_plan_preserves_operation_order() {
        let plan = InstallationPlan::new(vec![
            InstallationOperation::PrepareDisk {
                storage_id: DiscoveredStorageId::new("serial:usb-disk"),
                device_path: "/dev/sdb".into(),
            },
            InstallationOperation::CreateFilesystems {
                filesystem: "ext4".to_owned(),
            },
            InstallationOperation::MountFilesystems {
                mount_point: "/target".to_owned(),
            },
            InstallationOperation::BootstrapSystem {
                root: "/target".to_owned(),
            },
            InstallationOperation::ApplyPlans { count: 0 },
        ]);

        assert_eq!(
            plan.operations(),
            &[
                InstallationOperation::PrepareDisk {
                    storage_id: DiscoveredStorageId::new("serial:usb-disk"),
                    device_path: "/dev/sdb".into(),
                },
                InstallationOperation::CreateFilesystems {
                    filesystem: "ext4".to_owned(),
                },
                InstallationOperation::MountFilesystems {
                    mount_point: "/target".to_owned(),
                },
                InstallationOperation::BootstrapSystem {
                    root: "/target".to_owned(),
                },
                InstallationOperation::ApplyPlans { count: 0 }
            ]
        );
    }

    #[test]
    fn dry_run_executor_records_installation_summary() {
        let engine = Engine::from_registry(desktop_registry());

        let intent =
            InstallationIntent::new("desktop", DiscoveredStorageId::new("serial:usb-disk"));

        let storage = DiscoveredStorage::new("serial:usb-disk", StorageKind::Removable, "/dev/sdb");

        let prepared = PreparedInstallation::new(intent, storage, Vec::new());

        let mut executor = DryRunInstallationExecutor::default();

        engine
            .execute_installation(&prepared, &mut executor)
            .expect("dry-run installation should execute");

        assert_eq!(
            executor.summary(),
            Some(
                "Profile: desktop\nStorage: serial:usb-disk (removable)\nDevice: /dev/sdb\nPlans: 0"
            )
        );
    }

    #[test]
    fn executes_prepared_installation_through_executor() {
        let engine = Engine::from_registry(desktop_registry());

        let intent =
            InstallationIntent::new("desktop", DiscoveredStorageId::new("serial:usb-disk"));

        let storage = DiscoveredStorage::new("serial:usb-disk", StorageKind::Removable, "/dev/sdb");

        let prepared = PreparedInstallation::new(intent, storage, Vec::new());

        let mut executor = RecordingInstallationExecutor { executed: false };

        engine
            .execute_installation(&prepared, &mut executor)
            .expect("prepared installation should execute");

        assert!(executor.executed);
    }

    #[test]
    fn prepared_installation_exposes_dry_run_summary() {
        let intent =
            InstallationIntent::new("desktop", DiscoveredStorageId::new("serial:usb-disk"));

        let storage = DiscoveredStorage::new("serial:usb-disk", StorageKind::Removable, "/dev/sdb");

        let prepared = PreparedInstallation::new(intent, storage, Vec::new());

        assert_eq!(
            prepared.summary(),
            "Profile: desktop\nStorage: serial:usb-disk (removable)\nDevice: /dev/sdb\nPlans: 0"
        );
    }

    #[test]
    fn prepare_installation_rejects_missing_storage() {
        let engine = Engine::from_registry(desktop_registry());

        let profile = ApplianceProfile::new(
            "desktop",
            "Graphical desktop appliance",
            vec![Capability::new("desktop")],
        );

        let intent =
            InstallationIntent::new("desktop", DiscoveredStorageId::new("serial:missing-disk"));

        let error = engine
            .prepare_installation(intent, &profile, &[])
            .expect_err("missing installation storage should fail");

        assert!(error.contains("no longer available"));
    }

    #[test]
    fn prepare_installation_rejects_system_disk() {
        let engine = Engine::from_registry(desktop_registry());

        let profile = ApplianceProfile::new(
            "desktop",
            "Graphical desktop appliance",
            vec![Capability::new("desktop")],
        );

        let intent =
            InstallationIntent::new("desktop", DiscoveredStorageId::new("wwn:system-disk"));

        let storage = vec![DiscoveredStorage::new(
            "wwn:system-disk",
            StorageKind::System,
            "/dev/sda",
        )];

        let error = engine
            .prepare_installation(intent, &profile, &storage)
            .expect_err("system disk installation should fail");

        assert!(error.contains("system disk"));
    }

    #[test]
    fn prepares_valid_installation() {
        let engine = Engine::from_registry(desktop_registry());

        let profile = ApplianceProfile::new(
            "desktop",
            "Graphical desktop appliance",
            vec![Capability::new("desktop")],
        );

        let intent =
            InstallationIntent::new("desktop", DiscoveredStorageId::new("serial:usb-disk"));

        let storage = vec![
            DiscoveredStorage::new("wwn:system-disk", StorageKind::System, "/dev/sda"),
            DiscoveredStorage::new("serial:usb-disk", StorageKind::Removable, "/dev/sdb"),
        ];

        let prepared = engine
            .prepare_installation(intent, &profile, &storage)
            .expect("installation should prepare");

        assert_eq!(prepared.intent().profile_name(), "desktop");
        assert_eq!(prepared.storage().kind(), StorageKind::Removable);
        assert_eq!(
            prepared.storage().device_path(),
            std::path::Path::new("/dev/sdb")
        );
        assert_eq!(prepared.plans().len(), 1);
    }

    #[test]
    fn rejects_system_disk_as_installation_storage() {
        let engine = Engine::from_registry(desktop_registry());

        let intent =
            InstallationIntent::new("desktop", DiscoveredStorageId::new("wwn:system-disk"));

        let storage = vec![DiscoveredStorage::new(
            "wwn:system-disk",
            StorageKind::System,
            "/dev/sda",
        )];

        let error = engine
            .validate_installation_storage(&intent, &storage)
            .expect_err("system disk should not be a valid installation target");

        assert!(error.contains("system disk"));
    }

    #[test]
    fn validates_removable_installation_storage() {
        let engine = Engine::from_registry(desktop_registry());

        let intent =
            InstallationIntent::new("desktop", DiscoveredStorageId::new("serial:usb-disk"));

        let storage = vec![
            DiscoveredStorage::new("wwn:system-disk", StorageKind::System, "/dev/sda"),
            DiscoveredStorage::new("serial:usb-disk", StorageKind::Removable, "/dev/sdb"),
        ];

        let selected = engine
            .validate_installation_storage(&intent, &storage)
            .expect("removable installation storage should be valid");

        assert_eq!(selected.id(), intent.storage_id());
        assert_eq!(selected.kind(), StorageKind::Removable);
    }

    #[test]
    fn plans_installation_intent() {
        let engine = Engine::from_registry(desktop_registry());

        let profile = ApplianceProfile::new(
            "desktop",
            "Graphical desktop appliance",
            vec![Capability::new("desktop")],
        );

        let intent =
            InstallationIntent::new("desktop", DiscoveredStorageId::new("serial:usb-disk"));

        let plans = engine
            .plan_installation(&intent, &profile)
            .expect("installation intent should plan");

        assert_eq!(plans.len(), 1);
        assert_eq!(plans[0].capability, Capability::new("desktop"));
    }

    #[test]
    fn discovers_storage_through_inspector() {
        let engine = Engine::from_registry(desktop_registry());

        let storage = engine
            .discover_storage(&TestStorageInspector)
            .expect("storage discovery should succeed");

        assert_eq!(storage.len(), 2);
        assert_eq!(storage[0].kind(), StorageKind::System);
        assert_eq!(storage[1].kind(), StorageKind::Removable);
    }
    #[test]
    fn builds_plans_for_appliance_profile() {
        let engine = Engine::from_registry(desktop_registry());

        let profile = ApplianceProfile::new(
            "desktop",
            "Graphical desktop appliance",
            vec![Capability::new("desktop")],
        );

        let plans = engine
            .plan_profile(&profile)
            .expect("desktop profile plans should build");

        assert_eq!(plans.len(), 1);
        assert_eq!(plans[0].capability, Capability::new("desktop"));
        assert_eq!(plans[0].provider, ProviderId::new("desktop"));
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
