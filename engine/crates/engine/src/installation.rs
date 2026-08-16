use model::{DiscoveredStorage, DiscoveredStorageId, InstallationIntent, Plan};

use std::{io, path::PathBuf, process::Command};

/// Role of a partition in an installed DAIA system.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InstallationPartitionRole {
    /// EFI System Partition used for UEFI boot.
    EfiSystem,

    /// Root filesystem containing the installed appliance.
    Root,
}
/// Describes one partition required by an installation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstallationPartition {
    role: InstallationPartitionRole,
    filesystem: String,
    size_mib: Option<u64>,
}
impl InstallationPartition {
    /// Creates an installation partition description.
    #[must_use]
    pub fn new(
        role: InstallationPartitionRole,
        filesystem: impl Into<String>,
        size_mib: Option<u64>,
    ) -> Self {
        Self {
            role,
            filesystem: filesystem.into(),
            size_mib,
        }
    }

    /// Returns the partition role.
    #[must_use]
    pub const fn role(&self) -> InstallationPartitionRole {
        self.role
    }

    /// Returns the filesystem type.
    #[must_use]
    pub fn filesystem(&self) -> &str {
        &self.filesystem
    }
    /// Returns the requested partition size in MiB.
    ///
    /// `None` means the partition should consume the remaining space.
    #[must_use]
    pub const fn size_mib(&self) -> Option<u64> {
        self.size_mib
    }
}

/// Returns the default DAIA installation partition layout.
#[must_use]
pub fn default_installation_partitions() -> Vec<InstallationPartition> {
    vec![
        InstallationPartition::new(InstallationPartitionRole::EfiSystem, "fat32", Some(512)),
        InstallationPartition::new(InstallationPartitionRole::Root, "ext4", None),
    ]
}
/// A non-executed operation required to prepare an installation target.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InstallationOperation {
    /// Prepare the selected physical disk for installation.
    PrepareDisk {
        /// Stable identifier of the selected physical storage.
        storage_id: DiscoveredStorageId,

        /// Validated Linux device path for the selected storage.
        device_path: PathBuf,
    },
    /// Create the partition layout on the selected disk.
    PartitionDisk {
        /// Validated Linux device path for the selected storage.
        device_path: PathBuf,

        /// Partition layout to create on the selected disk.
        partitions: Vec<InstallationPartition>,
    },

    /// Create the filesystems required by the installed system.
    CreateFilesystems {
        partitions: Vec<InstallationPartition>,
    },

    /// Mount the prepared target filesystems.
    MountFilesystems { mount_point: String },

    /// Bootstrap the base operating system.
    BootstrapSystem { root: String },

    /// Apply the appliance execution plans.
    ApplyPlans { count: usize },
}
/// Executes one planned installation operation.
pub trait InstallationOperationExecutor {
    /// Error produced while executing an operation.
    type Error;

    /// Executes one installation operation.
    ///
    /// # Errors
    ///
    /// Returns an executor-specific error if the operation fails.
    fn execute_operation(&mut self, operation: &InstallationOperation) -> Result<(), Self::Error>;
}
/// Executes installation operations against the host system.
pub trait InstallationCommandRunner {
    fn status(&mut self, command: &mut Command) -> io::Result<()>;
}

/// Runs installation commands as operating-system processes.
#[derive(Clone, Copy, Debug, Default)]
pub struct ProcessInstallationCommandRunner;

impl InstallationCommandRunner for ProcessInstallationCommandRunner {
    fn status(&mut self, command: &mut Command) -> io::Result<()> {
        let status = command.status()?;

        if status.success() {
            Ok(())
        } else {
            Err(io::Error::other(format!(
                "installation command exited unsuccessfully: {status}"
            )))
        }
    }
}

pub struct SystemInstallationOperationExecutor<R> {
    runner: R,
}

impl<R> SystemInstallationOperationExecutor<R>
where
    R: InstallationCommandRunner,
{
    const fn with_runner(runner: R) -> Self {
        Self { runner }
    }
}

impl SystemInstallationOperationExecutor<ProcessInstallationCommandRunner> {
    /// Creates a system installation executor using real process execution.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            runner: ProcessInstallationCommandRunner,
        }
    }
}

impl<R> InstallationOperationExecutor for SystemInstallationOperationExecutor<R>
where
    R: InstallationCommandRunner,
{
    type Error = io::Error;

    fn execute_operation(&mut self, operation: &InstallationOperation) -> Result<(), Self::Error> {
        match operation {
            InstallationOperation::PrepareDisk { device_path, .. } => {
                let mut command = Command::new("wipefs");

                command.arg("--all").arg(device_path);

                self.runner.status(&mut command)
            }
            InstallationOperation::PartitionDisk {
                device_path,
                partitions,
            } => {
                let mut command = Command::new("parted");

                command
                    .arg("--script")
                    .arg(device_path)
                    .arg("mklabel")
                    .arg("gpt");

                if let Some(efi_partition) = partitions
                    .iter()
                    .find(|partition| partition.role() == InstallationPartitionRole::EfiSystem)
                {
                    if let Some(size_mib) = efi_partition.size_mib() {
                        command
                            .arg("mkpart")
                            .arg("ESP")
                            .arg(efi_partition.filesystem())
                            .arg("1MiB")
                            .arg(format!("{}MiB", size_mib + 1));
                    }
                }

                if let Some(root_partition) = partitions
                    .iter()
                    .find(|partition| partition.role() == InstallationPartitionRole::Root)
                {
                    let root_start_mib = partitions
                        .iter()
                        .find(|partition| partition.role() == InstallationPartitionRole::EfiSystem)
                        .and_then(InstallationPartition::size_mib)
                        .map_or(1, |size_mib| size_mib + 1);

                    command
                        .arg("mkpart")
                        .arg("primary")
                        .arg(root_partition.filesystem())
                        .arg(format!("{root_start_mib}MiB"))
                        .arg("100%");
                }

                self.runner.status(&mut command)
            }
            _ => Ok(()),
        }
    }
}
/// Ordered non-executed operations for installing an appliance.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstallationPlan {
    operations: Vec<InstallationOperation>,
}

impl InstallationPlan {
    /// Creates an installation plan.
    #[must_use]
    pub const fn new(operations: Vec<InstallationOperation>) -> Self {
        Self { operations }
    }

    /// Returns the ordered installation operations.
    #[must_use]
    pub fn operations(&self) -> &[InstallationOperation] {
        &self.operations
    }

    /// Executes the planned operations in order.
    ///
    /// # Errors
    ///
    /// Returns the first error produced by the operation executor.
    pub fn execute<E>(&self, executor: &mut E) -> Result<(), E::Error>
    where
        E: InstallationOperationExecutor,
    {
        for operation in &self.operations {
            executor.execute_operation(operation)?;
        }

        Ok(())
    }
}
/// A validated installation ready for later execution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedInstallation {
    intent: InstallationIntent,
    storage: DiscoveredStorage,
    plans: Vec<Plan>,
}

impl PreparedInstallation {
    /// Creates a prepared installation from validated components.
    #[must_use]
    pub const fn new(
        intent: InstallationIntent,
        storage: DiscoveredStorage,
        plans: Vec<Plan>,
    ) -> Self {
        Self {
            intent,
            storage,
            plans,
        }
    }

    /// Builds the ordered installation-operation plan.
    #[must_use]
    pub fn installation_plan(&self) -> InstallationPlan {
        InstallationPlan::new(vec![
            InstallationOperation::PrepareDisk {
                storage_id: self.intent.storage_id().clone(),
                device_path: self.storage.device_path().to_path_buf(),
            },
            InstallationOperation::PartitionDisk {
                device_path: self.storage.device_path().to_path_buf(),
                partitions: default_installation_partitions(),
            },
            InstallationOperation::CreateFilesystems {
                partitions: default_installation_partitions(),
            },
            InstallationOperation::MountFilesystems {
                mount_point: "/target".to_owned(),
            },
            InstallationOperation::BootstrapSystem {
                root: "/target".to_owned(),
            },
            InstallationOperation::ApplyPlans {
                count: self.plans.len(),
            },
        ])
    }

    /// Returns the confirmed installation intent.
    #[must_use]
    pub const fn intent(&self) -> &InstallationIntent {
        &self.intent
    }

    /// Returns the validated installation storage.
    #[must_use]
    pub const fn storage(&self) -> &DiscoveredStorage {
        &self.storage
    }

    /// Returns the execution plans for the installation.
    #[must_use]
    pub fn plans(&self) -> &[Plan] {
        &self.plans
    }

    /// Returns a human-readable dry-run summary.
    #[must_use]
    pub fn summary(&self) -> String {
        format!(
            "Profile: {}\nStorage: {} ({})\nDevice: {}\nPlans: {}",
            self.intent.profile_name(),
            self.intent.storage_id(),
            self.storage.kind(),
            self.storage.device_path().display(),
            self.plans.len()
        )
    }
}
/// Executes a prepared installation.
pub trait InstallationExecutor {
    /// Error produced by the executor.
    type Error;

    /// Executes the prepared installation.
    ///
    /// # Errors
    ///
    /// Returns an executor-specific error if execution fails.
    fn execute(&mut self, installation: &PreparedInstallation) -> Result<(), Self::Error>;
}
/// Non-destructive installation executor used for validation and previews.
#[derive(Clone, Debug, Default)]
pub struct DryRunInstallationExecutor {
    summary: Option<String>,
    plan: Option<InstallationPlan>,
    executed_operations: Vec<InstallationOperation>,
}

impl DryRunInstallationExecutor {
    /// Returns the summary recorded during the most recent execution.
    #[must_use]
    pub fn summary(&self) -> Option<&str> {
        self.summary.as_deref()
    }

    /// Returns the installation plan recorded during the most recent execution.
    #[must_use]
    pub const fn plan(&self) -> Option<&InstallationPlan> {
        self.plan.as_ref()
    }

    /// Returns operations recorded through the execution pipeline.
    #[must_use]
    pub fn executed_operations(&self) -> &[InstallationOperation] {
        &self.executed_operations
    }
}

impl InstallationExecutor for DryRunInstallationExecutor {
    type Error = std::convert::Infallible;

    fn execute(&mut self, installation: &PreparedInstallation) -> Result<(), Self::Error> {
        self.summary = Some(installation.summary());

        let plan = installation.installation_plan();
        plan.execute(self)?;

        self.plan = Some(plan);

        Ok(())
    }
}

impl InstallationOperationExecutor for DryRunInstallationExecutor {
    type Error = std::convert::Infallible;

    fn execute_operation(&mut self, operation: &InstallationOperation) -> Result<(), Self::Error> {
        self.executed_operations.push(operation.clone());
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::{
        InstallationCommandRunner, InstallationOperation, InstallationOperationExecutor,
        InstallationPartition, InstallationPartitionRole, ProcessInstallationCommandRunner,
        SystemInstallationOperationExecutor, default_installation_partitions,
    };
    use model::DiscoveredStorageId;

    use std::{io, process::Command};

    #[derive(Default)]
    struct RecordingCommandRunner {
        commands: Vec<Vec<String>>,
    }

    struct FailingCommandRunner;

    impl InstallationCommandRunner for FailingCommandRunner {
        fn status(&mut self, _command: &mut Command) -> io::Result<()> {
            Err(io::Error::other("command failed"))
        }
    }

    impl InstallationCommandRunner for RecordingCommandRunner {
        fn status(&mut self, command: &mut Command) -> io::Result<()> {
            let mut recorded = vec![command.get_program().to_string_lossy().into_owned()];

            recorded.extend(
                command
                    .get_args()
                    .map(|arg| arg.to_string_lossy().into_owned()),
            );

            self.commands.push(recorded);

            Ok(())
        }
    }

    #[test]
    fn installation_partition_describes_role_and_filesystem() {
        let partition =
            InstallationPartition::new(InstallationPartitionRole::EfiSystem, "fat32", Some(512));

        assert_eq!(partition.role(), InstallationPartitionRole::EfiSystem);
        assert_eq!(partition.filesystem(), "fat32");
        assert_eq!(partition.size_mib(), Some(512));
    }

    #[test]
    fn installation_partition_can_use_remaining_space() {
        let partition = InstallationPartition::new(InstallationPartitionRole::Root, "ext4", None);

        assert_eq!(partition.role(), InstallationPartitionRole::Root);
        assert_eq!(partition.filesystem(), "ext4");
        assert_eq!(partition.size_mib(), None);
    }

    #[test]
    fn default_installation_partitions_define_efi_and_root() {
        let partitions = default_installation_partitions();

        assert_eq!(partitions.len(), 2);

        assert_eq!(partitions[0].role(), InstallationPartitionRole::EfiSystem);
        assert_eq!(partitions[0].filesystem(), "fat32");
        assert_eq!(partitions[0].size_mib(), Some(512));

        assert_eq!(partitions[1].role(), InstallationPartitionRole::Root);
        assert_eq!(partitions[1].filesystem(), "ext4");
        assert_eq!(partitions[1].size_mib(), None);
    }

    #[test]
    fn process_command_runner_accepts_successful_command() {
        let mut runner = ProcessInstallationCommandRunner;
        let mut command = Command::new("true");

        runner
            .status(&mut command)
            .expect("successful command should succeed");
    }

    #[test]
    fn process_command_runner_rejects_unsuccessful_command() {
        let mut runner = ProcessInstallationCommandRunner;
        let mut command = Command::new("false");

        runner
            .status(&mut command)
            .expect_err("unsuccessful command should fail");
    }

    #[test]
    fn creates_system_executor_with_command_runner() {
        let executor =
            SystemInstallationOperationExecutor::with_runner(RecordingCommandRunner::default());

        assert!(executor.runner.commands.is_empty());
    }
    #[test]
    fn system_executor_sends_wipefs_command_for_prepare_disk() {
        let mut executor =
            SystemInstallationOperationExecutor::with_runner(RecordingCommandRunner::default());

        let operation = InstallationOperation::PrepareDisk {
            storage_id: DiscoveredStorageId::new("serial:usb-disk"),
            device_path: "/dev/sdb".into(),
        };

        executor
            .execute_operation(&operation)
            .expect("recording runner should accept command");

        assert_eq!(
            executor.runner.commands,
            vec![vec![
                "wipefs".to_owned(),
                "--all".to_owned(),
                "/dev/sdb".to_owned(),
            ]]
        );
    }
    #[test]
    fn system_executor_returns_prepare_disk_command_failure() {
        let mut executor = SystemInstallationOperationExecutor::with_runner(FailingCommandRunner);

        let operation = InstallationOperation::PrepareDisk {
            storage_id: DiscoveredStorageId::new("serial:usb-disk"),
            device_path: "/dev/sdb".into(),
        };

        let error = executor
            .execute_operation(&operation)
            .expect_err("prepare disk should return command failure");

        assert_eq!(error.kind(), io::ErrorKind::Other);
    }
    #[test]
    fn system_executor_sends_parted_command_for_partition_disk() {
        let mut executor =
            SystemInstallationOperationExecutor::with_runner(RecordingCommandRunner::default());

        let operation = InstallationOperation::PartitionDisk {
            device_path: "/dev/sdb".into(),
            partitions: default_installation_partitions(),
        };

        executor
            .execute_operation(&operation)
            .expect("recording runner should accept command");

        assert_eq!(
            executor.runner.commands,
            vec![vec![
                "parted".to_owned(),
                "--script".to_owned(),
                "/dev/sdb".to_owned(),
                "mklabel".to_owned(),
                "gpt".to_owned(),
                "mkpart".to_owned(),
                "ESP".to_owned(),
                "fat32".to_owned(),
                "1MiB".to_owned(),
                "513MiB".to_owned(),
                "mkpart".to_owned(),
                "primary".to_owned(),
                "ext4".to_owned(),
                "513MiB".to_owned(),
                "100%".to_owned(),
            ]]
        );
    }
    #[test]
    fn root_partition_start_follows_efi_partition_size() {
        let mut executor =
            SystemInstallationOperationExecutor::with_runner(RecordingCommandRunner::default());

        let operation = InstallationOperation::PartitionDisk {
            device_path: "/dev/sdb".into(),
            partitions: vec![
                InstallationPartition::new(
                    InstallationPartitionRole::EfiSystem,
                    "fat32",
                    Some(256),
                ),
                InstallationPartition::new(InstallationPartitionRole::Root, "ext4", None),
            ],
        };

        executor
            .execute_operation(&operation)
            .expect("recording runner should accept command");

        assert_eq!(
            executor.runner.commands,
            vec![vec![
                "parted".to_owned(),
                "--script".to_owned(),
                "/dev/sdb".to_owned(),
                "mklabel".to_owned(),
                "gpt".to_owned(),
                "mkpart".to_owned(),
                "ESP".to_owned(),
                "fat32".to_owned(),
                "1MiB".to_owned(),
                "257MiB".to_owned(),
                "mkpart".to_owned(),
                "primary".to_owned(),
                "ext4".to_owned(),
                "257MiB".to_owned(),
                "100%".to_owned(),
            ]]
        );
    }
}
