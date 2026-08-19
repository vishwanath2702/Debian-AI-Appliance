use model::{DiscoveredStorage, DiscoveredStorageId, InstallationIntent, Plan};

use std::{io, path::PathBuf, process::Command};

use crate::{
    BootstrapConfig, BuildBackend, MmdebstrapBootstrapper, MmdebstrapError, RootfsBackend,
};
use executor::{ExecuteError, RootfsRunError};
use registry::PackageRepository;

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

fn partition_device_path(device_path: &std::path::Path, partition_number: usize) -> PathBuf {
    let device = device_path.to_string_lossy();

    if device
        .chars()
        .last()
        .is_some_and(|character| character.is_ascii_digit())
    {
        PathBuf::from(format!("{device}p{partition_number}"))
    } else {
        PathBuf::from(format!("{device}{partition_number}"))
    }
}
fn filesystem_uuid<R>(runner: &mut R, partition_path: &std::path::Path) -> io::Result<String>
where
    R: InstallationCommandRunner,
{
    let mut command = Command::new("blkid");

    command
        .arg("-s")
        .arg("UUID")
        .arg("-o")
        .arg("value")
        .arg(partition_path);

    let output = runner.output(&mut command)?;

    let uuid = std::str::from_utf8(&output)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?
        .trim();

    if uuid.is_empty() {
        return Err(io::Error::other(format!(
            "filesystem UUID is missing for {}",
            partition_path.display()
        )));
    }

    Ok(uuid.to_owned())
}
fn command_in_root(root: &std::path::Path, program: &str, args: &[&str]) -> Command {
    let mut command = Command::new("sudo");

    command
        .arg("/usr/sbin/chroot")
        .arg(root)
        .arg(program)
        .args(args);

    command
}

fn installed_mount_point(mount: &InstallationMount) -> io::Result<PathBuf> {
    let relative = mount.mount_point().strip_prefix("/target").map_err(|_| {
        io::Error::other(format!(
            "installation mount is outside target root: {}",
            mount.mount_point().display()
        ))
    })?;

    if relative.as_os_str().is_empty() {
        Ok(PathBuf::from("/"))
    } else {
        Ok(PathBuf::from("/").join(relative))
    }
}

fn installation_fstab(
    root_uuid: &str,
    efi_uuid: &str,
    mounts: &[InstallationMount],
) -> io::Result<String> {
    let root_mount = mounts
        .iter()
        .find(|mount| mount.role() == InstallationPartitionRole::Root)
        .ok_or_else(|| io::Error::other("root mount is missing"))?;

    let efi_mount = mounts
        .iter()
        .find(|mount| mount.role() == InstallationPartitionRole::EfiSystem)
        .ok_or_else(|| io::Error::other("EFI mount is missing"))?;

    let root_mount_point = installed_mount_point(root_mount)?;
    let efi_mount_point = installed_mount_point(efi_mount)?;

    Ok(format!(
        "UUID={root_uuid}\t{}\text4\tdefaults\t0\t1\n\
         UUID={efi_uuid}\t{}\tvfat\tumask=0077\t0\t2\n",
        root_mount_point.display(),
        efi_mount_point.display(),
    ))
}

/// Describes one filesystem mount required by an installation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstallationMount {
    role: InstallationPartitionRole,
    mount_point: PathBuf,
}
impl InstallationMount {
    /// Creates an installation mount description.
    #[must_use]
    pub fn new(role: InstallationPartitionRole, mount_point: impl Into<PathBuf>) -> Self {
        Self {
            role,
            mount_point: mount_point.into(),
        }
    }

    /// Returns the partition role mounted at this location.
    #[must_use]
    pub const fn role(&self) -> InstallationPartitionRole {
        self.role
    }

    /// Returns the mount point.
    #[must_use]
    pub fn mount_point(&self) -> &std::path::Path {
        &self.mount_point
    }
}
/// Returns the default DAIA installation mount layout.
#[must_use]
pub fn default_installation_mounts() -> Vec<InstallationMount> {
    vec![
        InstallationMount::new(InstallationPartitionRole::Root, "/target"),
        InstallationMount::new(InstallationPartitionRole::EfiSystem, "/target/boot/efi"),
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
        /// Validated Linux device path for the selected storage.
        device_path: PathBuf,

        /// Partitions whose filesystems should be created.
        partitions: Vec<InstallationPartition>,
    },
    /// Mount the prepared target filesystems.
    MountFilesystems {
        device_path: PathBuf,
        partitions: Vec<InstallationPartition>,
        mounts: Vec<InstallationMount>,
    },
    /// Bootstrap the base operating system.
    BootstrapSystem {
        root: PathBuf,
        bootstrap: BootstrapConfig,
    },

    /// Apply the appliance execution plans.
    ApplyPlans { plans: Vec<Plan> },

    /// Configure persistent filesystem mounts for the installed system.

    /// Configure persistent filesystem mounts for the installed system.
    ConfigureFstab {
        device_path: PathBuf,
        partitions: Vec<InstallationPartition>,
        mounts: Vec<InstallationMount>,
    },

    /// Prepares runtime filesystems required by commands executed inside the target root.
    PrepareTargetRuntime { root: PathBuf },

    /// Install the bootloader into the installed system.
    InstallBootloader { root: PathBuf, device_path: PathBuf },
    /// Cleans up temporary runtime filesystems mounted inside the target root.
    CleanupTargetRuntime { root: PathBuf },
    /// Unmounts the installed filesystems after installation is complete.
    UnmountFilesystems { mounts: Vec<InstallationMount> },
}
impl InstallationOperation {
    /// Returns whether this operation cleans up installation resources.
    #[must_use]
    pub const fn is_cleanup(&self) -> bool {
        matches!(
            self,
            Self::CleanupTargetRuntime { .. } | Self::UnmountFilesystems { .. }
        )
    }
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

    fn output(&mut self, command: &mut Command) -> io::Result<Vec<u8>>;
}
/// Writes files into the installed system.
pub trait InstallationFileWriter {
    fn write(&mut self, path: &std::path::Path, contents: &[u8]) -> io::Result<()>;
}

/// Writes installed-system files through the host filesystem.
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemInstallationFileWriter;

impl InstallationFileWriter for SystemInstallationFileWriter {
    fn write(&mut self, path: &std::path::Path, contents: &[u8]) -> io::Result<()> {
        std::fs::write(path, contents)
    }
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

    fn output(&mut self, command: &mut Command) -> io::Result<Vec<u8>> {
        let output = command.output()?;

        if output.status.success() {
            Ok(output.stdout)
        } else {
            Err(io::Error::other(format!(
                "installation command exited unsuccessfully: {}",
                output.status
            )))
        }
    }
}

pub struct SystemInstallationOperationExecutor<R, B, P, W = SystemInstallationFileWriter> {
    runner: R,
    bootstrapper: B,
    plan_executor: P,
    file_writer: W,
}

#[cfg(test)]
impl<R, B, P> SystemInstallationOperationExecutor<R, B, P, SystemInstallationFileWriter>
where
    R: InstallationCommandRunner,
    B: InstallationBootstrapper,
    P: InstallationPlanExecutor,
{
    const fn with_dependencies(runner: R, bootstrapper: B, plan_executor: P) -> Self {
        Self {
            runner,
            bootstrapper,
            plan_executor,
            file_writer: SystemInstallationFileWriter,
        }
    }
}

#[cfg(test)]
impl<R, B, P, W> SystemInstallationOperationExecutor<R, B, P, W>
where
    R: InstallationCommandRunner,
    B: InstallationBootstrapper,
    P: InstallationPlanExecutor,
    W: InstallationFileWriter,
{
    const fn with_all_dependencies(
        runner: R,
        bootstrapper: B,
        plan_executor: P,
        file_writer: W,
    ) -> Self {
        Self {
            runner,
            bootstrapper,
            plan_executor,
            file_writer,
        }
    }
}

impl
    SystemInstallationOperationExecutor<
        ProcessInstallationCommandRunner,
        MmdebstrapBootstrapper,
        RootfsInstallationPlanExecutor,
        SystemInstallationFileWriter,
    >
{
    /// Creates a production installation executor.
    #[must_use]
    pub fn new(asset_directory: PathBuf, package_repository: PackageRepository) -> Self {
        Self {
            runner: ProcessInstallationCommandRunner,
            bootstrapper: MmdebstrapBootstrapper::new(),
            plan_executor: RootfsInstallationPlanExecutor::new(
                PathBuf::from("/target"),
                asset_directory,
                package_repository,
            ),
            file_writer: SystemInstallationFileWriter,
        }
    }
}

impl<R, B, P, W> InstallationOperationExecutor for SystemInstallationOperationExecutor<R, B, P, W>
where
    R: InstallationCommandRunner,
    B: InstallationBootstrapper,
    P: InstallationPlanExecutor,
    W: InstallationFileWriter,
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
                let efi_partition = partitions
                    .iter()
                    .find(|partition| partition.role() == InstallationPartitionRole::EfiSystem)
                    .ok_or_else(|| io::Error::other("EFI partition is missing"))?;

                let root_partition = partitions
                    .iter()
                    .find(|partition| partition.role() == InstallationPartitionRole::Root)
                    .ok_or_else(|| io::Error::other("root partition is missing"))?;
                let mut command = Command::new("parted");

                command
                    .arg("--script")
                    .arg(device_path)
                    .arg("mklabel")
                    .arg("gpt");

                let efi_size_mib = efi_partition
                    .size_mib()
                    .ok_or_else(|| io::Error::other("EFI partition size is missing"))?;
                if efi_size_mib == 0 {
                    return Err(io::Error::other(
                        "EFI partition size must be greater than zero",
                    ));
                }

                command
                    .arg("mkpart")
                    .arg("ESP")
                    .arg(efi_partition.filesystem())
                    .arg("1MiB")
                    .arg(format!("{}MiB", efi_size_mib + 1));

                let root_start_mib = efi_size_mib + 1;

                command
                    .arg("mkpart")
                    .arg("primary")
                    .arg(root_partition.filesystem())
                    .arg(format!("{root_start_mib}MiB"))
                    .arg("100%");

                self.runner.status(&mut command)
            }

            InstallationOperation::CreateFilesystems {
                device_path,
                partitions,
            } => {
                let efi_partition = partitions
                    .iter()
                    .position(|partition| partition.role() == InstallationPartitionRole::EfiSystem)
                    .ok_or_else(|| io::Error::other("EFI partition is missing"))?;

                let partition = &partitions[efi_partition];

                if partition.filesystem() != "fat32" {
                    return Err(io::Error::other(format!(
                        "unsupported EFI filesystem: {}",
                        partition.filesystem()
                    )));
                }

                let partition_number = efi_partition + 1;
                let partition_path = partition_device_path(device_path, partition_number);

                let mut command = Command::new("mkfs.fat");

                command.arg("-F").arg("32").arg(partition_path);

                self.runner.status(&mut command)?;

                let root_partition = partitions
                    .iter()
                    .position(|partition| partition.role() == InstallationPartitionRole::Root)
                    .ok_or_else(|| io::Error::other("root partition is missing"))?;

                let partition_number = root_partition + 1;
                let partition_path = partition_device_path(device_path, partition_number);

                let command_name = match partitions[root_partition].filesystem() {
                    "ext4" => "mkfs.ext4",
                    filesystem => {
                        return Err(io::Error::other(format!(
                            "unsupported root filesystem: {filesystem}"
                        )));
                    }
                };

                let mut command = Command::new(command_name);

                command.arg("-F").arg(partition_path);

                self.runner.status(&mut command)?;

                Ok(())
            }
            InstallationOperation::MountFilesystems {
                device_path,
                partitions,
                mounts,
            } => {
                let root_mount = mounts
                    .iter()
                    .find(|mount| mount.role() == InstallationPartitionRole::Root)
                    .ok_or_else(|| io::Error::other("root mount is missing"))?;

                let efi_mount = mounts
                    .iter()
                    .find(|mount| mount.role() == InstallationPartitionRole::EfiSystem)
                    .ok_or_else(|| io::Error::other("EFI mount is missing"))?;

                let root_partition_number = partitions
                    .iter()
                    .position(|partition| partition.role() == InstallationPartitionRole::Root)
                    .map(|index| index + 1)
                    .ok_or_else(|| io::Error::other("root partition is missing"))?;

                let efi_partition_number = partitions
                    .iter()
                    .position(|partition| partition.role() == InstallationPartitionRole::EfiSystem)
                    .map(|index| index + 1)
                    .ok_or_else(|| io::Error::other("EFI partition is missing"))?;

                let root_partition_path = partition_device_path(device_path, root_partition_number);

                let efi_partition_path = partition_device_path(device_path, efi_partition_number);

                let mut command = Command::new("mkdir");

                command.arg("-p").arg(root_mount.mount_point());

                self.runner.status(&mut command)?;

                let mut command = Command::new("mount");

                command
                    .arg(root_partition_path)
                    .arg(root_mount.mount_point());

                self.runner.status(&mut command)?;

                let mut command = Command::new("mkdir");

                command.arg("-p").arg(efi_mount.mount_point());

                self.runner.status(&mut command)?;

                let mut command = Command::new("mount");

                command.arg(efi_partition_path).arg(efi_mount.mount_point());

                self.runner.status(&mut command)?;

                Ok(())
            }
            InstallationOperation::BootstrapSystem { root, bootstrap } => self
                .bootstrapper
                .bootstrap(root, bootstrap)
                .map_err(|_| io::Error::other("installation bootstrap failed")),
            InstallationOperation::ApplyPlans { plans } => self
                .plan_executor
                .apply_plans(plans)
                .map_err(|_| io::Error::other("installation plan execution failed")),
            InstallationOperation::ConfigureFstab {
                device_path,
                partitions,
                mounts,
            } => {
                let root_partition_number = partitions
                    .iter()
                    .position(|partition| partition.role() == InstallationPartitionRole::Root)
                    .map(|index| index + 1)
                    .ok_or_else(|| io::Error::other("root partition is missing"))?;

                let efi_partition_number = partitions
                    .iter()
                    .position(|partition| partition.role() == InstallationPartitionRole::EfiSystem)
                    .map(|index| index + 1)
                    .ok_or_else(|| io::Error::other("EFI partition is missing"))?;

                let root_partition_path = partition_device_path(device_path, root_partition_number);

                let efi_partition_path = partition_device_path(device_path, efi_partition_number);

                let root_uuid = filesystem_uuid(&mut self.runner, &root_partition_path)?;

                let efi_uuid = filesystem_uuid(&mut self.runner, &efi_partition_path)?;

                let fstab = installation_fstab(&root_uuid, &efi_uuid, mounts)?;

                self.file_writer
                    .write(std::path::Path::new("/target/etc/fstab"), fstab.as_bytes())
            }
            InstallationOperation::PrepareTargetRuntime { root } => {
                let target_dev = root.join("dev");

                let mut mkdir = Command::new("mkdir");
                mkdir.arg("-p").arg(&target_dev);

                self.runner.status(&mut mkdir)?;

                let mut mount = Command::new("mount");
                mount.arg("--rbind").arg("/dev").arg(&target_dev);

                self.runner.status(&mut mount)?;

                let target_proc = root.join("proc");

                let mut mkdir = Command::new("mkdir");
                mkdir.arg("-p").arg(&target_proc);

                self.runner.status(&mut mkdir)?;

                let mut mount = Command::new("mount");
                mount.arg("-t").arg("proc").arg("proc").arg(&target_proc);

                self.runner.status(&mut mount)?;
                let target_sys = root.join("sys");

                let mut mkdir = Command::new("mkdir");
                mkdir.arg("-p").arg(&target_sys);

                self.runner.status(&mut mkdir)?;

                let mut mount = Command::new("mount");
                mount.arg("--rbind").arg("/sys").arg(&target_sys);

                self.runner.status(&mut mount)?;
                let target_run = root.join("run");

                let mut mkdir = Command::new("mkdir");
                mkdir.arg("-p").arg(&target_run);

                self.runner.status(&mut mkdir)?;

                let mut mount = Command::new("mount");
                mount.arg("--rbind").arg("/run").arg(&target_run);

                self.runner.status(&mut mount)
            }
            InstallationOperation::InstallBootloader { root, .. } => {
                let mut grub_install = command_in_root(
                    root,
                    "grub-install",
                    &[
                        "--target=x86_64-efi",
                        "--efi-directory=/boot/efi",
                        "--bootloader-id=DAIA",
                    ],
                );

                self.runner.status(&mut grub_install)?;

                let mut update_grub = command_in_root(root, "update-grub", &[]);

                self.runner.status(&mut update_grub)
            }
            InstallationOperation::CleanupTargetRuntime { root } => {
                let target_run = root.join("run");

                let mut unmount = Command::new("umount");
                unmount.arg("-R").arg(&target_run);

                self.runner.status(&mut unmount)?;

                let target_sys = root.join("sys");

                let mut unmount = Command::new("umount");
                unmount.arg("-R").arg(&target_sys);

                self.runner.status(&mut unmount)?;

                let target_proc = root.join("proc");

                let mut unmount = Command::new("umount");
                unmount.arg("-R").arg(&target_proc);

                self.runner.status(&mut unmount)?;

                let target_dev = root.join("dev");

                let mut unmount = Command::new("umount");
                unmount.arg("-R").arg(&target_dev);

                self.runner.status(&mut unmount)
            }

            InstallationOperation::UnmountFilesystems { mounts } => {
                let efi_mount = mounts
                    .iter()
                    .find(|mount| mount.role() == InstallationPartitionRole::EfiSystem)
                    .ok_or_else(|| io::Error::other("EFI mount is missing"))?;

                let mut unmount = Command::new("umount");
                unmount.arg(efi_mount.mount_point());

                self.runner.status(&mut unmount)?;

                let root_mount = mounts
                    .iter()
                    .find(|mount| mount.role() == InstallationPartitionRole::Root)
                    .ok_or_else(|| io::Error::other("root mount is missing"))?;

                let mut unmount = Command::new("umount");
                unmount.arg(root_mount.mount_point());

                self.runner.status(&mut unmount)
            }
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
    bootstrap: BootstrapConfig,
}

impl PreparedInstallation {
    /// Creates a prepared installation from validated components.
    #[must_use]
    pub const fn new(
        intent: InstallationIntent,
        storage: DiscoveredStorage,
        plans: Vec<Plan>,
        bootstrap: BootstrapConfig,
    ) -> Self {
        Self {
            intent,
            storage,
            plans,
            bootstrap,
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
                device_path: self.storage.device_path().to_path_buf(),
                partitions: default_installation_partitions(),
            },
            InstallationOperation::MountFilesystems {
                device_path: self.storage.device_path().to_path_buf(),
                partitions: default_installation_partitions(),
                mounts: default_installation_mounts(),
            },
            InstallationOperation::BootstrapSystem {
                root: "/target".into(),
                bootstrap: self.bootstrap.clone(),
            },
            InstallationOperation::ApplyPlans {
                plans: self.plans.clone(),
            },
            InstallationOperation::ConfigureFstab {
                device_path: self.storage.device_path().to_path_buf(),
                partitions: default_installation_partitions(),
                mounts: default_installation_mounts(),
            },
            InstallationOperation::PrepareTargetRuntime {
                root: "/target".into(),
            },
            InstallationOperation::InstallBootloader {
                root: "/target".into(),
                device_path: self.storage.device_path().to_path_buf(),
            },
            InstallationOperation::CleanupTargetRuntime {
                root: "/target".into(),
            },
            InstallationOperation::UnmountFilesystems {
                mounts: default_installation_mounts(),
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
    /// Returns the root filesystem bootstrap configuration.
    #[must_use]
    pub const fn bootstrap(&self) -> &BootstrapConfig {
        &self.bootstrap
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

/// Executes the root filesystem bootstrap stage of an installation.
pub trait InstallationBootstrapper {
    /// Error produced while bootstrapping.
    type Error;

    /// Bootstraps the target root filesystem.
    fn bootstrap(
        &self,
        root: &std::path::Path,
        config: &BootstrapConfig,
    ) -> Result<(), Self::Error>;
}

impl InstallationBootstrapper for MmdebstrapBootstrapper {
    type Error = MmdebstrapError;

    fn bootstrap(
        &self,
        root: &std::path::Path,
        config: &BootstrapConfig,
    ) -> Result<(), Self::Error> {
        self.bootstrap_root(root, config)
    }
}

/// Executes appliance plans against an installation root.
pub trait InstallationPlanExecutor {
    /// Error produced while applying plans.
    type Error;

    /// Applies the supplied appliance plans.
    fn apply_plans(&mut self, plans: &[Plan]) -> Result<(), Self::Error>;
}

/// Applies installation plans to the mounted target root filesystem.
pub struct RootfsInstallationPlanExecutor {
    backend: RootfsBackend,
}

impl RootfsInstallationPlanExecutor {
    /// Creates a plan executor for an installed root filesystem.
    #[must_use]
    pub const fn new(
        rootfs: PathBuf,
        asset_directory: PathBuf,
        package_repository: PackageRepository,
    ) -> Self {
        Self {
            backend: RootfsBackend::new(rootfs, asset_directory, package_repository),
        }
    }
}

impl InstallationPlanExecutor for RootfsInstallationPlanExecutor {
    type Error = ExecuteError<RootfsRunError>;

    fn apply_plans(&mut self, plans: &[Plan]) -> Result<(), Self::Error> {
        for plan in plans {
            self.backend.build(plan)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        BootstrapConfig, InstallationBootstrapper, InstallationCommandRunner,
        InstallationFileWriter, InstallationMount, InstallationOperation,
        InstallationOperationExecutor, InstallationPartition, InstallationPartitionRole,
        InstallationPlanExecutor, PathBuf, PreparedInstallation, ProcessInstallationCommandRunner,
        SystemInstallationOperationExecutor, command_in_root, default_installation_mounts,
        default_installation_partitions, filesystem_uuid, installation_fstab,
        installed_mount_point, partition_device_path,
    };
    use model::{
        Capability, DiscoveredStorage, DiscoveredStorageId, InstallationIntent, Plan, ProviderId,
        StorageKind,
    };

    use std::{io, process::Command};

    #[derive(Default)]
    struct RecordingInstallationFileWriter {
        writes: Vec<(PathBuf, Vec<u8>)>,
    }

    impl InstallationFileWriter for RecordingInstallationFileWriter {
        fn write(&mut self, path: &std::path::Path, contents: &[u8]) -> io::Result<()> {
            self.writes.push((path.to_path_buf(), contents.to_vec()));
            Ok(())
        }
    }
    #[derive(Default)]
    struct RecordingCommandRunner {
        commands: Vec<Vec<String>>,
        outputs: Vec<Vec<u8>>,
    }
    struct FailingCommandRunner;

    impl InstallationCommandRunner for FailingCommandRunner {
        fn status(&mut self, _command: &mut Command) -> io::Result<()> {
            Err(io::Error::other("command failed"))
        }

        fn output(&mut self, _command: &mut Command) -> io::Result<Vec<u8>> {
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
        fn output(&mut self, command: &mut Command) -> io::Result<Vec<u8>> {
            let mut recorded = vec![command.get_program().to_string_lossy().into_owned()];

            recorded.extend(
                command
                    .get_args()
                    .map(|arg| arg.to_string_lossy().into_owned()),
            );

            self.commands.push(recorded);

            if self.outputs.is_empty() {
                return Err(io::Error::other("no recorded command output"));
            }

            Ok(self.outputs.remove(0))
        }
    }

    #[derive(Default)]
    struct RecordingInstallationBootstrapper {
        calls: std::sync::Mutex<Vec<(PathBuf, BootstrapConfig)>>,
    }

    impl InstallationBootstrapper for RecordingInstallationBootstrapper {
        type Error = std::convert::Infallible;

        fn bootstrap(
            &self,
            root: &std::path::Path,
            config: &BootstrapConfig,
        ) -> Result<(), Self::Error> {
            self.calls
                .lock()
                .expect("recording bootstrap calls should not be poisoned")
                .push((root.to_path_buf(), config.clone()));

            Ok(())
        }
    }
    struct FailingInstallationBootstrapper;

    impl InstallationBootstrapper for FailingInstallationBootstrapper {
        type Error = io::Error;

        fn bootstrap(
            &self,
            _root: &std::path::Path,
            _config: &BootstrapConfig,
        ) -> Result<(), Self::Error> {
            Err(io::Error::other("bootstrap failed"))
        }
    }

    #[derive(Default)]
    struct RecordingInstallationPlanExecutor {
        plans: Vec<Plan>,
    }

    impl InstallationPlanExecutor for RecordingInstallationPlanExecutor {
        type Error = std::convert::Infallible;

        fn apply_plans(&mut self, plans: &[Plan]) -> Result<(), Self::Error> {
            self.plans.extend_from_slice(plans);
            Ok(())
        }
    }

    impl RecordingCommandRunner {
        fn with_outputs(outputs: Vec<Vec<u8>>) -> Self {
            Self {
                commands: Vec::new(),
                outputs,
            }
        }
    }

    struct FailingAtCommandRunner {
        commands: Vec<Vec<String>>,
        fail_at: usize,
    }

    impl InstallationCommandRunner for FailingAtCommandRunner {
        fn status(&mut self, command: &mut Command) -> io::Result<()> {
            let mut recorded = vec![command.get_program().to_string_lossy().into_owned()];

            recorded.extend(
                command
                    .get_args()
                    .map(|arg| arg.to_string_lossy().into_owned()),
            );

            self.commands.push(recorded);

            if self.commands.len() == self.fail_at {
                Err(io::Error::other("command failed"))
            } else {
                Ok(())
            }
        }

        fn output(&mut self, _command: &mut Command) -> io::Result<Vec<u8>> {
            Err(io::Error::other("unexpected command output request"))
        }
    }

    #[test]
    fn identifies_installation_cleanup_operations() {
        assert!(
            InstallationOperation::CleanupTargetRuntime {
                root: "/target".into(),
            }
            .is_cleanup()
        );

        assert!(
            InstallationOperation::UnmountFilesystems {
                mounts: default_installation_mounts(),
            }
            .is_cleanup()
        );

        assert!(
            !InstallationOperation::InstallBootloader {
                root: "/target".into(),
                device_path: "/dev/sdb".into(),
            }
            .is_cleanup()
        );
    }

    #[test]
    fn system_executor_unmounts_installation_filesystems() {
        let mut executor = SystemInstallationOperationExecutor::with_dependencies(
            RecordingCommandRunner::default(),
            RecordingInstallationBootstrapper::default(),
            RecordingInstallationPlanExecutor::default(),
        );

        let operation = InstallationOperation::UnmountFilesystems {
            mounts: default_installation_mounts(),
        };

        executor
            .execute_operation(&operation)
            .expect("filesystem unmount should succeed");

        assert_eq!(
            executor.runner.commands,
            vec![
                vec!["umount".to_owned(), "/target/boot/efi".to_owned(),],
                vec!["umount".to_owned(), "/target".to_owned(),],
            ]
        );
    }
    #[test]
    fn system_executor_cleans_up_target_run_runtime_mount() {
        let mut executor = SystemInstallationOperationExecutor::with_dependencies(
            RecordingCommandRunner::default(),
            RecordingInstallationBootstrapper::default(),
            RecordingInstallationPlanExecutor::default(),
        );

        let operation = InstallationOperation::CleanupTargetRuntime {
            root: "/target".into(),
        };

        executor
            .execute_operation(&operation)
            .expect("target runtime cleanup should succeed");

        assert_eq!(
            executor.runner.commands,
            vec![
                vec![
                    "umount".to_owned(),
                    "-R".to_owned(),
                    "/target/run".to_owned(),
                ],
                vec![
                    "umount".to_owned(),
                    "-R".to_owned(),
                    "/target/sys".to_owned(),
                ],
                vec![
                    "umount".to_owned(),
                    "-R".to_owned(),
                    "/target/proc".to_owned(),
                ],
                vec![
                    "umount".to_owned(),
                    "-R".to_owned(),
                    "/target/dev".to_owned(),
                ],
            ]
        );
    }
    #[test]
    fn system_executor_prepares_target_runtime_mounts() {
        let mut executor = SystemInstallationOperationExecutor::with_dependencies(
            RecordingCommandRunner::default(),
            RecordingInstallationBootstrapper::default(),
            RecordingInstallationPlanExecutor::default(),
        );

        let operation = InstallationOperation::PrepareTargetRuntime {
            root: "/target".into(),
        };

        executor
            .execute_operation(&operation)
            .expect("target runtime preparation should succeed");

        assert_eq!(
            executor.runner.commands,
            vec![
                vec![
                    "mkdir".to_owned(),
                    "-p".to_owned(),
                    "/target/dev".to_owned(),
                ],
                vec![
                    "mount".to_owned(),
                    "--rbind".to_owned(),
                    "/dev".to_owned(),
                    "/target/dev".to_owned(),
                ],
                vec![
                    "mkdir".to_owned(),
                    "-p".to_owned(),
                    "/target/proc".to_owned(),
                ],
                vec![
                    "mount".to_owned(),
                    "-t".to_owned(),
                    "proc".to_owned(),
                    "proc".to_owned(),
                    "/target/proc".to_owned(),
                ],
                vec![
                    "mkdir".to_owned(),
                    "-p".to_owned(),
                    "/target/sys".to_owned(),
                ],
                vec![
                    "mount".to_owned(),
                    "--rbind".to_owned(),
                    "/sys".to_owned(),
                    "/target/sys".to_owned(),
                ],
                vec![
                    "mkdir".to_owned(),
                    "-p".to_owned(),
                    "/target/run".to_owned(),
                ],
                vec![
                    "mount".to_owned(),
                    "--rbind".to_owned(),
                    "/run".to_owned(),
                    "/target/run".to_owned(),
                ],
            ]
        );
    }

    #[test]
    fn system_executor_stops_bootloader_installation_when_grub_install_fails() {
        let mut executor = SystemInstallationOperationExecutor::with_dependencies(
            FailingAtCommandRunner {
                commands: Vec::new(),
                fail_at: 1,
            },
            RecordingInstallationBootstrapper::default(),
            RecordingInstallationPlanExecutor::default(),
        );

        let operation = InstallationOperation::InstallBootloader {
            root: "/target".into(),
            device_path: "/dev/sdb".into(),
        };

        executor
            .execute_operation(&operation)
            .expect_err("grub-install failure should fail bootloader installation");

        assert_eq!(
            executor.runner.commands,
            vec![vec![
                "sudo".to_owned(),
                "/usr/sbin/chroot".to_owned(),
                "/target".to_owned(),
                "grub-install".to_owned(),
                "--target=x86_64-efi".to_owned(),
                "--efi-directory=/boot/efi".to_owned(),
                "--bootloader-id=DAIA".to_owned(),
            ]]
        );
    }

    #[test]
    fn system_executor_runs_grub_install_in_target_root() {
        let mut executor = SystemInstallationOperationExecutor::with_dependencies(
            RecordingCommandRunner::default(),
            RecordingInstallationBootstrapper::default(),
            RecordingInstallationPlanExecutor::default(),
        );

        let operation = InstallationOperation::InstallBootloader {
            root: "/target".into(),
            device_path: "/dev/sdb".into(),
        };

        executor
            .execute_operation(&operation)
            .expect("bootloader installation should succeed");

        assert_eq!(
            executor.runner.commands,
            vec![
                vec![
                    "sudo".to_owned(),
                    "/usr/sbin/chroot".to_owned(),
                    "/target".to_owned(),
                    "grub-install".to_owned(),
                    "--target=x86_64-efi".to_owned(),
                    "--efi-directory=/boot/efi".to_owned(),
                    "--bootloader-id=DAIA".to_owned(),
                ],
                vec![
                    "sudo".to_owned(),
                    "/usr/sbin/chroot".to_owned(),
                    "/target".to_owned(),
                    "update-grub".to_owned(),
                ],
            ]
        );
    }

    #[test]
    fn command_in_root_constructs_chroot_command() {
        let command = command_in_root(
            std::path::Path::new("/target"),
            "example-command",
            &["--first", "value"],
        );

        let recorded = std::iter::once(command.get_program())
            .chain(command.get_args())
            .map(|value| value.to_string_lossy().into_owned())
            .collect::<Vec<_>>();

        assert_eq!(
            recorded,
            vec![
                "sudo".to_owned(),
                "/usr/sbin/chroot".to_owned(),
                "/target".to_owned(),
                "example-command".to_owned(),
                "--first".to_owned(),
                "value".to_owned(),
            ]
        );
    }

    #[test]
    fn system_executor_does_not_write_fstab_for_missing_root_partition() {
        let mut executor = SystemInstallationOperationExecutor::with_all_dependencies(
            RecordingCommandRunner::default(),
            RecordingInstallationBootstrapper::default(),
            RecordingInstallationPlanExecutor::default(),
            RecordingInstallationFileWriter::default(),
        );

        let operation = InstallationOperation::ConfigureFstab {
            device_path: "/dev/sdb".into(),
            partitions: vec![InstallationPartition::new(
                InstallationPartitionRole::EfiSystem,
                "fat32",
                Some(512),
            )],
            mounts: default_installation_mounts(),
        };

        let error = executor
            .execute_operation(&operation)
            .expect_err("missing root partition should fail fstab configuration");

        assert!(error.to_string().contains("root partition is missing"));
        assert!(executor.runner.commands.is_empty());
        assert!(executor.file_writer.writes.is_empty());
    }

    #[test]
    fn system_executor_does_not_write_fstab_when_uuid_lookup_fails() {
        let mut executor = SystemInstallationOperationExecutor::with_all_dependencies(
            FailingCommandRunner,
            RecordingInstallationBootstrapper::default(),
            RecordingInstallationPlanExecutor::default(),
            RecordingInstallationFileWriter::default(),
        );

        let operation = InstallationOperation::ConfigureFstab {
            device_path: "/dev/sdb".into(),
            partitions: default_installation_partitions(),
            mounts: default_installation_mounts(),
        };

        executor
            .execute_operation(&operation)
            .expect_err("UUID lookup failure should fail fstab configuration");

        assert!(executor.file_writer.writes.is_empty());
    }

    #[test]
    fn system_executor_configures_fstab_with_filesystem_uuids() {
        let mut executor = SystemInstallationOperationExecutor::with_all_dependencies(
            RecordingCommandRunner::with_outputs(vec![
                b"root-uuid\n".to_vec(),
                b"efi-uuid\n".to_vec(),
            ]),
            RecordingInstallationBootstrapper::default(),
            RecordingInstallationPlanExecutor::default(),
            RecordingInstallationFileWriter::default(),
        );

        let operation = InstallationOperation::ConfigureFstab {
            device_path: "/dev/sdb".into(),
            partitions: default_installation_partitions(),
            mounts: default_installation_mounts(),
        };

        executor
            .execute_operation(&operation)
            .expect("fstab configuration should succeed");

        assert_eq!(
            executor.runner.commands,
            vec![
                vec![
                    "blkid".to_owned(),
                    "-s".to_owned(),
                    "UUID".to_owned(),
                    "-o".to_owned(),
                    "value".to_owned(),
                    "/dev/sdb2".to_owned(),
                ],
                vec![
                    "blkid".to_owned(),
                    "-s".to_owned(),
                    "UUID".to_owned(),
                    "-o".to_owned(),
                    "value".to_owned(),
                    "/dev/sdb1".to_owned(),
                ],
            ]
        );

        assert_eq!(
            executor.file_writer.writes,
            vec![(
                PathBuf::from("/target/etc/fstab"),
                concat!(
                    "UUID=root-uuid\t/\text4\tdefaults\t0\t1\n",
                    "UUID=efi-uuid\t/boot/efi\tvfat\tumask=0077\t0\t2\n",
                )
                .as_bytes()
                .to_vec(),
            )]
        );
    }
    #[test]
    fn system_executor_accepts_recording_file_writer_dependency() {
        let executor = SystemInstallationOperationExecutor::with_all_dependencies(
            RecordingCommandRunner::default(),
            RecordingInstallationBootstrapper::default(),
            RecordingInstallationPlanExecutor::default(),
            RecordingInstallationFileWriter::default(),
        );

        assert!(executor.file_writer.writes.is_empty());
    }

    #[test]
    fn installation_fstab_rejects_missing_efi_mount() {
        let mounts = vec![InstallationMount::new(
            InstallationPartitionRole::Root,
            "/target",
        )];

        let error = installation_fstab("root-uuid", "efi-uuid", &mounts)
            .expect_err("missing EFI mount should fail");

        assert!(error.to_string().contains("EFI mount is missing"));
    }

    #[test]
    fn installed_mount_point_converts_target_root_to_root() {
        let mount = InstallationMount::new(InstallationPartitionRole::Root, "/target");

        assert_eq!(
            installed_mount_point(&mount).expect("root mount should convert"),
            PathBuf::from("/")
        );
    }

    #[test]
    fn installed_mount_point_strips_target_prefix() {
        let mount =
            InstallationMount::new(InstallationPartitionRole::EfiSystem, "/target/boot/efi");

        assert_eq!(
            installed_mount_point(&mount).expect("EFI mount should convert"),
            PathBuf::from("/boot/efi")
        );
    }

    #[test]
    fn installation_fstab_uses_filesystem_uuids() {
        let fstab = installation_fstab("root-uuid", "efi-uuid", &default_installation_mounts())
            .expect("fstab should be generated");

        assert_eq!(
            fstab,
            concat!(
                "UUID=root-uuid\t/\text4\tdefaults\t0\t1\n",
                "UUID=efi-uuid\t/boot/efi\tvfat\tumask=0077\t0\t2\n",
            )
        );
    }

    #[test]
    fn filesystem_uuid_reads_uuid_with_blkid() {
        let mut runner = RecordingCommandRunner::with_outputs(vec![b"root-uuid\n".to_vec()]);

        let uuid = filesystem_uuid(&mut runner, std::path::Path::new("/dev/sdb2"))
            .expect("filesystem UUID should be discovered");

        assert_eq!(uuid, "root-uuid");

        assert_eq!(
            runner.commands,
            vec![vec![
                "blkid".to_owned(),
                "-s".to_owned(),
                "UUID".to_owned(),
                "-o".to_owned(),
                "value".to_owned(),
                "/dev/sdb2".to_owned(),
            ]]
        );
    }

    #[test]
    fn filesystem_uuid_returns_command_failure() {
        let mut runner = FailingCommandRunner;

        let error = filesystem_uuid(&mut runner, std::path::Path::new("/dev/sdb2"))
            .expect_err("blkid failure should be returned");

        assert!(error.to_string().contains("command failed"));
    }

    #[test]
    fn filesystem_uuid_rejects_empty_output() {
        let mut runner = RecordingCommandRunner::with_outputs(vec![b"\n".to_vec()]);

        let error = filesystem_uuid(&mut runner, std::path::Path::new("/dev/sdb2"))
            .expect_err("empty UUID should fail");

        assert!(error.to_string().contains("filesystem UUID is missing"));
    }

    #[test]
    fn recording_command_runner_returns_command_output() {
        let mut runner = RecordingCommandRunner::with_outputs(vec![b"root-uuid\n".to_vec()]);

        let mut command = Command::new("blkid");
        command
            .arg("-s")
            .arg("UUID")
            .arg("-o")
            .arg("value")
            .arg("/dev/sdb2");

        let output = runner
            .output(&mut command)
            .expect("recording runner should return command output");

        assert_eq!(output, b"root-uuid\n");

        assert_eq!(
            runner.commands,
            vec![vec![
                "blkid".to_owned(),
                "-s".to_owned(),
                "UUID".to_owned(),
                "-o".to_owned(),
                "value".to_owned(),
                "/dev/sdb2".to_owned(),
            ]]
        );
    }

    #[test]
    fn system_executor_routes_apply_plans_to_plan_executor() {
        let mut executor = SystemInstallationOperationExecutor::with_dependencies(
            RecordingCommandRunner::default(),
            RecordingInstallationBootstrapper::default(),
            RecordingInstallationPlanExecutor::default(),
        );

        let plan = Plan {
            capability: Capability::new("desktop"),
            provider: ProviderId::new("gnome"),
            steps: Vec::new(),
        };

        let operation = InstallationOperation::ApplyPlans {
            plans: vec![plan.clone()],
        };

        executor
            .execute_operation(&operation)
            .expect("apply plans operation should execute");

        assert_eq!(executor.plan_executor.plans, vec![plan]);
    }
    #[test]
    fn installation_plan_executor_records_plans() {
        let mut executor = RecordingInstallationPlanExecutor::default();

        let plans = Vec::<Plan>::new();

        executor
            .apply_plans(&plans)
            .expect("recording plan executor should succeed");

        assert!(executor.plans.is_empty());
    }

    #[test]
    fn system_executor_returns_bootstrap_failure() {
        let mut executor = SystemInstallationOperationExecutor::with_dependencies(
            RecordingCommandRunner::default(),
            FailingInstallationBootstrapper,
            RecordingInstallationPlanExecutor::default(),
        );

        let operation = InstallationOperation::BootstrapSystem {
            root: "/target".into(),
            bootstrap: BootstrapConfig::default(),
        };

        let error = executor
            .execute_operation(&operation)
            .expect_err("bootstrap failure should be returned");

        assert_eq!(error.kind(), io::ErrorKind::Other);
        assert!(error.to_string().contains("installation bootstrap failed"));
    }

    #[test]
    fn system_executor_routes_bootstrap_operation_to_bootstrapper() {
        let bootstrapper = RecordingInstallationBootstrapper::default();

        let config = BootstrapConfig::new(
            "trixie",
            "amd64",
            "https://deb.debian.org/debian",
            vec!["main".to_owned()],
            "minbase",
        );

        let mut executor = SystemInstallationOperationExecutor::with_dependencies(
            RecordingCommandRunner::default(),
            bootstrapper,
            RecordingInstallationPlanExecutor::default(),
        );

        let operation = InstallationOperation::BootstrapSystem {
            root: "/target".into(),
            bootstrap: config.clone(),
        };

        executor
            .execute_operation(&operation)
            .expect("bootstrap operation should execute");

        let calls = executor
            .bootstrapper
            .calls
            .lock()
            .expect("recording bootstrap calls should not be poisoned");

        assert_eq!(
            calls.as_slice(),
            &[(std::path::PathBuf::from("/target"), config)]
        );
    }
    #[test]
    fn installation_bootstrapper_records_root_and_configuration() {
        let bootstrapper = RecordingInstallationBootstrapper::default();

        let config = BootstrapConfig::new(
            "trixie",
            "amd64",
            "https://deb.debian.org/debian",
            vec!["main".to_owned()],
            "minbase",
        );

        bootstrapper
            .bootstrap(std::path::Path::new("/target"), &config)
            .expect("recording bootstrapper should succeed");

        let calls = bootstrapper
            .calls
            .lock()
            .expect("recording bootstrap calls should not be poisoned");

        assert_eq!(
            calls.as_slice(),
            &[(std::path::PathBuf::from("/target"), config)]
        );
    }

    #[test]
    fn installation_plan_carries_bootstrap_configuration() {
        let bootstrap = BootstrapConfig::new(
            "trixie",
            "amd64",
            "https://deb.debian.org/debian",
            vec!["main".to_owned(), "non-free-firmware".to_owned()],
            "minbase",
        );

        let intent =
            InstallationIntent::new("desktop", DiscoveredStorageId::new("serial:usb-disk"));

        let storage = DiscoveredStorage::new("serial:usb-disk", StorageKind::Removable, "/dev/sdb");

        let prepared = PreparedInstallation::new(intent, storage, Vec::new(), bootstrap.clone());

        let plan = prepared.installation_plan();

        let bootstrap_operation = plan
            .operations()
            .iter()
            .find_map(|operation| match operation {
                InstallationOperation::BootstrapSystem { root, bootstrap } => {
                    Some((root, bootstrap))
                }
                _ => None,
            })
            .expect("installation plan should contain bootstrap operation");

        assert_eq!(bootstrap_operation.0, std::path::Path::new("/target"));
        assert_eq!(bootstrap_operation.1, &bootstrap);
    }

    #[test]
    fn system_executor_rejects_missing_efi_partition_before_mounting() {
        let mut executor = SystemInstallationOperationExecutor::with_dependencies(
            RecordingCommandRunner::default(),
            RecordingInstallationBootstrapper::default(),
            RecordingInstallationPlanExecutor::default(),
        );
        let operation = InstallationOperation::MountFilesystems {
            device_path: "/dev/sdb".into(),
            partitions: vec![InstallationPartition::new(
                InstallationPartitionRole::Root,
                "ext4",
                None,
            )],
            mounts: default_installation_mounts(),
        };

        let error = executor
            .execute_operation(&operation)
            .expect_err("missing EFI partition should fail");

        assert!(error.to_string().contains("EFI partition is missing"));
        assert!(executor.runner.commands.is_empty());
    }

    #[test]
    fn system_executor_rejects_missing_root_partition_before_mounting() {
        let mut executor = SystemInstallationOperationExecutor::with_dependencies(
            RecordingCommandRunner::default(),
            RecordingInstallationBootstrapper::default(),
            RecordingInstallationPlanExecutor::default(),
        );

        let operation = InstallationOperation::MountFilesystems {
            device_path: "/dev/sdb".into(),
            partitions: vec![InstallationPartition::new(
                InstallationPartitionRole::EfiSystem,
                "fat32",
                Some(512),
            )],
            mounts: default_installation_mounts(),
        };

        let error = executor
            .execute_operation(&operation)
            .expect_err("missing root partition should fail");

        assert!(error.to_string().contains("root partition is missing"));
        assert!(executor.runner.commands.is_empty());
    }

    #[test]
    fn system_executor_mounts_custom_partition_order() {
        let mut executor = SystemInstallationOperationExecutor::with_dependencies(
            RecordingCommandRunner::default(),
            RecordingInstallationBootstrapper::default(),
            RecordingInstallationPlanExecutor::default(),
        );
        let operation = InstallationOperation::MountFilesystems {
            device_path: "/dev/sdb".into(),
            partitions: vec![
                InstallationPartition::new(InstallationPartitionRole::Root, "ext4", None),
                InstallationPartition::new(
                    InstallationPartitionRole::EfiSystem,
                    "fat32",
                    Some(512),
                ),
            ],
            mounts: default_installation_mounts(),
        };

        executor
            .execute_operation(&operation)
            .expect("recording runner should accept command");

        assert_eq!(
            executor.runner.commands,
            vec![
                vec!["mkdir".to_owned(), "-p".to_owned(), "/target".to_owned(),],
                vec![
                    "mount".to_owned(),
                    "/dev/sdb1".to_owned(),
                    "/target".to_owned(),
                ],
                vec![
                    "mkdir".to_owned(),
                    "-p".to_owned(),
                    "/target/boot/efi".to_owned(),
                ],
                vec![
                    "mount".to_owned(),
                    "/dev/sdb2".to_owned(),
                    "/target/boot/efi".to_owned(),
                ],
            ]
        );
    }

    #[test]
    fn system_executor_rejects_missing_root_mount_before_execution() {
        let mut executor = SystemInstallationOperationExecutor::with_dependencies(
            RecordingCommandRunner::default(),
            RecordingInstallationBootstrapper::default(),
            RecordingInstallationPlanExecutor::default(),
        );
        let operation = InstallationOperation::MountFilesystems {
            device_path: "/dev/sdb".into(),
            partitions: default_installation_partitions(),
            mounts: vec![InstallationMount::new(
                InstallationPartitionRole::EfiSystem,
                "/target/boot/efi",
            )],
        };

        let error = executor
            .execute_operation(&operation)
            .expect_err("missing root mount should fail");

        assert!(error.to_string().contains("root mount is missing"));
        assert!(executor.runner.commands.is_empty());
    }

    #[test]
    fn system_executor_rejects_missing_efi_mount_before_execution() {
        let mut executor = SystemInstallationOperationExecutor::with_dependencies(
            RecordingCommandRunner::default(),
            RecordingInstallationBootstrapper::default(),
            RecordingInstallationPlanExecutor::default(),
        );
        let operation = InstallationOperation::MountFilesystems {
            device_path: "/dev/sdb".into(),
            partitions: default_installation_partitions(),
            mounts: vec![InstallationMount::new(
                InstallationPartitionRole::Root,
                "/target",
            )],
        };

        let error = executor
            .execute_operation(&operation)
            .expect_err("missing EFI mount should fail");

        assert!(error.to_string().contains("EFI mount is missing"));
        assert!(executor.runner.commands.is_empty());
    }

    #[test]
    fn default_layout_places_root_on_second_partition() {
        let root_partition_number = default_installation_partitions()
            .iter()
            .position(|partition| partition.role() == InstallationPartitionRole::Root)
            .map(|index| index + 1);

        assert_eq!(root_partition_number, Some(2));
    }

    #[test]
    fn system_executor_creates_mount_points_and_mounts_filesystems() {
        let mut executor = SystemInstallationOperationExecutor::with_dependencies(
            RecordingCommandRunner::default(),
            RecordingInstallationBootstrapper::default(),
            RecordingInstallationPlanExecutor::default(),
        );

        let operation = InstallationOperation::MountFilesystems {
            device_path: "/dev/sdb".into(),
            partitions: default_installation_partitions(),
            mounts: default_installation_mounts(),
        };

        executor
            .execute_operation(&operation)
            .expect("recording runner should accept command");

        assert_eq!(
            executor.runner.commands,
            vec![
                vec!["mkdir".to_owned(), "-p".to_owned(), "/target".to_owned(),],
                vec![
                    "mount".to_owned(),
                    "/dev/sdb2".to_owned(),
                    "/target".to_owned(),
                ],
                vec![
                    "mkdir".to_owned(),
                    "-p".to_owned(),
                    "/target/boot/efi".to_owned(),
                ],
                vec![
                    "mount".to_owned(),
                    "/dev/sdb1".to_owned(),
                    "/target/boot/efi".to_owned(),
                ],
            ]
        );
    }

    #[test]
    fn default_installation_mounts_define_root_and_efi() {
        let mounts = default_installation_mounts();

        assert_eq!(mounts.len(), 2);

        assert_eq!(mounts[0].role(), InstallationPartitionRole::Root);
        assert_eq!(mounts[0].mount_point(), std::path::Path::new("/target"));

        assert_eq!(mounts[1].role(), InstallationPartitionRole::EfiSystem);
        assert_eq!(
            mounts[1].mount_point(),
            std::path::Path::new("/target/boot/efi")
        );
    }

    #[test]
    fn system_executor_rejects_zero_efi_partition_size() {
        let mut executor = SystemInstallationOperationExecutor::with_dependencies(
            RecordingCommandRunner::default(),
            RecordingInstallationBootstrapper::default(),
            RecordingInstallationPlanExecutor::default(),
        );
        let operation = InstallationOperation::PartitionDisk {
            device_path: "/dev/sdb".into(),
            partitions: vec![
                InstallationPartition::new(InstallationPartitionRole::EfiSystem, "fat32", Some(0)),
                InstallationPartition::new(InstallationPartitionRole::Root, "ext4", None),
            ],
        };

        let error = executor
            .execute_operation(&operation)
            .expect_err("zero-sized EFI partition should fail");

        assert!(
            error
                .to_string()
                .contains("EFI partition size must be greater than zero")
        );
    }

    #[test]
    fn system_executor_rejects_efi_partition_without_size() {
        let mut executor = SystemInstallationOperationExecutor::with_dependencies(
            RecordingCommandRunner::default(),
            RecordingInstallationBootstrapper::default(),
            RecordingInstallationPlanExecutor::default(),
        );
        let operation = InstallationOperation::PartitionDisk {
            device_path: "/dev/sdb".into(),
            partitions: vec![
                InstallationPartition::new(InstallationPartitionRole::EfiSystem, "fat32", None),
                InstallationPartition::new(InstallationPartitionRole::Root, "ext4", None),
            ],
        };

        let error = executor
            .execute_operation(&operation)
            .expect_err("EFI partition without size should fail");

        assert!(error.to_string().contains("EFI partition size is missing"));
    }

    #[test]
    fn system_executor_rejects_partition_layout_without_efi() {
        let mut executor = SystemInstallationOperationExecutor::with_dependencies(
            RecordingCommandRunner::default(),
            RecordingInstallationBootstrapper::default(),
            RecordingInstallationPlanExecutor::default(),
        );
        let operation = InstallationOperation::PartitionDisk {
            device_path: "/dev/sdb".into(),
            partitions: vec![InstallationPartition::new(
                InstallationPartitionRole::Root,
                "ext4",
                None,
            )],
        };

        let error = executor
            .execute_operation(&operation)
            .expect_err("missing EFI partition should fail");

        assert!(error.to_string().contains("EFI partition is missing"));
    }

    #[test]
    fn system_executor_rejects_partition_layout_without_root() {
        let mut executor = SystemInstallationOperationExecutor::with_dependencies(
            RecordingCommandRunner::default(),
            RecordingInstallationBootstrapper::default(),
            RecordingInstallationPlanExecutor::default(),
        );
        let operation = InstallationOperation::PartitionDisk {
            device_path: "/dev/sdb".into(),
            partitions: vec![InstallationPartition::new(
                InstallationPartitionRole::EfiSystem,
                "fat32",
                Some(512),
            )],
        };

        let error = executor
            .execute_operation(&operation)
            .expect_err("missing root partition should fail");

        assert!(error.to_string().contains("root partition is missing"));
    }

    #[test]
    fn system_executor_rejects_missing_efi_partition() {
        let mut executor = SystemInstallationOperationExecutor::with_dependencies(
            RecordingCommandRunner::default(),
            RecordingInstallationBootstrapper::default(),
            RecordingInstallationPlanExecutor::default(),
        );
        let operation = InstallationOperation::CreateFilesystems {
            device_path: "/dev/sdb".into(),
            partitions: vec![InstallationPartition::new(
                InstallationPartitionRole::Root,
                "ext4",
                None,
            )],
        };

        let error = executor
            .execute_operation(&operation)
            .expect_err("missing EFI partition should fail");

        assert!(error.to_string().contains("EFI partition is missing"));
    }

    #[test]
    fn system_executor_rejects_missing_root_partition() {
        let mut executor = SystemInstallationOperationExecutor::with_dependencies(
            RecordingCommandRunner::default(),
            RecordingInstallationBootstrapper::default(),
            RecordingInstallationPlanExecutor::default(),
        );
        let operation = InstallationOperation::CreateFilesystems {
            device_path: "/dev/sdb".into(),
            partitions: vec![InstallationPartition::new(
                InstallationPartitionRole::EfiSystem,
                "fat32",
                Some(512),
            )],
        };

        let error = executor
            .execute_operation(&operation)
            .expect_err("missing root partition should fail");

        assert!(error.to_string().contains("root partition is missing"));
    }
    #[test]
    fn system_executor_rejects_unsupported_efi_filesystem() {
        let mut executor = SystemInstallationOperationExecutor::with_dependencies(
            RecordingCommandRunner::default(),
            RecordingInstallationBootstrapper::default(),
            RecordingInstallationPlanExecutor::default(),
        );
        let operation = InstallationOperation::CreateFilesystems {
            device_path: "/dev/sdb".into(),
            partitions: vec![
                InstallationPartition::new(InstallationPartitionRole::EfiSystem, "ext4", Some(512)),
                InstallationPartition::new(InstallationPartitionRole::Root, "ext4", None),
            ],
        };

        let error = executor
            .execute_operation(&operation)
            .expect_err("unsupported EFI filesystem should fail");

        assert!(error.to_string().contains("unsupported EFI filesystem"));
    }

    #[test]
    fn system_executor_rejects_unsupported_root_filesystem() {
        let mut executor = SystemInstallationOperationExecutor::with_dependencies(
            RecordingCommandRunner::default(),
            RecordingInstallationBootstrapper::default(),
            RecordingInstallationPlanExecutor::default(),
        );
        let operation = InstallationOperation::CreateFilesystems {
            device_path: "/dev/sdb".into(),
            partitions: vec![
                InstallationPartition::new(
                    InstallationPartitionRole::EfiSystem,
                    "fat32",
                    Some(512),
                ),
                InstallationPartition::new(InstallationPartitionRole::Root, "xfs", None),
            ],
        };

        let error = executor
            .execute_operation(&operation)
            .expect_err("unsupported root filesystem should fail");

        assert!(error.to_string().contains("unsupported root filesystem"));
    }
    #[test]
    fn system_executor_creates_efi_and_root_filesystems() {
        let mut executor = SystemInstallationOperationExecutor::with_dependencies(
            RecordingCommandRunner::default(),
            RecordingInstallationBootstrapper::default(),
            RecordingInstallationPlanExecutor::default(),
        );
        let operation = InstallationOperation::CreateFilesystems {
            device_path: "/dev/sdb".into(),
            partitions: default_installation_partitions(),
        };

        executor
            .execute_operation(&operation)
            .expect("recording runner should accept command");

        assert_eq!(
            executor.runner.commands,
            vec![
                vec![
                    "mkfs.fat".to_owned(),
                    "-F".to_owned(),
                    "32".to_owned(),
                    "/dev/sdb1".to_owned(),
                ],
                vec![
                    "mkfs.ext4".to_owned(),
                    "-F".to_owned(),
                    "/dev/sdb2".to_owned(),
                ],
            ]
        );
    }
    #[test]
    fn builds_partition_path_for_sd_device() {
        assert_eq!(
            partition_device_path(std::path::Path::new("/dev/sdb"), 1),
            std::path::PathBuf::from("/dev/sdb1")
        );
    }

    #[test]
    fn builds_partition_path_for_nvme_device() {
        assert_eq!(
            partition_device_path(std::path::Path::new("/dev/nvme0n1"), 2),
            std::path::PathBuf::from("/dev/nvme0n1p2")
        );
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
        let executor = SystemInstallationOperationExecutor::with_dependencies(
            RecordingCommandRunner::default(),
            RecordingInstallationBootstrapper::default(),
            RecordingInstallationPlanExecutor::default(),
        );
        assert!(executor.runner.commands.is_empty());
    }
    #[test]
    fn system_executor_sends_wipefs_command_for_prepare_disk() {
        let mut executor = SystemInstallationOperationExecutor::with_dependencies(
            RecordingCommandRunner::default(),
            RecordingInstallationBootstrapper::default(),
            RecordingInstallationPlanExecutor::default(),
        );
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
        let mut executor = SystemInstallationOperationExecutor::with_dependencies(
            FailingCommandRunner,
            RecordingInstallationBootstrapper::default(),
            RecordingInstallationPlanExecutor::default(),
        );
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
        let mut executor = SystemInstallationOperationExecutor::with_dependencies(
            RecordingCommandRunner::default(),
            RecordingInstallationBootstrapper::default(),
            RecordingInstallationPlanExecutor::default(),
        );
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
        let mut executor = SystemInstallationOperationExecutor::with_dependencies(
            RecordingCommandRunner::default(),
            RecordingInstallationBootstrapper::default(),
            RecordingInstallationPlanExecutor::default(),
        );
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
