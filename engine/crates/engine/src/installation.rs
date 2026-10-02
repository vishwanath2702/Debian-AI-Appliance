use model::{DiscoveredStorage, DiscoveredStorageId, InstallationIntent, Plan, UserConfiguration};

use std::{
    io::{self, Write},
    path::PathBuf,
    process::{Command, Stdio},
};

use crate::{SystemContentImportFileSystem, SystemContentImportOperationExecutor};

const RUNTIME_PAYLOAD_DIRECTORY: &str = "/run/live/medium/daia";

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
const REQUIRED_INSTALLATION_COMMANDS: &[&str] = &[
    "wipefs",
    "parted",
    "mkfs.fat",
    "mkfs.ext4",
    "mkdir",
    "mount",
    "umount",
    "blkid",
    "sudo",
    "unsquashfs",
    "systemctl",
];

fn installation_command_path_is_executable(path: &std::path::Path) -> bool {
    use std::os::unix::fs::PermissionsExt;

    path.metadata()
        .is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
}

fn installation_command_exists_in_path(command: &str, path: &std::ffi::OsStr) -> bool {
    std::env::split_paths(path)
        .any(|directory| installation_command_path_is_executable(&directory.join(command)))
}

fn installation_command_exists(command: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|path| installation_command_exists_in_path(command, &path))
}

/// Validates that the commands required for system installation are available.
///
/// # Errors
///
/// Returns an error when a required installation command is unavailable.
pub fn validate_installation_commands() -> io::Result<()> {
    for command in REQUIRED_INSTALLATION_COMMANDS {
        if !installation_command_exists(command) {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("required installation command not found: {command}"),
            ));
        }
    }

    let chroot = std::path::Path::new("/usr/sbin/chroot");

    if !installation_command_path_is_executable(chroot) {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "required installation command not found: {}",
                chroot.display()
            ),
        ));
    }

    Ok(())
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
    /// Install the prepared DAIA system image into the target filesystem.
    InstallSystemImage {
        /// Target root filesystem receiving the installed system.
        root: PathBuf,

        /// SquashFS image containing the prepared DAIA system.
        image: PathBuf,
    },

    /// Remove live-environment configuration from the installed system.
    NormalizeInstalledSystem {
        /// Target root filesystem containing the installed system.
        root: PathBuf,
    },

    /// Create the configured human administrator in the installed system.
    CreateAdministrator {
        /// Target root filesystem containing the installed system.
        root: PathBuf,

        /// Non-secret identity of the human administrator.
        user: UserConfiguration,
    },

    ConfigureFstab {
        device_path: PathBuf,
        partitions: Vec<InstallationPartition>,
        mounts: Vec<InstallationMount>,
    },

    /// Import selected external content into the installed appliance.
    ImportContent {
        content: crate::PreparedContentImport,
    },

    /// Persists appliance state while the installed target is still mounted.
    PersistApplianceState {
        root: PathBuf,
        profile_name: String,
        model_realization_intents: Vec<model::ModelRealizationIntent>,
    },

    /// Deploys the DAIA runtime into the installed system.
    DeployRuntime { root: PathBuf },

    /// Enables DAIA first-boot initialization in the installed system.
    EnableFirstBoot { root: PathBuf },

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
fn copy_directory_contents(
    source: &std::path::Path,
    destination: &std::path::Path,
) -> io::Result<()> {
    std::fs::create_dir_all(destination)?;

    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());

        if source_path.is_dir() {
            copy_directory_contents(&source_path, &destination_path)?;
        } else if source_path.is_file() {
            std::fs::copy(&source_path, &destination_path)?;
        }
    }

    Ok(())
}

/// Executes installation operations against the host system.
pub trait InstallationCommandRunner {
    fn status(&mut self, command: &mut Command) -> io::Result<()>;

    fn status_with_input(&mut self, command: &mut Command, input: &[u8]) -> io::Result<()>;

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

    fn status_with_input(&mut self, command: &mut Command, input: &[u8]) -> io::Result<()> {
        command.stdin(Stdio::piped());

        let mut child = command.spawn()?;

        child
            .stdin
            .take()
            .ok_or_else(|| io::Error::other("installation command stdin is unavailable"))?
            .write_all(input)?;

        let status = child.wait()?;

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

pub struct SystemInstallationOperationExecutor<R, W = SystemInstallationFileWriter> {
    runner: R,
    file_writer: W,
    target_root: PathBuf,
    runtime_payload_directory: PathBuf,
    imported_content: Vec<model::ImportedContentItem>,
    root_password: String,
    administrator_password: String,
}

#[cfg(test)]
impl<R> SystemInstallationOperationExecutor<R, SystemInstallationFileWriter>
where
    R: InstallationCommandRunner,
{
    fn with_dependencies(runner: R) -> Self {
        Self {
            runner,
            file_writer: SystemInstallationFileWriter,
            target_root: PathBuf::from("/target"),
            runtime_payload_directory: PathBuf::from(RUNTIME_PAYLOAD_DIRECTORY),
            imported_content: Vec::new(),
            root_password: String::new(),
            administrator_password: String::new(),
        }
    }
    fn with_target_root(mut self, target_root: PathBuf) -> Self {
        self.target_root = target_root;
        self
    }

    fn with_runtime_payload_directory(mut self, runtime_payload_directory: PathBuf) -> Self {
        self.runtime_payload_directory = runtime_payload_directory;
        self
    }

    fn with_root_password(mut self, root_password: impl Into<String>) -> Self {
        self.root_password = root_password.into();
        self
    }

    fn with_administrator_password(mut self, administrator_password: impl Into<String>) -> Self {
        self.administrator_password = administrator_password.into();
        self
    }
}

impl<R, W> SystemInstallationOperationExecutor<R, W> {
    /// Returns content realized during successful installation operations.
    #[must_use]
    pub fn imported_content(&self) -> &[model::ImportedContentItem] {
        &self.imported_content
    }
}

#[cfg(test)]
impl<R, W> SystemInstallationOperationExecutor<R, W>
where
    R: InstallationCommandRunner,
    W: InstallationFileWriter,
{
    fn with_all_dependencies(runner: R, file_writer: W) -> Self {
        Self {
            runner,
            file_writer,
            target_root: PathBuf::from("/target"),
            runtime_payload_directory: PathBuf::from(RUNTIME_PAYLOAD_DIRECTORY),
            imported_content: Vec::new(),
            root_password: String::new(),
            administrator_password: String::new(),
        }
    }
}

impl
    SystemInstallationOperationExecutor<
        ProcessInstallationCommandRunner,
        SystemInstallationFileWriter,
    >
{
    /// Creates a production installation executor.
    #[must_use]
    pub fn new(root_password: String, administrator_password: String) -> Self {
        Self {
            runner: ProcessInstallationCommandRunner,
            file_writer: SystemInstallationFileWriter,
            target_root: PathBuf::from("/target"),
            runtime_payload_directory: PathBuf::from(RUNTIME_PAYLOAD_DIRECTORY),
            imported_content: Vec::new(),
            root_password,
            administrator_password,
        }
    }
}

impl<R, W> InstallationOperationExecutor for SystemInstallationOperationExecutor<R, W>
where
    R: InstallationCommandRunner,
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
                    .arg("100%")
                    .arg("set")
                    .arg("1")
                    .arg("esp")
                    .arg("on");

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
            InstallationOperation::InstallSystemImage { root, image } => {
                let mut command = Command::new("unsquashfs");

                command.arg("-f").arg("-d").arg(root).arg(image);

                self.runner.status(&mut command)
            }
            InstallationOperation::NormalizeInstalledSystem { root } => {
                let root_argument = format!("--root={}", root.display());

                let mut command = Command::new("systemctl");
                command
                    .arg(&root_argument)
                    .arg("disable")
                    .arg("daia-installer.service");
                self.runner.status(&mut command)?;

                let installer_service = root.join("etc/systemd/system/daia-installer.service");

                match std::fs::symlink_metadata(&installer_service) {
                    Ok(_) => std::fs::remove_file(&installer_service)?,
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                    Err(error) => return Err(error),
                }

                let mut command = Command::new("systemctl");
                command
                    .arg(&root_argument)
                    .arg("set-default")
                    .arg("graphical.target");
                self.runner.status(&mut command)?;

                let mut command = Command::new("systemctl");
                command
                    .arg(&root_argument)
                    .arg("enable")
                    .arg("getty@tty1.service");
                self.runner.status(&mut command)
            }
            InstallationOperation::CreateAdministrator { root, user } => {
                let mut command = Command::new("chroot");

                command
                    .arg(root)
                    .arg("useradd")
                    .arg("--create-home")
                    .arg("--user-group")
                    .arg("--groups")
                    .arg("sudo")
                    .arg("--shell")
                    .arg("/bin/bash")
                    .arg("--comment")
                    .arg(user.display_name())
                    .arg(user.username());

                self.runner.status(&mut command)?;

                let mut password_command = Command::new("chroot");

                password_command.arg(root).arg("chpasswd");

                let password_input = format!(
                    "root:{}\n{}:{}\n",
                    self.root_password,
                    user.username(),
                    self.administrator_password
                );

                self.runner
                    .status_with_input(&mut password_command, password_input.as_bytes())?;

                Ok(())
            }
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
            InstallationOperation::ImportContent { content } => {
                let mut executor = SystemContentImportOperationExecutor::new(
                    SystemContentImportFileSystem::with_root(&self.target_root),
                );

                let imported_content = content.execute(&mut executor)?;
                self.imported_content.extend(imported_content);

                Ok(())
            }
            InstallationOperation::PersistApplianceState {
                root,
                profile_name,
                model_realization_intents,
            } => {
                let mut appliance_state = state::ApplianceState::new(profile_name);
                appliance_state.apply_content_imports(
                    self.imported_content.iter().cloned(),
                    model_realization_intents,
                );

                appliance_state.write_to_root(root)
            }
            InstallationOperation::DeployRuntime { root } => {
                copy_directory_contents(&self.runtime_payload_directory, root)
            }
            InstallationOperation::EnableFirstBoot { root } => {
                let wants_directory = root.join("etc/systemd/system/multi-user.target.wants");
                let service_link = wants_directory.join("daia-firstboot.service");

                std::fs::create_dir_all(&wants_directory)?;

                match std::fs::symlink_metadata(&service_link) {
                    Ok(_) => std::fs::remove_file(&service_link)?,
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                    Err(error) => return Err(error),
                }

                std::os::unix::fs::symlink(
                    "/etc/systemd/system/daia-firstboot.service",
                    service_link,
                )?;

                Ok(())
            }
            InstallationOperation::PrepareTargetRuntime { root } => {
                let target_dev = root.join("dev");

                let mut mkdir = Command::new("mkdir");
                mkdir.arg("-p").arg(&target_dev);

                self.runner.status(&mut mkdir)?;

                let mut mount = Command::new("mount");
                mount.arg("--bind").arg("/dev").arg(&target_dev);

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
                mount.arg("--bind").arg("/sys").arg(&target_sys);

                self.runner.status(&mut mount)?;
                let target_run = root.join("run");

                let mut mkdir = Command::new("mkdir");
                mkdir.arg("-p").arg(&target_run);

                self.runner.status(&mut mkdir)?;

                let mut mount = Command::new("mount");
                mount.arg("--bind").arg("/run").arg(&target_run);

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
                        "--removable",
                    ],
                );

                self.runner.status(&mut grub_install)?;

                let mut update_grub = command_in_root(root, "update-grub", &[]);

                self.runner.status(&mut update_grub)
            }
            InstallationOperation::CleanupTargetRuntime { root } => {
                let mut first_error = None;

                for target in [
                    root.join("run"),
                    root.join("sys"),
                    root.join("proc"),
                    root.join("dev"),
                ] {
                    let mut unmount = Command::new("umount");
                    unmount.arg("-R").arg(target);

                    if let Err(error) = self.runner.status(&mut unmount) {
                        if first_error.is_none() {
                            first_error = Some(error);
                        }
                    }
                }

                match first_error {
                    Some(error) => Err(error),
                    None => Ok(()),
                }
            }

            InstallationOperation::UnmountFilesystems { mounts } => {
                let efi_mount = mounts
                    .iter()
                    .find(|mount| mount.role() == InstallationPartitionRole::EfiSystem)
                    .ok_or_else(|| io::Error::other("EFI mount is missing"))?;

                let mut first_error = None;

                let mut unmount = Command::new("umount");
                unmount.arg(efi_mount.mount_point());

                if let Err(error) = self.runner.status(&mut unmount) {
                    first_error = Some(error);
                }

                if let Some(root_mount) = mounts
                    .iter()
                    .find(|mount| mount.role() == InstallationPartitionRole::Root)
                {
                    let mut unmount = Command::new("umount");
                    unmount.arg(root_mount.mount_point());

                    if let Err(error) = self.runner.status(&mut unmount) {
                        if first_error.is_none() {
                            first_error = Some(error);
                        }
                    }
                } else if first_error.is_none() {
                    first_error = Some(io::Error::other("root mount is missing"));
                }

                match first_error {
                    Some(error) => Err(error),
                    None => Ok(()),
                }
            }
        }
    }
}

impl<R, W> InstallationExecutor for SystemInstallationOperationExecutor<R, W>
where
    R: InstallationCommandRunner,
    W: InstallationFileWriter,
{
    type Error = io::Error;

    fn execute(&mut self, installation: &PreparedInstallation) -> Result<(), Self::Error> {
        let plan = installation.installation_plan();

        plan.execute_with_cleanup(self)
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
    /// Executes planned operations while allowing cleanup operations to run after failure.
    ///
    /// # Errors
    ///
    /// Returns the first operation error after attempting all remaining cleanup operations.
    pub fn execute_with_cleanup<E>(&self, executor: &mut E) -> Result<(), E::Error>
    where
        E: InstallationOperationExecutor,
    {
        let mut first_error = None;

        for operation in &self.operations {
            if first_error.is_some() && !operation.is_cleanup() {
                continue;
            }

            if let Err(error) = executor.execute_operation(operation) {
                if first_error.is_none() {
                    first_error = Some(error);
                }
            }
        }

        match first_error {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
}
/// A validated installation ready for later execution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedInstallation {
    intent: InstallationIntent,
    storage: DiscoveredStorage,
    plans: Vec<Plan>,
    system_image: PathBuf,
}

impl PreparedInstallation {
    /// Creates a prepared installation from validated components.
    #[must_use]
    pub const fn new(
        intent: InstallationIntent,
        storage: DiscoveredStorage,
        plans: Vec<Plan>,
        system_image: PathBuf,
    ) -> Self {
        Self {
            intent,
            storage,
            plans,
            system_image,
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
            InstallationOperation::InstallSystemImage {
                root: "/target".into(),
                image: self.system_image.clone(),
            },
            InstallationOperation::NormalizeInstalledSystem {
                root: "/target".into(),
            },
            InstallationOperation::CreateAdministrator {
                root: "/target".into(),
                user: self.intent.user().clone(),
            },
            InstallationOperation::ConfigureFstab {
                device_path: self.storage.device_path().to_path_buf(),
                partitions: default_installation_partitions(),
                mounts: default_installation_mounts(),
            },
            InstallationOperation::DeployRuntime {
                root: "/target".into(),
            },
            InstallationOperation::EnableFirstBoot {
                root: "/target".into(),
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
    /// Returns the prepared DAIA system image used for installation.
    #[must_use]
    pub const fn system_image(&self) -> &PathBuf {
        &self.system_image
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
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedApplianceInstallation {
    installation: PreparedInstallation,
    content: crate::PreparedContentImport,
    model_realization_intents: Vec<model::ModelRealizationIntent>,
}

impl PreparedApplianceInstallation {
    pub const fn new(
        installation: PreparedInstallation,
        content: crate::PreparedContentImport,
        model_realization_intents: Vec<model::ModelRealizationIntent>,
    ) -> Self {
        Self {
            installation,
            content,
            model_realization_intents,
        }
    }

    pub const fn installation(&self) -> &PreparedInstallation {
        &self.installation
    }

    pub const fn content(&self) -> &crate::PreparedContentImport {
        &self.content
    }

    /// Returns model realization intents associated with the prepared content.
    #[must_use]
    pub fn model_realization_intents(&self) -> &[model::ModelRealizationIntent] {
        &self.model_realization_intents
    }

    #[must_use]
    pub fn installation_plan(&self) -> InstallationPlan {
        let installation_plan = self.installation.installation_plan();
        let additional_operations = if self.content.intent().items().is_empty() {
            1
        } else {
            2
        };
        let mut operations =
            Vec::with_capacity(installation_plan.operations().len() + additional_operations);

        for operation in installation_plan.operations() {
            operations.push(operation.clone());

            if matches!(operation, InstallationOperation::ConfigureFstab { .. }) {
                if !self.content.intent().items().is_empty() {
                    operations.push(InstallationOperation::ImportContent {
                        content: self.content.clone(),
                    });
                }

                operations.push(InstallationOperation::PersistApplianceState {
                    root: PathBuf::from("/target"),
                    profile_name: self.installation.intent().profile_name().to_owned(),
                    model_realization_intents: self.model_realization_intents.clone(),
                });
            }
        }

        InstallationPlan::new(operations)
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

        plan.execute_with_cleanup(self)?;

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
        InstallationCommandRunner, InstallationExecutor, InstallationFileWriter, InstallationMount,
        InstallationOperation, InstallationOperationExecutor, InstallationPartition,
        InstallationPartitionRole, InstallationPlan, PathBuf, PreparedInstallation,
        ProcessInstallationCommandRunner, REQUIRED_INSTALLATION_COMMANDS,
        RUNTIME_PAYLOAD_DIRECTORY, SystemInstallationOperationExecutor, command_in_root,
        default_installation_mounts, default_installation_partitions, filesystem_uuid,
        installation_command_exists, installation_command_exists_in_path,
        installation_command_path_is_executable, installation_fstab, installed_mount_point,
        partition_device_path, validate_installation_commands,
    };
    use model::{DiscoveredStorage, DiscoveredStorageId, InstallationIntent, StorageKind};

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
        inputs: Vec<Vec<u8>>,
        outputs: Vec<Vec<u8>>,
    }
    struct FailingCommandRunner;

    impl InstallationCommandRunner for FailingCommandRunner {
        fn status(&mut self, _command: &mut Command) -> io::Result<()> {
            Err(io::Error::other("command failed"))
        }

        fn status_with_input(&mut self, _command: &mut Command, _input: &[u8]) -> io::Result<()> {
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

        fn status_with_input(&mut self, command: &mut Command, input: &[u8]) -> io::Result<()> {
            let mut recorded = vec![command.get_program().to_string_lossy().into_owned()];

            recorded.extend(
                command
                    .get_args()
                    .map(|arg| arg.to_string_lossy().into_owned()),
            );

            self.commands.push(recorded);
            self.inputs.push(input.to_vec());

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

    impl RecordingCommandRunner {
        fn with_outputs(outputs: Vec<Vec<u8>>) -> Self {
            Self {
                commands: Vec::new(),
                inputs: Vec::new(),
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

        fn status_with_input(&mut self, command: &mut Command, _input: &[u8]) -> io::Result<()> {
            self.status(command)
        }

        fn output(&mut self, _command: &mut Command) -> io::Result<Vec<u8>> {
            Err(io::Error::other("unexpected command output request"))
        }
    }

    #[test]
    fn installation_plan_unmounts_filesystems_after_content_import_failure() {
        let temporary_directory =
            tempfile::tempdir().expect("temporary directory should be created");

        let source = temporary_directory.path().join("missing-model.gguf");

        let item = model::ExternalContentItem::new(model::ContentSourceId::new("local"), source);

        let content = crate::PreparedContentImport::new(
            model::ContentImportIntent::new(vec![item.id().clone()]),
            vec![item],
            model::ContentImportDestination::new("/var/lib/daia/content"),
        );

        let target_root = temporary_directory.path().join("target");

        let mut executor = SystemInstallationOperationExecutor::with_dependencies(
            RecordingCommandRunner::default(),
        )
        .with_target_root(target_root);

        let plan = InstallationPlan::new(vec![
            InstallationOperation::ImportContent { content },
            InstallationOperation::UnmountFilesystems {
                mounts: default_installation_mounts(),
            },
        ]);

        let error = plan
            .execute_with_cleanup(&mut executor)
            .expect_err("content import failure should be reported");

        assert_eq!(error.kind(), io::ErrorKind::NotFound);

        assert_eq!(
            executor.runner.commands,
            vec![
                vec!["umount".to_owned(), "/target/boot/efi".to_owned()],
                vec!["umount".to_owned(), "/target".to_owned()],
            ]
        );
    }

    #[test]
    fn runtime_payload_directory_uses_live_boot_medium() {
        assert_eq!(RUNTIME_PAYLOAD_DIRECTORY, "/run/live/medium/daia");
    }

    #[test]
    fn system_executor_installs_prepared_system_image() {
        let runner = RecordingCommandRunner::default();
        let mut executor = SystemInstallationOperationExecutor::with_dependencies(runner);

        executor
            .execute_operation(&InstallationOperation::InstallSystemImage {
                root: PathBuf::from("/target"),
                image: PathBuf::from("/run/live/medium/live/filesystem.squashfs"),
            })
            .expect("prepared system image should be installed");

        assert_eq!(
            executor.runner.commands,
            vec![vec![
                "unsquashfs".to_owned(),
                "-f".to_owned(),
                "-d".to_owned(),
                "/target".to_owned(),
                "/run/live/medium/live/filesystem.squashfs".to_owned(),
            ]]
        );
    }

    #[test]
    fn system_executor_normalizes_installed_system() {
        let temporary_directory =
            tempfile::tempdir().expect("temporary directory should be created");
        let target_root = temporary_directory.path().join("target");
        let systemd_directory = target_root.join("etc/systemd/system");

        std::fs::create_dir_all(&systemd_directory).expect("systemd directory should be created");

        let installer_service = systemd_directory.join("daia-installer.service");
        std::fs::write(&installer_service, b"live installer service")
            .expect("live installer service should be written");

        let runner = RecordingCommandRunner::default();
        let mut executor = SystemInstallationOperationExecutor::with_dependencies(runner);

        executor
            .execute_operation(&InstallationOperation::NormalizeInstalledSystem {
                root: target_root.clone(),
            })
            .expect("installed system should be normalized");

        assert!(
            !installer_service.exists(),
            "live installer service must be removed from the installed system"
        );

        let root_argument = format!("--root={}", target_root.display());

        assert_eq!(
            executor.runner.commands,
            vec![
                vec![
                    "systemctl".to_owned(),
                    root_argument.clone(),
                    "disable".to_owned(),
                    "daia-installer.service".to_owned(),
                ],
                vec![
                    "systemctl".to_owned(),
                    root_argument.clone(),
                    "set-default".to_owned(),
                    "graphical.target".to_owned(),
                ],
                vec![
                    "systemctl".to_owned(),
                    root_argument,
                    "enable".to_owned(),
                    "getty@tty1.service".to_owned(),
                ],
            ]
        );
    }

    #[test]
    fn system_executor_normalizes_installed_system_when_live_service_is_absent() {
        let temporary_directory =
            tempfile::tempdir().expect("temporary directory should be created");
        let target_root = temporary_directory.path().join("target");

        std::fs::create_dir_all(target_root.join("etc/systemd/system"))
            .expect("systemd directory should be created");

        let runner = RecordingCommandRunner::default();
        let mut executor = SystemInstallationOperationExecutor::with_dependencies(runner);

        executor
            .execute_operation(&InstallationOperation::NormalizeInstalledSystem {
                root: target_root,
            })
            .expect("normalization should tolerate an absent live installer service");

        assert_eq!(executor.runner.commands.len(), 3);
    }

    #[test]
    fn system_executor_deploys_runtime_file_under_target_root() {
        let temporary_directory =
            tempfile::tempdir().expect("temporary directory should be created");

        let runtime_payload_directory = temporary_directory.path().join("payload");
        std::fs::create_dir_all(&runtime_payload_directory)
            .expect("runtime payload directory should be created");

        std::fs::write(
            runtime_payload_directory.join("runtime.txt"),
            b"runtime data",
        )
        .expect("runtime payload file should be written");

        let target_root = temporary_directory.path().join("target");
        std::fs::create_dir_all(&target_root).expect("target root should be created");

        let mut executor = SystemInstallationOperationExecutor::with_dependencies(
            RecordingCommandRunner::default(),
        )
        .with_runtime_payload_directory(runtime_payload_directory);

        executor
            .execute_operation(&InstallationOperation::DeployRuntime {
                root: target_root.clone(),
            })
            .expect("runtime deployment should succeed");

        assert_eq!(
            std::fs::read(target_root.join("runtime.txt"))
                .expect("deployed runtime file should exist"),
            b"runtime data"
        );
    }

    #[test]
    fn system_executor_deploys_runtime_directories_under_target_root() {
        let temporary_directory =
            tempfile::tempdir().expect("temporary directory should be created");

        let runtime_payload_directory = temporary_directory.path().join("payload");
        let nested_runtime_directory = runtime_payload_directory.join("opt/daia");

        std::fs::create_dir_all(&nested_runtime_directory)
            .expect("nested runtime payload directory should be created");

        std::fs::write(
            nested_runtime_directory.join("runtime.txt"),
            b"nested runtime data",
        )
        .expect("nested runtime payload file should be written");

        let target_root = temporary_directory.path().join("target");

        let mut executor = SystemInstallationOperationExecutor::with_dependencies(
            RecordingCommandRunner::default(),
        )
        .with_runtime_payload_directory(runtime_payload_directory);

        executor
            .execute_operation(&InstallationOperation::DeployRuntime {
                root: target_root.clone(),
            })
            .expect("runtime deployment should succeed");

        assert_eq!(
            std::fs::read(target_root.join("opt/daia/runtime.txt"))
                .expect("nested deployed runtime file should exist"),
            b"nested runtime data"
        );
    }

    #[cfg(unix)]
    #[test]
    fn system_executor_preserves_runtime_file_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let temporary_directory =
            tempfile::tempdir().expect("temporary directory should be created");

        let runtime_payload_directory = temporary_directory.path().join("payload");
        let runtime_script = runtime_payload_directory.join("opt/daia/bootstrap.sh");

        std::fs::create_dir_all(
            runtime_script
                .parent()
                .expect("runtime script should have a parent directory"),
        )
        .expect("runtime payload directory should be created");

        std::fs::write(&runtime_script, b"#!/bin/sh\n").expect("runtime script should be written");

        std::fs::set_permissions(&runtime_script, std::fs::Permissions::from_mode(0o755))
            .expect("runtime script permissions should be set");

        let target_root = temporary_directory.path().join("target");

        let mut executor = SystemInstallationOperationExecutor::with_dependencies(
            RecordingCommandRunner::default(),
        )
        .with_runtime_payload_directory(runtime_payload_directory);

        executor
            .execute_operation(&InstallationOperation::DeployRuntime {
                root: target_root.clone(),
            })
            .expect("runtime deployment should succeed");

        let deployed_mode = std::fs::metadata(target_root.join("opt/daia/bootstrap.sh"))
            .expect("deployed runtime script should exist")
            .permissions()
            .mode()
            & 0o777;

        assert_eq!(deployed_mode, 0o755);
    }

    #[test]
    fn system_executor_rejects_missing_runtime_payload_directory() {
        let temporary_directory =
            tempfile::tempdir().expect("temporary directory should be created");

        let runtime_payload_directory = temporary_directory.path().join("missing-payload");
        let target_root = temporary_directory.path().join("target");

        let mut executor = SystemInstallationOperationExecutor::with_dependencies(
            RecordingCommandRunner::default(),
        )
        .with_runtime_payload_directory(runtime_payload_directory);

        let error = executor
            .execute_operation(&InstallationOperation::DeployRuntime { root: target_root })
            .expect_err("missing runtime payload should fail deployment");

        assert_eq!(error.kind(), io::ErrorKind::NotFound);
    }

    #[test]
    fn system_executor_sets_account_passwords_through_stdin() {
        let mut executor = SystemInstallationOperationExecutor::with_dependencies(
            RecordingCommandRunner::default(),
        )
        .with_root_password("root-secret-password")
        .with_administrator_password("admin-secret-password");

        executor
            .execute_operation(&InstallationOperation::CreateAdministrator {
                root: "/target".into(),
                user: model::UserConfiguration::new("admin", "DAIA Administrator"),
            })
            .expect("administrator creation should succeed");

        assert_eq!(
            executor.runner.commands,
            vec![
                vec![
                    "chroot",
                    "/target",
                    "useradd",
                    "--create-home",
                    "--user-group",
                    "--groups",
                    "sudo",
                    "--shell",
                    "/bin/bash",
                    "--comment",
                    "DAIA Administrator",
                    "admin",
                ],
                vec!["chroot", "/target", "chpasswd"],
            ]
        );
        assert_eq!(
            executor.runner.inputs,
            vec![b"root:root-secret-password\nadmin:admin-secret-password\n".to_vec()]
        );
        assert!(executor.runner.commands.iter().flatten().all(|argument| {
            !argument.contains("root-secret-password")
                && !argument.contains("admin-secret-password")
        }));
    }

    #[cfg(unix)]
    #[test]
    fn system_executor_enables_first_boot_service() {
        let temporary_directory =
            tempfile::tempdir().expect("temporary directory should be created");

        let target_root = temporary_directory.path().join("target");

        let mut executor = SystemInstallationOperationExecutor::with_dependencies(
            RecordingCommandRunner::default(),
        );

        executor
            .execute_operation(&InstallationOperation::EnableFirstBoot {
                root: target_root.clone(),
            })
            .expect("first-boot activation should succeed");

        let service_link =
            target_root.join("etc/systemd/system/multi-user.target.wants/daia-firstboot.service");

        assert_eq!(
            std::fs::read_link(service_link).expect("first-boot service symlink should exist"),
            std::path::PathBuf::from("/etc/systemd/system/daia-firstboot.service")
        );
    }

    #[cfg(unix)]
    #[test]
    fn system_executor_can_enable_first_boot_service_twice() {
        let temporary_directory =
            tempfile::tempdir().expect("temporary directory should be created");

        let target_root = temporary_directory.path().join("target");

        let mut executor = SystemInstallationOperationExecutor::with_dependencies(
            RecordingCommandRunner::default(),
        );

        let operation = InstallationOperation::EnableFirstBoot {
            root: target_root.clone(),
        };

        executor
            .execute_operation(&operation)
            .expect("first first-boot activation should succeed");

        executor
            .execute_operation(&operation)
            .expect("second first-boot activation should succeed");

        assert_eq!(
            std::fs::read_link(
                target_root
                    .join("etc/systemd/system/multi-user.target.wants/daia-firstboot.service")
            )
            .expect("first-boot service symlink should exist"),
            std::path::PathBuf::from("/etc/systemd/system/daia-firstboot.service")
        );
    }

    #[test]
    fn system_executor_imports_content_under_target_root() {
        let temporary_directory =
            tempfile::tempdir().expect("temporary directory should be created");

        let source = temporary_directory.path().join("model.gguf");
        std::fs::write(&source, b"model data").expect("source content should be written");

        let item = model::ExternalContentItem::new(model::ContentSourceId::new("local"), source);
        let source_item_id = item.id().clone();

        let content = crate::PreparedContentImport::new(
            model::ContentImportIntent::new(vec![item.id().clone()]),
            vec![item],
            model::ContentImportDestination::new("/var/lib/daia/content"),
        );

        let matching_realization_intent = model::ModelRealizationIntent::new(
            model::ModelRealizationId::new("model"),
            model::InferenceEngineId::ollama(),
            source_item_id,
        );

        let unmatched_realization_intent = model::ModelRealizationIntent::new(
            model::ModelRealizationId::new("missing-model"),
            model::InferenceEngineId::ollama(),
            model::ExternalContentItemId::new("missing-source"),
        );

        let target_root = temporary_directory.path().join("target");

        let mut executor = SystemInstallationOperationExecutor::with_dependencies(
            RecordingCommandRunner::default(),
        )
        .with_target_root(target_root.clone());

        executor
            .execute_operation(&InstallationOperation::ImportContent { content })
            .expect("content import should succeed");

        assert_eq!(
            std::fs::read(target_root.join("var/lib/daia/content/model.gguf"))
                .expect("imported content should exist"),
            b"model data"
        );

        assert_eq!(executor.imported_content().len(), 1);
        assert_eq!(
            executor.imported_content()[0].path(),
            target_root.join("var/lib/daia/content/model.gguf")
        );

        executor
            .execute_operation(&InstallationOperation::PersistApplianceState {
                root: target_root.clone(),
                profile_name: "desktop".to_owned(),
                model_realization_intents: vec![
                    matching_realization_intent,
                    unmatched_realization_intent,
                ],
            })
            .expect("appliance state persistence should succeed");

        let persisted_state = std::fs::read_to_string(target_root.join("var/lib/daia/state.json"))
            .expect("appliance state should be written beneath target root");

        let persisted_state: serde_json::Value =
            serde_json::from_str(&persisted_state).expect("appliance state should be valid JSON");

        assert_eq!(persisted_state["profile_name"], "desktop");

        let imported_content = persisted_state["imported_content"]
            .as_array()
            .expect("imported content should be an array");

        assert_eq!(imported_content.len(), 1);
        assert_eq!(
            imported_content[0]["source_item_id"],
            executor.imported_content()[0].source_item_id().as_str()
        );
        assert_eq!(
            imported_content[0]["path"],
            target_root
                .join("var/lib/daia/content/model.gguf")
                .to_string_lossy()
                .as_ref()
        );

        let model_realizations = persisted_state["model_realizations"]
            .as_array()
            .expect("model realizations should be an array");

        assert_eq!(
            model_realizations.len(),
            1,
            "only intents backed by successfully imported content should persist"
        );
        assert_eq!(model_realizations[0]["id"], "model");
        assert_eq!(model_realizations[0]["engine"], "ollama");
        assert_eq!(
            model_realizations[0]["content"]["source_item_id"],
            executor.imported_content()[0].source_item_id().as_str()
        );
        assert_eq!(
            model_realizations[0]["content"]["path"],
            target_root
                .join("var/lib/daia/content/model.gguf")
                .to_string_lossy()
                .as_ref()
        );
    }

    #[test]
    fn system_executor_implements_installation_executor() {
        let intent = InstallationIntent::new(
            "desktop",
            DiscoveredStorageId::new("serial:usb-disk"),
            model::UserConfiguration::new("admin", "DAIA Administrator"),
        );

        let storage = DiscoveredStorage::new("serial:usb-disk", StorageKind::Removable, "/dev/sdb");

        let prepared = PreparedInstallation::new(
            intent,
            storage,
            Vec::new(),
            PathBuf::from("/run/live/medium/live/filesystem.squashfs"),
        );

        let mut executor =
            SystemInstallationOperationExecutor::with_dependencies(FailingCommandRunner);

        let error = InstallationExecutor::execute(&mut executor, &prepared)
            .expect_err("system installation should report command failure");

        assert_eq!(error.to_string(), "command failed");
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
    fn installation_plan_unmounts_filesystems_after_mount_failure() {
        let mut executor =
            SystemInstallationOperationExecutor::with_dependencies(FailingAtCommandRunner {
                commands: Vec::new(),
                fail_at: 3,
            });

        let plan = InstallationPlan::new(vec![
            InstallationOperation::MountFilesystems {
                device_path: "/dev/sdb".into(),
                partitions: default_installation_partitions(),
                mounts: default_installation_mounts(),
            },
            InstallationOperation::UnmountFilesystems {
                mounts: default_installation_mounts(),
            },
        ]);

        let error = plan
            .execute_with_cleanup(&mut executor)
            .expect_err("mount failure should be reported");

        assert_eq!(error.to_string(), "command failed");

        assert_eq!(
            executor.runner.commands,
            vec![
                vec!["mkdir".to_owned(), "-p".to_owned(), "/target".to_owned()],
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
                vec!["umount".to_owned(), "/target/boot/efi".to_owned()],
                vec!["umount".to_owned(), "/target".to_owned()],
            ]
        );
    }

    #[test]
    fn system_executor_unmounts_installation_filesystems() {
        let mut executor = SystemInstallationOperationExecutor::with_dependencies(
            RecordingCommandRunner::default(),
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
    fn system_executor_continues_filesystem_unmount_after_command_failure() {
        let mut executor =
            SystemInstallationOperationExecutor::with_dependencies(FailingAtCommandRunner {
                commands: Vec::new(),
                fail_at: 1,
            });

        let operation = InstallationOperation::UnmountFilesystems {
            mounts: default_installation_mounts(),
        };

        let error = executor
            .execute_operation(&operation)
            .expect_err("filesystem unmount should report command failure");

        assert_eq!(error.to_string(), "command failed");

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
    fn installation_plan_cleans_up_after_target_runtime_preparation_failure() {
        let mut executor =
            SystemInstallationOperationExecutor::with_dependencies(FailingAtCommandRunner {
                commands: Vec::new(),
                fail_at: 5,
            });

        let plan = InstallationPlan::new(vec![
            InstallationOperation::PrepareTargetRuntime {
                root: "/target".into(),
            },
            InstallationOperation::CleanupTargetRuntime {
                root: "/target".into(),
            },
        ]);

        let error = plan
            .execute_with_cleanup(&mut executor)
            .expect_err("target runtime preparation failure should be reported");

        assert_eq!(error.to_string(), "command failed");

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
                    "--bind".to_owned(),
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
    fn system_executor_continues_target_runtime_cleanup_after_command_failure() {
        let mut executor =
            SystemInstallationOperationExecutor::with_dependencies(FailingAtCommandRunner {
                commands: Vec::new(),
                fail_at: 1,
            });

        let operation = InstallationOperation::CleanupTargetRuntime {
            root: "/target".into(),
        };

        let error = executor
            .execute_operation(&operation)
            .expect_err("target runtime cleanup should report command failure");

        assert_eq!(error.to_string(), "command failed");

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
                    "--bind".to_owned(),
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
                    "--bind".to_owned(),
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
                    "--bind".to_owned(),
                    "/run".to_owned(),
                    "/target/run".to_owned(),
                ],
            ]
        );
    }

    #[test]
    fn installation_plan_cleans_up_after_bootloader_failure() {
        let mut executor =
            SystemInstallationOperationExecutor::with_dependencies(FailingAtCommandRunner {
                commands: Vec::new(),
                fail_at: 1,
            });

        let plan = InstallationPlan::new(vec![
            InstallationOperation::InstallBootloader {
                root: "/target".into(),
                device_path: "/dev/sdb".into(),
            },
            InstallationOperation::CleanupTargetRuntime {
                root: "/target".into(),
            },
            InstallationOperation::UnmountFilesystems {
                mounts: default_installation_mounts(),
            },
        ]);

        let error = plan
            .execute_with_cleanup(&mut executor)
            .expect_err("bootloader failure should be reported");

        assert_eq!(error.to_string(), "command failed");

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
                    "--removable".to_owned(),
                ],
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
                vec!["umount".to_owned(), "/target/boot/efi".to_owned()],
                vec!["umount".to_owned(), "/target".to_owned()],
            ]
        );
    }

    #[test]
    fn system_executor_stops_bootloader_installation_when_grub_install_fails() {
        let mut executor =
            SystemInstallationOperationExecutor::with_dependencies(FailingAtCommandRunner {
                commands: Vec::new(),
                fail_at: 1,
            });

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
                "--removable".to_owned(),
            ]]
        );
    }

    #[test]
    fn system_executor_returns_update_grub_failure() {
        let mut executor =
            SystemInstallationOperationExecutor::with_dependencies(FailingAtCommandRunner {
                commands: Vec::new(),
                fail_at: 2,
            });

        let operation = InstallationOperation::InstallBootloader {
            root: "/target".into(),
            device_path: "/dev/sdb".into(),
        };

        let error = executor
            .execute_operation(&operation)
            .expect_err("update-grub failure should fail bootloader installation");

        assert_eq!(error.to_string(), "command failed");

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
                    "--removable".to_owned(),
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
    fn system_executor_runs_grub_install_in_target_root() {
        let mut executor = SystemInstallationOperationExecutor::with_dependencies(
            RecordingCommandRunner::default(),
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
                    "--removable".to_owned(),
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
    fn finds_existing_installation_command() {
        assert!(installation_command_exists("sh"));
    }

    #[test]
    fn rejects_missing_installation_command() {
        assert!(!installation_command_exists(
            "daia-command-that-should-not-exist"
        ));
    }

    #[test]
    fn rejects_non_executable_installation_command() {
        use std::os::unix::fs::PermissionsExt;

        let directory = std::env::temp_dir().join(format!(
            "daia-installation-command-test-{}",
            std::process::id()
        ));

        if directory.exists() {
            std::fs::remove_dir_all(&directory).expect("existing test directory should be removed");
        }

        std::fs::create_dir(&directory).expect("test directory should be created");

        let command = directory.join("not-executable");

        std::fs::write(&command, "#!/bin/sh\nexit 0\n").expect("test command should be written");

        let mut permissions = std::fs::metadata(&command)
            .expect("test command metadata should be available")
            .permissions();

        permissions.set_mode(0o644);

        std::fs::set_permissions(&command, permissions)
            .expect("test command permissions should be set");

        assert!(!installation_command_exists_in_path(
            "not-executable",
            directory.as_os_str()
        ));

        std::fs::remove_dir_all(&directory).expect("test directory should be removed");
    }

    #[test]
    fn required_installation_commands_include_executor_dependencies() {
        assert_eq!(
            REQUIRED_INSTALLATION_COMMANDS,
            &[
                "wipefs",
                "parted",
                "mkfs.fat",
                "mkfs.ext4",
                "mkdir",
                "mount",
                "umount",
                "blkid",
                "sudo",
                "unsquashfs",
                "systemctl",
            ]
        );
    }

    #[test]
    fn installation_chroot_path_is_executable() {
        assert!(installation_command_path_is_executable(
            std::path::Path::new("/usr/sbin/chroot")
        ));
    }

    #[test]
    fn validates_available_installation_commands() {
        if REQUIRED_INSTALLATION_COMMANDS
            .iter()
            .all(|command| installation_command_exists(command))
        {
            validate_installation_commands()
                .expect("available installation commands should validate");
        }
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
    fn installation_plan_unmounts_filesystems_after_fstab_failure() {
        let mut executor = SystemInstallationOperationExecutor::with_all_dependencies(
            RecordingCommandRunner::default(),
            RecordingInstallationFileWriter::default(),
        );

        let plan = InstallationPlan::new(vec![
            InstallationOperation::ConfigureFstab {
                device_path: "/dev/sdb".into(),
                partitions: vec![InstallationPartition::new(
                    InstallationPartitionRole::EfiSystem,
                    "fat32",
                    Some(512),
                )],
                mounts: default_installation_mounts(),
            },
            InstallationOperation::UnmountFilesystems {
                mounts: default_installation_mounts(),
            },
        ]);

        let error = plan
            .execute_with_cleanup(&mut executor)
            .expect_err("fstab failure should be reported");

        assert_eq!(error.to_string(), "root partition is missing");

        assert_eq!(
            executor.runner.commands,
            vec![
                vec!["umount".to_owned(), "/target/boot/efi".to_owned()],
                vec!["umount".to_owned(), "/target".to_owned()],
            ]
        );

        assert!(executor.file_writer.writes.is_empty());
    }

    #[test]
    fn system_executor_does_not_write_fstab_for_missing_root_partition() {
        let mut executor = SystemInstallationOperationExecutor::with_all_dependencies(
            RecordingCommandRunner::default(),
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
    fn installation_plan_carries_system_image() {
        let image = PathBuf::from("/run/live/medium/live/filesystem.squashfs");

        let intent = InstallationIntent::new(
            "desktop",
            DiscoveredStorageId::new("serial:usb-disk"),
            model::UserConfiguration::new("admin", "DAIA Administrator"),
        );

        let storage = DiscoveredStorage::new("serial:usb-disk", StorageKind::Removable, "/dev/sdb");

        let prepared = PreparedInstallation::new(intent, storage, Vec::new(), image.clone());

        let plan = prepared.installation_plan();

        let image_operation = plan
            .operations()
            .iter()
            .find_map(|operation| match operation {
                InstallationOperation::InstallSystemImage { root, image } => Some((root, image)),
                _ => None,
            })
            .expect("installation plan should contain system image operation");

        assert_eq!(image_operation.0, std::path::Path::new("/target"));
        assert_eq!(image_operation.1, &image);
    }

    #[test]
    fn system_executor_rejects_missing_efi_partition_before_mounting() {
        let mut executor = SystemInstallationOperationExecutor::with_dependencies(
            RecordingCommandRunner::default(),
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
    fn system_executor_returns_efi_filesystem_command_failure() {
        let mut executor =
            SystemInstallationOperationExecutor::with_dependencies(FailingCommandRunner);

        let operation = InstallationOperation::CreateFilesystems {
            device_path: "/dev/sdb".into(),
            partitions: default_installation_partitions(),
        };

        let error = executor
            .execute_operation(&operation)
            .expect_err("EFI filesystem creation should return command failure");

        assert_eq!(error.kind(), io::ErrorKind::Other);
    }

    #[test]
    fn system_executor_returns_root_filesystem_command_failure() {
        let mut executor =
            SystemInstallationOperationExecutor::with_dependencies(FailingAtCommandRunner {
                commands: Vec::new(),
                fail_at: 2,
            });

        let operation = InstallationOperation::CreateFilesystems {
            device_path: "/dev/sdb".into(),
            partitions: default_installation_partitions(),
        };

        let error = executor
            .execute_operation(&operation)
            .expect_err("root filesystem creation should return command failure");

        assert_eq!(error.to_string(), "command failed");

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
    fn system_executor_creates_efi_and_root_filesystems() {
        let mut executor = SystemInstallationOperationExecutor::with_dependencies(
            RecordingCommandRunner::default(),
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
        );
        assert!(executor.runner.commands.is_empty());
    }
    #[test]
    fn system_executor_sends_wipefs_command_for_prepare_disk() {
        let mut executor = SystemInstallationOperationExecutor::with_dependencies(
            RecordingCommandRunner::default(),
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
        let mut executor =
            SystemInstallationOperationExecutor::with_dependencies(FailingCommandRunner);
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
    fn system_executor_returns_partition_disk_command_failure() {
        let mut executor =
            SystemInstallationOperationExecutor::with_dependencies(FailingCommandRunner);

        let operation = InstallationOperation::PartitionDisk {
            device_path: "/dev/sdb".into(),
            partitions: default_installation_partitions(),
        };

        let error = executor
            .execute_operation(&operation)
            .expect_err("partition disk should return command failure");

        assert_eq!(error.kind(), io::ErrorKind::Other);
    }

    #[test]
    fn system_executor_sends_parted_command_for_partition_disk() {
        let mut executor = SystemInstallationOperationExecutor::with_dependencies(
            RecordingCommandRunner::default(),
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
                "set".to_owned(),
                "1".to_owned(),
                "esp".to_owned(),
                "on".to_owned(),
            ]]
        );
    }
    #[test]
    fn root_partition_start_follows_efi_partition_size() {
        let mut executor = SystemInstallationOperationExecutor::with_dependencies(
            RecordingCommandRunner::default(),
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
                "set".to_owned(),
                "1".to_owned(),
                "esp".to_owned(),
                "on".to_owned(),
            ]]
        );
    }
}
