//! Read-only mounting for external DAIA content volumes.

use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use model::DiscoveredContentVolume;

use crate::StorageInspectError;

/// Mounts external-content volumes for inspection.
///
/// Implementations must not make external content writable.
pub trait ContentVolumeMounter {
    /// Mounts the supplied volume read-only and returns its mountpoint.
    ///
    /// # Errors
    ///
    /// Returns an error when the mountpoint cannot be prepared or the
    /// volume cannot be mounted read-only.
    fn mount_read_only(
        &self,
        volume: &DiscoveredContentVolume,
    ) -> Result<PathBuf, StorageInspectError>;

    /// Unmounts a DAIA-owned external-content mountpoint.
    ///
    /// # Errors
    ///
    /// Returns an error when the volume cannot be unmounted.
    fn unmount(&self, mountpoint: &Path) -> Result<(), StorageInspectError>;
}

/// Linux external-content mounter backed by `mount`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LinuxContentVolumeMounter {
    command: PathBuf,
    umount_command: PathBuf,
    lsblk_command: PathBuf,
    mount_root: PathBuf,
}

impl Default for LinuxContentVolumeMounter {
    fn default() -> Self {
        Self::new()
    }
}

impl LinuxContentVolumeMounter {
    /// Creates a mounter using the DAIA-owned content mount root.
    #[must_use]
    pub fn new() -> Self {
        Self {
            command: PathBuf::from("mount"),
            umount_command: PathBuf::from("umount"),
            lsblk_command: PathBuf::from("lsblk"),
            mount_root: PathBuf::from("/run/daia/content"),
        }
    }

    /// Uses a custom `mount` executable.
    #[must_use]
    pub fn with_command(mut self, command: impl Into<PathBuf>) -> Self {
        self.command = command.into();
        self
    }

    /// Uses a custom `umount` executable.
    #[must_use]
    pub fn with_umount_command(mut self, command: impl Into<PathBuf>) -> Self {
        self.umount_command = command.into();
        self
    }

    /// Uses a custom `lsblk` executable for mount-time inspection.
    #[must_use]
    pub fn with_lsblk_command(mut self, command: impl Into<PathBuf>) -> Self {
        self.lsblk_command = command.into();
        self
    }

    /// Uses a custom DAIA-owned mount root.
    #[must_use]
    pub fn with_mount_root(mut self, mount_root: impl Into<PathBuf>) -> Self {
        self.mount_root = mount_root.into();
        self
    }

    // Obtains a fresh storage snapshot for mount-time safety validation.
    // The caller must validate the selected device against this snapshot.
    fn inspect_mount_topology(
        &self,
    ) -> Result<Vec<crate::lsblk::LsblkDevice>, StorageInspectError> {
        let output = Command::new(&self.lsblk_command)
            .arg("--json")
            .arg("--paths")
            .arg("--bytes")
            .arg("--output")
            .arg("PATH,TYPE,RM,WWN,SERIAL,SIZE,FSTYPE,LABEL,MOUNTPOINT,PARTTYPE,UUID")
            .output()?;

        if !output.status.success() {
            return Err(StorageInspectError::ProcessFailed {
                command: self.lsblk_command.display().to_string(),
                status: output.status,
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            });
        }

        let parsed: crate::lsblk::LsblkOutput = serde_json::from_slice(&output.stdout)
            .map_err(|error| StorageInspectError::InvalidOutput(error.to_string()))?;

        Ok(parsed.blockdevices)
    }

    /// Returns the configured mount executable.
    #[must_use]
    pub fn command(&self) -> &Path {
        &self.command
    }

    /// Returns the configured `umount` executable.
    #[must_use]
    pub fn umount_command(&self) -> &Path {
        &self.umount_command
    }

    /// Returns the DAIA-owned mount root.
    #[must_use]
    pub fn mount_root(&self) -> &Path {
        &self.mount_root
    }

    fn mountpoint_for(
        &self,
        volume: &DiscoveredContentVolume,
    ) -> Result<PathBuf, StorageInspectError> {
        let name = volume
            .device_path()
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| !name.is_empty())
            .ok_or_else(|| {
                StorageInspectError::InvalidOutput(format!(
                    "content volume has no usable device name: {}",
                    volume.device_path().display()
                ))
            })?;

        Ok(self.mount_root.join(name))
    }
}

// Resolves filesystem aliases before comparing device paths.
// This alone is not sufficient for block-device mount authorization.
fn same_resolved_device_path(left: &Path, right: &Path) -> Result<bool, StorageInspectError> {
    let left = fs::canonicalize(left)?;
    let right = fs::canonicalize(right)?;
    Ok(left == right)
}

// Compares actual Linux block-device identities.
// Unlike path-only comparison, this requires block-device special files.
fn same_block_device(left: &Path, right: &Path) -> Result<bool, StorageInspectError> {
    use std::os::unix::fs::{FileTypeExt, MetadataExt};

    let left = fs::canonicalize(left)?;
    let right = fs::canonicalize(right)?;

    let left_metadata = fs::metadata(&left)?;
    let right_metadata = fs::metadata(&right)?;

    if !left_metadata.file_type().is_block_device() || !right_metadata.file_type().is_block_device()
    {
        return Err(StorageInspectError::InvalidOutput(
            "device identity requires block devices".to_owned(),
        ));
    }

    Ok(left_metadata.rdev() == right_metadata.rdev())
}

// Checks whether a selected block-device path appears in the active swap table.
// This does not resolve device aliases or stacked-device ancestry.
fn validate_content_volume_swap(selected: &Path, swaps: &str) -> Result<(), StorageInspectError> {
    let mut lines = swaps.lines();

    let header = lines.next().ok_or_else(|| {
        StorageInspectError::InvalidOutput("active swap table is empty".to_owned())
    })?;

    if header.split_whitespace().collect::<Vec<_>>()
        != ["Filename", "Type", "Size", "Used", "Priority"]
    {
        return Err(StorageInspectError::InvalidOutput(
            "active swap table has an unexpected header".to_owned(),
        ));
    }

    for line in lines {
        let fields: Vec<_> = line.split_whitespace().collect();

        if fields.len() != 5 {
            return Err(StorageInspectError::InvalidOutput(
                "active swap table contains an invalid entry".to_owned(),
            ));
        }

        if Path::new(fields[0]) == selected {
            return Err(StorageInspectError::InvalidOutput(
                "content partition is an active swap device".to_owned(),
            ));
        }
    }

    Ok(())
}

// Validates active swap using actual block-device identities.
// Swap files are not compared as block devices.
// This function does not authorize mounting.
fn validate_content_volume_swap_identity(
    selected: &Path,
    swaps: &str,
) -> Result<(), StorageInspectError> {
    use std::os::unix::fs::FileTypeExt;

    let selected_metadata = fs::metadata(selected)?;

    if !selected_metadata.file_type().is_block_device() {
        return Err(StorageInspectError::InvalidOutput(
            "selected content path is not a block device".to_owned(),
        ));
    }

    let mut lines = swaps.lines();

    let header = lines.next().ok_or_else(|| {
        StorageInspectError::InvalidOutput("active swap table is empty".to_owned())
    })?;

    if header.split_whitespace().collect::<Vec<_>>()
        != ["Filename", "Type", "Size", "Used", "Priority"]
    {
        return Err(StorageInspectError::InvalidOutput(
            "active swap table has an unexpected header".to_owned(),
        ));
    }

    for line in lines {
        let fields: Vec<_> = line.split_whitespace().collect();

        if fields.len() != 5 {
            return Err(StorageInspectError::InvalidOutput(
                "active swap table contains an invalid entry".to_owned(),
            ));
        }

        if fields[1] != "partition" {
            continue;
        }

        if same_block_device(selected, Path::new(fields[0]))? {
            return Err(StorageInspectError::InvalidOutput(
                "content partition is an active swap device".to_owned(),
            ));
        }
    }

    Ok(())
}

// Reads the kernel's active swap table and rejects active swap devices.
// This is an additional safety check, not mount authorization.
fn validate_content_volume_active_swap(selected: &Path) -> Result<(), StorageInspectError> {
    validate_content_volume_active_swap_from(selected, Path::new("/proc/swaps"))
}

fn validate_content_volume_active_swap_from(
    selected: &Path,
    swaps_path: &Path,
) -> Result<(), StorageInspectError> {
    let swaps = fs::read_to_string(swaps_path)?;
    validate_content_volume_swap(selected, &swaps)
}

// Combines partition metadata and active-swap checks.
// Passing this check does not authorize mounting.
fn validate_content_volume_eligibility(
    volume: &DiscoveredContentVolume,
    devices: &[crate::lsblk::LsblkDevice],
    swaps: &str,
) -> Result<(), StorageInspectError> {
    validate_content_volume_metadata(volume, devices)?;
    validate_content_volume_swap_identity(volume.device_path(), swaps)
}

// Checks the selected partition's subtree against active storage use.
// This is a topology check only; it does not authorize mounting.
fn validate_content_volume_protected_ancestry(
    selected: &Path,
    devices: &[crate::lsblk::LsblkDevice],
) -> Result<(), StorageInspectError> {
    use std::collections::{HashMap, HashSet};

    use crate::lsblk::LsblkDevice;

    #[derive(Default)]
    struct DeviceState {
        occurrences: usize,
        device_types: HashSet<String>,
        active: bool,
        children: HashSet<String>,
    }

    fn collect(device: &LsblkDevice, graph: &mut HashMap<String, DeviceState>) {
        let state = graph.entry(device.path.clone()).or_default();

        state.occurrences += 1;
        state.device_types.insert(device.device_type.clone());
        state.active |=
            device.mountpoint.is_some() || device.filesystem_type.as_deref() == Some("swap");

        for child in &device.children {
            graph
                .entry(device.path.clone())
                .or_default()
                .children
                .insert(child.path.clone());

            collect(child, graph);
        }
    }

    let mut graph = HashMap::new();

    for device in devices {
        collect(device, &mut graph);
    }

    let selected_path = selected.to_string_lossy();

    let Some(selected_state) = graph.get(selected_path.as_ref()) else {
        return Err(StorageInspectError::InvalidOutput(
            "selected content partition has missing topology".to_owned(),
        ));
    };

    if selected_state.occurrences != 1
        || selected_state.device_types.len() != 1
        || !selected_state.device_types.contains("part")
    {
        return Err(StorageInspectError::InvalidOutput(
            "selected content partition has missing or ambiguous topology".to_owned(),
        ));
    }

    let mut visited = HashSet::new();
    let mut pending = vec![selected_path.into_owned()];

    while let Some(path) = pending.pop() {
        if !visited.insert(path.clone()) {
            continue;
        }

        let Some(state) = graph.get(&path) else {
            return Err(StorageInspectError::InvalidOutput(
                "content volume topology contains an unresolved device".to_owned(),
            ));
        };

        if state.active || state.device_types.len() != 1 {
            return Err(StorageInspectError::InvalidOutput(
                "selected content partition belongs to active or ambiguous storage".to_owned(),
            ));
        }

        pending.extend(state.children.iter().cloned());
    }

    Ok(())
}

// Metadata eligibility is not sufficient to authorize mounting.
// Active system-device protection must also be verified at mount time.
fn validate_content_volume_metadata(
    volume: &DiscoveredContentVolume,
    devices: &[crate::lsblk::LsblkDevice],
) -> Result<(), StorageInspectError> {
    use crate::lsblk::LsblkDevice;

    fn find_partition<'a>(
        devices: &'a [LsblkDevice],
        selected: &Path,
    ) -> Option<(&'a LsblkDevice, &'a LsblkDevice)> {
        let mut matched = None;

        for disk in devices.iter().filter(|device| device.device_type == "disk") {
            for child in &disk.children {
                if child.device_type == "part" && Path::new(&child.path) == selected {
                    if matched.is_some() {
                        return None;
                    }

                    matched = Some((disk, child));
                }
            }
        }

        matched
    }

    let (disk, partition) = find_partition(devices, volume.device_path()).ok_or_else(|| {
        StorageInspectError::InvalidOutput("selected content partition was not found".to_owned())
    })?;

    if Path::new(&disk.path) != volume.parent_device_path() {
        return Err(StorageInspectError::InvalidOutput(
            "content partition parent disk does not match".to_owned(),
        ));
    }

    fn has_active_descendant(device: &crate::lsblk::LsblkDevice) -> bool {
        device.children.iter().any(|child| {
            child.mountpoint.is_some()
                || child.filesystem_type.as_deref() == Some("swap")
                || has_active_descendant(child)
        })
    }

    if has_active_descendant(partition) {
        return Err(StorageInspectError::InvalidOutput(
            "content partition has an active descendant device".to_owned(),
        ));
    }

    if partition.mountpoint.is_some() || volume.mountpoint().is_some() {
        return Err(StorageInspectError::InvalidOutput(
            "content partition is already mounted".to_owned(),
        ));
    }

    if partition.filesystem_type.as_deref() != Some(volume.filesystem_type()) {
        return Err(StorageInspectError::InvalidOutput(
            "content partition filesystem does not match".to_owned(),
        ));
    }

    let filesystem = volume.filesystem_type();

    if !matches!(
        filesystem,
        "ext4" | "ext3" | "xfs" | "btrfs" | "vfat" | "exfat" | "ntfs"
    ) {
        return Err(StorageInspectError::InvalidOutput(
            "unsupported content filesystem".to_owned(),
        ));
    }

    if partition
        .filesystem_uuid
        .as_deref()
        .is_none_or(str::is_empty)
    {
        return Err(StorageInspectError::InvalidOutput(
            "content partition filesystem UUID is missing".to_owned(),
        ));
    }

    let partition_type = partition
        .partition_type
        .as_deref()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            StorageInspectError::InvalidOutput("content partition type is missing".to_owned())
        })?;

    let partition_type = partition_type.to_ascii_lowercase();

    const PROTECTED_TYPES: &[&str] = &[
        // EFI System Partition
        "c12a7328-f81f-11d2-ba4b-00a0c93ec93b",
        // BIOS boot partition
        "21686148-6449-6e6f-744e-656564454649",
        // Microsoft recovery partition
        "de94bba4-06d1-4d40-a16a-bfd50179d6ac",
        // Windows recovery environment / utility partition
        "a19d880f-05fc-4d3b-a006-743f0f84911e",
    ];

    if PROTECTED_TYPES.contains(&partition_type.as_str()) {
        return Err(StorageInspectError::InvalidOutput(
            "protected partition type cannot supply external content".to_owned(),
        ));
    }

    Ok(())
}

// Validates current partition metadata and protected storage ancestry.
// Active-swap identity checks and production wiring remain separate.
fn authorize_content_volume_mount(
    volume: &DiscoveredContentVolume,
    devices: &[crate::lsblk::LsblkDevice],
) -> Result<(), StorageInspectError> {
    validate_content_volume_metadata(volume, devices)?;
    validate_content_volume_protected_ancestry(volume.device_path(), devices)
}

impl ContentVolumeMounter for LinuxContentVolumeMounter {
    fn mount_read_only(
        &self,
        volume: &DiscoveredContentVolume,
    ) -> Result<PathBuf, StorageInspectError> {
        if let Some(mountpoint) = volume.mountpoint() {
            return Err(StorageInspectError::InvalidOutput(format!(
                "content volume {} is already mounted at {}; DAIA requires an unmounted volume for read-only inspection",
                volume.device_path().display(),
                mountpoint.display()
            )));
        }

        use std::os::unix::fs::FileTypeExt;

        let metadata = fs::metadata(volume.device_path())?;

        if !metadata.file_type().is_block_device() {
            return Err(StorageInspectError::InvalidOutput(format!(
                "content volume {} is not a block device",
                volume.device_path().display()
            )));
        }

        // Require fresh storage inspection before invoking mount.
        // Full topology authorization will be connected separately.
        let _fresh_topology = self.inspect_mount_topology()?;

        let mountpoint = self.mountpoint_for(volume)?;

        fs::create_dir_all(&mountpoint)?;

        let output = Command::new(&self.command)
            .arg("--read-only")
            .arg("--types")
            .arg(volume.filesystem_type())
            .arg("--")
            .arg(volume.device_path())
            .arg(&mountpoint)
            .output()?;

        if !output.status.success() {
            return Err(StorageInspectError::ProcessFailed {
                command: self.command.display().to_string(),
                status: output.status,
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            });
        }

        Ok(mountpoint)
    }

    fn unmount(&self, mountpoint: &Path) -> Result<(), StorageInspectError> {
        if !mountpoint.starts_with(&self.mount_root) || mountpoint == self.mount_root {
            return Err(StorageInspectError::InvalidOutput(format!(
                "refusing to unmount non-DAIA content mountpoint: {}",
                mountpoint.display()
            )));
        }

        let output = Command::new(&self.umount_command)
            .arg("--")
            .arg(mountpoint)
            .output()?;

        if !output.status.success() {
            return Err(StorageInspectError::ProcessFailed {
                command: self.umount_command.display().to_string(),
                status: output.status,
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            });
        }

        match fs::remove_dir(mountpoint) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs::{self, File},
        io::Write,
        os::unix::fs::PermissionsExt,
        thread,
        time::Duration,
    };

    use model::DiscoveredContentVolume;
    use tempfile::TempDir;

    use super::{ContentVolumeMounter, LinuxContentVolumeMounter};

    fn command_script(contents: &str) -> (TempDir, std::path::PathBuf) {
        let directory = TempDir::new().expect("temporary directory should be created");
        let command = directory.path().join("mount");
        let temporary_command = directory.path().join("mount.tmp");

        let mut file =
            File::create(&temporary_command).expect("temporary mount command should be created");

        file.write_all(contents.as_bytes())
            .expect("temporary mount command should be written");

        file.sync_all()
            .expect("temporary mount command should be synchronized");

        drop(file);

        let mut permissions = fs::metadata(&temporary_command)
            .expect("temporary mount command metadata should be readable")
            .permissions();

        permissions.set_mode(0o755);

        fs::set_permissions(&temporary_command, permissions)
            .expect("temporary mount command should be executable");

        fs::rename(&temporary_command, &command)
            .expect("temporary mount command should be installed");

        thread::sleep(Duration::from_millis(10));

        (directory, command)
    }

    #[test]
    fn mount_rejects_missing_block_device_before_command() {
        let directory = TempDir::new().expect("temporary directory should be created");

        let marker = directory.path().join("mount-invoked");
        let command = directory.path().join("mock-mount");

        let script = format!("#!/bin/sh\ntouch '{}'\nexit 0\n", marker.display());

        fs::write(&command, script).expect("mock mount command should be written");

        let mut permissions = fs::metadata(&command)
            .expect("mock mount metadata should be readable")
            .permissions();

        permissions.set_mode(0o755);

        fs::set_permissions(&command, permissions).expect("mock mount should be executable");

        let mounter = LinuxContentVolumeMounter::new()
            .with_command(&command)
            .with_mount_root(directory.path().join("content"));

        let volume = DiscoveredContentVolume::new(
            directory.path().join("missing-block-device"),
            "/dev/sdb",
            "ext4",
        );

        let result = mounter.mount_read_only(&volume);

        assert!(result.is_err(), "missing block device must be rejected");

        assert!(
            !marker.exists(),
            "mount command must not run for an invalid block device"
        );
    }

    #[test]
    fn mount_topology_reader_rejects_invalid_json() {
        let (_directory, command) = command_script("#!/bin/sh\nprintf 'invalid-json\\n'\n");

        let mounter = LinuxContentVolumeMounter::new().with_lsblk_command(command);

        let error = mounter
            .inspect_mount_topology()
            .expect_err("invalid topology JSON must be rejected");

        assert!(
            matches!(error, crate::StorageInspectError::InvalidOutput(_)),
            "unexpected topology error: {error}"
        );
    }

    #[test]
    fn mount_rejects_failed_fresh_topology_before_command() {
        use std::os::unix::fs::PermissionsExt;

        let directory = tempfile::tempdir().expect("temporary directory");
        let marker = directory.path().join("mount-invoked");

        let mount_command = directory.path().join("mount-command");
        std::fs::write(
            &mount_command,
            format!("#!/bin/sh\nprintf invoked > '{}'\n", marker.display()),
        )
        .expect("write mount command");

        let mut permissions = std::fs::metadata(&mount_command)
            .expect("mount command metadata")
            .permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(&mount_command, permissions)
            .expect("make mount command executable");

        let (_lsblk_directory, lsblk_command) = command_script("#!/bin/sh\nexit 1\n");

        let volume = model::DiscoveredContentVolume::new("/dev/sdb1", "/dev/sdb", "ext4");

        let mounter = LinuxContentVolumeMounter::new()
            .with_command(mount_command)
            .with_lsblk_command(lsblk_command)
            .with_mount_root(directory.path().join("mounts"));

        let result = mounter.mount_read_only(&volume);

        assert!(
            result.is_err(),
            "failed fresh topology inspection must prevent mounting"
        );
        assert!(
            !marker.exists(),
            "mount command must not execute after topology failure"
        );
    }

    fn metadata_fixture(
        partition_type: Option<&str>,
        filesystem_uuid: Option<&str>,
        filesystem_type: &str,
    ) -> Vec<crate::lsblk::LsblkDevice> {
        use crate::lsblk::LsblkDevice;

        let partition = LsblkDevice {
            path: "/dev/sdb1".to_owned(),
            device_type: "part".to_owned(),
            rm: true,
            wwn: None,
            serial: None,
            size: 500_000,
            filesystem_type: Some(filesystem_type.to_owned()),
            label: None,
            mountpoint: None,
            partition_type: partition_type.map(str::to_owned),
            filesystem_uuid: filesystem_uuid.map(str::to_owned),
            children: Vec::new(),
        };

        vec![LsblkDevice {
            path: "/dev/sdb".to_owned(),
            device_type: "disk".to_owned(),
            rm: true,
            wwn: None,
            serial: None,
            size: 1_000_000,
            filesystem_type: None,
            label: None,
            mountpoint: None,
            partition_type: None,
            filesystem_uuid: None,
            children: vec![partition],
        }]
    }

    const LINUX_DATA_PARTITION: &str = "0fc63daf-8483-4772-8e79-3d69d8477de4";

    #[test]
    fn mount_authorization_rejects_active_descendant() {
        let mut devices = metadata_fixture(Some(LINUX_DATA_PARTITION), Some("data-uuid"), "ext4");

        let mut descendant = devices[0].children[0].clone();
        descendant.path = "/dev/mapper/active-root".to_owned();
        descendant.device_type = "lvm".to_owned();
        descendant.mountpoint = Some("/".to_owned());
        descendant.children.clear();

        devices[0].children[0].children.push(descendant);

        let volume = model::DiscoveredContentVolume::new("/dev/sdb1", "/dev/sdb", "ext4");

        let result = super::authorize_content_volume_mount(&volume, &devices);

        assert!(
            result.is_err(),
            "active descendant must prevent mount authorization"
        );
    }

    #[test]
    fn topology_accepts_unmounted_data_partition() {
        let devices = metadata_fixture(Some(LINUX_DATA_PARTITION), Some("data-uuid"), "ext4");

        super::validate_content_volume_protected_ancestry(
            std::path::Path::new("/dev/sdb1"),
            &devices,
        )
        .expect("unmounted data partition should pass topology validation");
    }

    #[test]
    fn topology_accepts_data_partition_beside_mounted_root() {
        let mut devices = metadata_fixture(Some(LINUX_DATA_PARTITION), Some("data-uuid"), "ext4");

        let mut root = devices[0].children[0].clone();
        root.path = "/dev/sdb2".to_owned();
        root.mountpoint = Some("/".to_owned());

        devices[0].children.push(root);

        super::validate_content_volume_protected_ancestry(
            std::path::Path::new("/dev/sdb1"),
            &devices,
        )
        .expect("mounted sibling must not disqualify independent data partition");
    }

    #[test]
    fn topology_rejects_mounted_descendant() {
        let mut devices = metadata_fixture(Some(LINUX_DATA_PARTITION), Some("data-uuid"), "ext4");

        let mut descendant = devices[0].children[0].clone();
        descendant.path = "/dev/mapper/active-root".to_owned();
        descendant.device_type = "lvm".to_owned();
        descendant.mountpoint = Some("/".to_owned());
        descendant.children.clear();

        devices[0].children[0].children.push(descendant);

        assert!(
            super::validate_content_volume_protected_ancestry(
                std::path::Path::new("/dev/sdb1"),
                &devices,
            )
            .is_err()
        );
    }

    #[test]
    fn topology_rejects_swap_descendant() {
        let mut devices = metadata_fixture(Some(LINUX_DATA_PARTITION), Some("data-uuid"), "ext4");

        let mut descendant = devices[0].children[0].clone();
        descendant.path = "/dev/mapper/active-swap".to_owned();
        descendant.device_type = "crypt".to_owned();
        descendant.filesystem_type = Some("swap".to_owned());
        descendant.children.clear();

        devices[0].children[0].children.push(descendant);

        assert!(
            super::validate_content_volume_protected_ancestry(
                std::path::Path::new("/dev/sdb1"),
                &devices,
            )
            .is_err()
        );
    }

    #[test]
    fn topology_rejects_missing_and_duplicate_selection() {
        let mut devices = metadata_fixture(Some(LINUX_DATA_PARTITION), Some("data-uuid"), "ext4");

        assert!(
            super::validate_content_volume_protected_ancestry(
                std::path::Path::new("/dev/missing"),
                &devices,
            )
            .is_err()
        );

        devices.push(devices[0].clone());

        assert!(
            super::validate_content_volume_protected_ancestry(
                std::path::Path::new("/dev/sdb1"),
                &devices,
            )
            .is_err()
        );
    }

    #[test]
    fn topology_rejects_active_shared_raid_descendant() {
        let mut devices = metadata_fixture(Some(LINUX_DATA_PARTITION), Some("data-uuid"), "ext4");

        // First RAID appearance has no visible active descendants.
        let mut raid = devices[0].children[0].clone();
        raid.path = "/dev/md0".to_owned();
        raid.device_type = "raid1".to_owned();
        raid.mountpoint = None;
        raid.children.clear();

        devices[0].children[0].children.push(raid.clone());

        // An independent sibling must not affect eligibility.
        let mut sibling = devices[0].children[0].clone();
        sibling.path = "/dev/sdb2".to_owned();
        sibling.children.clear();
        devices[0].children.push(sibling);

        // Second RAID appearance exposes the active root dependency.
        let mut root = raid.clone();
        root.path = "/dev/mapper/root".to_owned();
        root.device_type = "crypt".to_owned();
        root.mountpoint = Some("/".to_owned());
        root.children.clear();

        raid.children.push(root);

        let mut second_disk = devices[0].clone();
        second_disk.path = "/dev/sdc".to_owned();
        second_disk.children.clear();

        let mut second_partition = devices[0].children[0].clone();
        second_partition.path = "/dev/sdc1".to_owned();
        second_partition.children = vec![raid];

        second_disk.children.push(second_partition);
        devices.push(second_disk);

        let result = super::validate_content_volume_protected_ancestry(
            std::path::Path::new("/dev/sdb1"),
            &devices,
        );

        assert!(
            result.is_err(),
            "selected partition must be rejected when a shared RAID descendant backs active root"
        );
    }

    #[test]
    fn metadata_accepts_unmounted_data_partition() {
        let volume = DiscoveredContentVolume::new("/dev/sdb1", "/dev/sdb", "ext4");

        let devices = metadata_fixture(
            Some(LINUX_DATA_PARTITION),
            Some("model-volume-uuid"),
            "ext4",
        );

        super::validate_content_volume_metadata(&volume, &devices)
            .expect("valid data partition should pass metadata validation");
    }

    #[test]
    fn metadata_rejects_efi_partition() {
        let volume = DiscoveredContentVolume::new("/dev/sdb1", "/dev/sdb", "vfat");

        let devices = metadata_fixture(
            Some("c12a7328-f81f-11d2-ba4b-00a0c93ec93b"),
            Some("efi-uuid"),
            "vfat",
        );

        assert!(super::validate_content_volume_metadata(&volume, &devices).is_err());
    }

    #[test]
    fn metadata_rejects_recovery_partition() {
        let volume = DiscoveredContentVolume::new("/dev/sdb1", "/dev/sdb", "ntfs");

        let devices = metadata_fixture(
            Some("de94bba4-06d1-4d40-a16a-bfd50179d6ac"),
            Some("recovery-uuid"),
            "ntfs",
        );

        assert!(super::validate_content_volume_metadata(&volume, &devices).is_err());
    }

    #[test]
    fn metadata_rejects_missing_filesystem_uuid() {
        let volume = DiscoveredContentVolume::new("/dev/sdb1", "/dev/sdb", "ext4");

        let devices = metadata_fixture(Some(LINUX_DATA_PARTITION), None, "ext4");

        assert!(super::validate_content_volume_metadata(&volume, &devices).is_err());
    }

    #[test]
    fn metadata_rejects_missing_partition_type() {
        let volume = DiscoveredContentVolume::new("/dev/sdb1", "/dev/sdb", "ext4");

        let devices = metadata_fixture(None, Some("model-volume-uuid"), "ext4");

        assert!(super::validate_content_volume_metadata(&volume, &devices).is_err());
    }

    #[test]
    fn metadata_rejects_mismatched_parent_disk() {
        let volume = DiscoveredContentVolume::new("/dev/sdb1", "/dev/sdc", "ext4");

        let devices = metadata_fixture(
            Some(LINUX_DATA_PARTITION),
            Some("model-volume-uuid"),
            "ext4",
        );

        assert!(super::validate_content_volume_metadata(&volume, &devices).is_err());
    }

    #[test]
    fn metadata_rejects_unsupported_filesystem() {
        let volume = DiscoveredContentVolume::new("/dev/sdb1", "/dev/sdb", "swap");

        let devices = metadata_fixture(Some(LINUX_DATA_PARTITION), Some("swap-uuid"), "swap");

        assert!(super::validate_content_volume_metadata(&volume, &devices).is_err());
    }

    #[test]
    fn metadata_rejects_duplicate_partition_identity() {
        let volume = DiscoveredContentVolume::new("/dev/sdb1", "/dev/sdb", "ext4");

        let mut devices = metadata_fixture(
            Some(LINUX_DATA_PARTITION),
            Some("model-volume-uuid"),
            "ext4",
        );

        let mut duplicate_devices = metadata_fixture(
            Some(LINUX_DATA_PARTITION),
            Some("model-volume-uuid"),
            "ext4",
        );

        let duplicate = duplicate_devices[0].children.remove(0);
        devices[0].children.push(duplicate);

        let error = super::validate_content_volume_metadata(&volume, &devices)
            .expect_err("duplicate partition identity must be rejected");

        assert!(
            error
                .to_string()
                .contains("selected content partition was not found")
        );
    }

    #[test]
    fn metadata_rejects_partition_with_active_descendant() {
        let volume = DiscoveredContentVolume::new("/dev/sdb1", "/dev/sdb", "ext4");

        let mut devices = metadata_fixture(
            Some(LINUX_DATA_PARTITION),
            Some("model-volume-uuid"),
            "ext4",
        );

        let mut descendant_devices =
            metadata_fixture(Some(LINUX_DATA_PARTITION), Some("descendant-uuid"), "ext4");

        let mut descendant = descendant_devices[0].children.remove(0);
        descendant.device_type = "lvm".to_owned();
        descendant.mountpoint = Some("/".to_owned());

        devices[0].children[0].children.push(descendant);

        let error = super::validate_content_volume_metadata(&volume, &devices)
            .expect_err("active descendant must be rejected");

        assert!(
            error
                .to_string()
                .contains("content partition has an active descendant device")
        );
    }

    #[test]
    fn swap_validation_rejects_unreadable_table() {
        let directory = tempfile::TempDir::new().expect("temporary directory should be created");

        let missing = directory.path().join("missing-swaps");

        let error = super::validate_content_volume_active_swap_from(
            std::path::Path::new("/dev/sdb1"),
            &missing,
        )
        .expect_err("unreadable swap table must be rejected");

        assert!(
            matches!(error, crate::StorageInspectError::Io(_)),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn eligibility_accepts_inactive_data_partition() {
        let volume = DiscoveredContentVolume::new("/dev/sdb1", "/dev/sdb", "ext4");

        let devices = metadata_fixture(
            Some(LINUX_DATA_PARTITION),
            Some("model-volume-uuid"),
            "ext4",
        );

        let swaps = "Filename Type Size Used Priority\n";

        super::validate_content_volume_metadata(&volume, &devices)
            .expect("inactive data partition should pass metadata checks");
        super::validate_content_volume_swap(volume.device_path(), swaps)
            .expect("inactive data partition should pass path-based swap checks");
    }

    #[test]
    fn eligibility_rejects_active_swap_partition() {
        let volume = DiscoveredContentVolume::new("/dev/sdb1", "/dev/sdb", "ext4");

        let devices = metadata_fixture(
            Some(LINUX_DATA_PARTITION),
            Some("model-volume-uuid"),
            "ext4",
        );

        let swaps = "Filename Type Size Used Priority\n/dev/sdb1 partition 1024 0 -2\n";

        super::validate_content_volume_metadata(&volume, &devices)
            .expect("partition metadata should be valid");

        let error = super::validate_content_volume_swap(volume.device_path(), swaps)
            .expect_err("active swap partition must be rejected");

        assert!(
            error.to_string().contains("active swap"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn eligibility_rejects_non_block_device_selection() {
        let directory = tempfile::TempDir::new().expect("temporary directory should be created");

        let selected = directory.path().join("selected");
        fs::write(&selected, b"not a block device")
            .expect("regular-file fixture should be created");

        let mut devices = metadata_fixture(
            Some(LINUX_DATA_PARTITION),
            Some("model-volume-uuid"),
            "ext4",
        );

        devices[0].children[0].path = selected.to_string_lossy().into_owned();

        let volume = DiscoveredContentVolume::new(&selected, "/dev/sdb", "ext4");

        let swaps = "Filename Type Size Used Priority\n";

        let error = super::validate_content_volume_eligibility(&volume, &devices, swaps)
            .expect_err("regular files must not pass mount eligibility");

        assert!(
            error.to_string().contains("not a block device"),
            "unexpected eligibility error: {error}"
        );
    }

    #[test]
    fn swap_identity_rejects_regular_selected_path() {
        let directory = tempfile::TempDir::new().expect("temporary directory should be created");

        let selected = directory.path().join("selected");

        fs::write(&selected, b"fixture").expect("fixture should be created");

        let swaps = "Filename Type Size Used Priority\\n";

        let error = super::validate_content_volume_swap_identity(&selected, swaps)
            .expect_err("regular files must not be selected as content devices");

        assert!(
            error.to_string().contains("not a block device"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn swap_identity_rejects_missing_selected_path() {
        let directory = tempfile::TempDir::new().expect("temporary directory should be created");

        let missing = directory.path().join("missing");
        let swaps = "Filename Type Size Used Priority\\n";

        let error = super::validate_content_volume_swap_identity(&missing, swaps)
            .expect_err("missing selected device must fail");

        assert!(
            matches!(error, crate::StorageInspectError::Io(_)),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn real_block_device_alias_matches_identity() {
        let device = std::path::Path::new("/dev/sda3");
        let alias = std::path::Path::new("/dev/disk/by-uuid/75077d19-d4fb-463a-8249-06d54c93842a");

        if !device.exists() || !alias.exists() {
            return;
        }

        assert!(
            super::same_block_device(device, alias)
                .expect("block-device identity should be readable")
        );
    }

    #[test]
    fn distinct_real_block_devices_have_distinct_identities() {
        let first = std::path::Path::new("/dev/sda2");
        let second = std::path::Path::new("/dev/sda3");

        if !first.exists() || !second.exists() {
            return;
        }

        assert!(
            !super::same_block_device(first, second)
                .expect("block-device identities should be readable")
        );
    }

    #[test]
    fn active_swap_alias_is_rejected_by_identity() {
        let alias = std::path::Path::new("/dev/disk/by-uuid/75077d19-d4fb-463a-8249-06d54c93842a");

        if !alias.exists() {
            return;
        }

        let swaps = match fs::read_to_string("/proc/swaps") {
            Ok(swaps) => swaps,
            Err(_) => return,
        };

        if !swaps
            .lines()
            .skip(1)
            .any(|line| line.split_whitespace().next() == Some("/dev/sda3"))
        {
            return;
        }

        let error = super::validate_content_volume_swap_identity(alias, &swaps)
            .expect_err("active swap alias must be rejected");

        assert!(
            error.to_string().contains("active swap device"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn block_device_identity_rejects_regular_files() {
        let directory = tempfile::TempDir::new().expect("temporary directory should be created");

        let first = directory.path().join("first");
        let second = directory.path().join("second");

        fs::write(&first, b"first").expect("first fixture should be created");
        fs::write(&second, b"second").expect("second fixture should be created");

        let error = super::same_block_device(&first, &second)
            .expect_err("regular files must not pass block-device validation");

        assert!(
            error.to_string().contains("requires block devices"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn block_device_identity_rejects_missing_path() {
        let directory = tempfile::TempDir::new().expect("temporary directory should be created");

        let first = directory.path().join("first");
        let missing = directory.path().join("missing");

        fs::write(&first, b"fixture").expect("fixture should be created");

        let error = super::same_block_device(&first, &missing)
            .expect_err("unresolved device path must be rejected");

        assert!(
            matches!(error, crate::StorageInspectError::Io(_)),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn device_identity_recognizes_symlink_alias() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::TempDir::new().expect("temporary directory should be created");

        let device = directory.path().join("device");
        let alias = directory.path().join("alias");

        fs::write(&device, b"fixture").expect("fixture should be created");
        symlink(&device, &alias).expect("alias should be created");

        assert!(
            super::same_resolved_device_path(&device, &alias).expect("both paths should resolve")
        );
    }

    #[test]
    fn device_identity_rejects_unresolved_path() {
        let directory = tempfile::TempDir::new().expect("temporary directory should be created");

        let device = directory.path().join("device");
        let missing = directory.path().join("missing");

        fs::write(&device, b"fixture").expect("fixture should be created");

        assert!(
            super::same_resolved_device_path(&device, &missing).is_err(),
            "unresolved identity must fail closed"
        );
    }

    #[test]
    fn swap_validation_accepts_inactive_partition() {
        let swaps = "Filename Type Size Used Priority\n/dev/sdc1 partition 1024 0 -2\n";

        super::validate_content_volume_swap(std::path::Path::new("/dev/sdb1"), swaps)
            .expect("inactive partition should be accepted");
    }

    #[test]
    fn swap_validation_rejects_active_partition() {
        let swaps = "Filename Type Size Used Priority\n/dev/sdb1 partition 1024 0 -2\n";

        let error = super::validate_content_volume_swap(std::path::Path::new("/dev/sdb1"), swaps)
            .expect_err("active swap partition must be rejected");

        assert!(error.to_string().contains("active swap device"));
    }

    #[test]
    fn swap_validation_rejects_invalid_table() {
        let error = super::validate_content_volume_swap(
            std::path::Path::new("/dev/sdb1"),
            "invalid header\\n",
        )
        .expect_err("invalid swap table must be rejected");

        assert!(error.to_string().contains("unexpected header"));
    }

    #[test]
    fn metadata_rejects_unknown_partition() {
        let volume = DiscoveredContentVolume::new("/dev/sdc1", "/dev/sdc", "ext4");

        let devices = metadata_fixture(
            Some(LINUX_DATA_PARTITION),
            Some("model-volume-uuid"),
            "ext4",
        );

        assert!(super::validate_content_volume_metadata(&volume, &devices).is_err());
    }

    #[test]
    fn mounts_external_volume_read_only_under_daia_mount_root() {
        let (_directory, command) = command_script(
            r#"#!/bin/sh
printf '%s\n' "$@" > "$DAIA_TEST_MOUNT_ARGS"
"#,
        );

        let mount_root = TempDir::new().expect("temporary mount root should be created");
        let arguments = mount_root.path().join("mount-arguments");

        let wrapper = mount_root.path().join("mount-wrapper");
        fs::copy(&command, &wrapper).expect("mount command should be copied");

        let script = format!(
            "#!/bin/sh\nDAIA_TEST_MOUNT_ARGS='{}' exec '{}' \"$@\"\n",
            arguments.display(),
            command.display(),
        );

        fs::write(&wrapper, script).expect("mount wrapper should be written");

        let mut permissions = fs::metadata(&wrapper)
            .expect("mount wrapper metadata should be readable")
            .permissions();

        permissions.set_mode(0o755);

        fs::set_permissions(&wrapper, permissions).expect("mount wrapper should be executable");

        let volume =
            DiscoveredContentVolume::new("/dev/sdb1", "/dev/sdb", "ext4").with_label("DAIA-MODELS");

        let mounter = LinuxContentVolumeMounter::new()
            .with_command(wrapper)
            .with_mount_root(mount_root.path());

        let mountpoint = mounter
            .mount_read_only(&volume)
            .expect("content volume should mount");

        assert_eq!(mountpoint, mount_root.path().join("sdb1"));
        assert!(mountpoint.is_dir());

        let arguments = fs::read_to_string(arguments).expect("mount arguments should exist");

        assert_eq!(
            arguments.lines().collect::<Vec<_>>(),
            vec![
                "--read-only",
                "--types",
                "ext4",
                "--",
                "/dev/sdb1",
                mountpoint
                    .to_str()
                    .expect("test mountpoint should be UTF-8"),
            ]
        );
    }

    #[test]
    fn rejects_existing_mountpoint() {
        let (_directory, command) = command_script(
            r#"#!/bin/sh
exit 99
"#,
        );

        let volume = DiscoveredContentVolume::new("/dev/sdb1", "/dev/sdb", "ext4")
            .with_mountpoint("/media/existing");

        let mounter = LinuxContentVolumeMounter::new().with_command(command);

        let error = mounter
            .mount_read_only(&volume)
            .expect_err("existing mountpoint should be rejected");

        let message = error.to_string();

        assert!(message.contains("/dev/sdb1"));
        assert!(message.contains("/media/existing"));
        assert!(message.contains("requires an unmounted volume"));
    }

    #[test]
    fn unmounts_daia_owned_mountpoint_and_removes_directory() {
        let (_directory, command) = command_script(
            r#"#!/bin/sh
printf '%s\n' "$@" > "$DAIA_TEST_UMOUNT_ARGS"
"#,
        );

        let mount_root = TempDir::new().expect("temporary mount root should be created");
        let mountpoint = mount_root.path().join("sdb1");

        fs::create_dir(&mountpoint).expect("test mountpoint should be created");

        let arguments = mount_root.path().join("umount-arguments");
        let wrapper = mount_root.path().join("umount-wrapper");

        let script = format!(
            "#!/bin/sh\nDAIA_TEST_UMOUNT_ARGS='{}' exec '{}' \"$@\"\n",
            arguments.display(),
            command.display(),
        );

        fs::write(&wrapper, script).expect("umount wrapper should be written");

        let mut permissions = fs::metadata(&wrapper)
            .expect("umount wrapper metadata should be readable")
            .permissions();

        permissions.set_mode(0o755);

        fs::set_permissions(&wrapper, permissions).expect("umount wrapper should be executable");

        let mounter = LinuxContentVolumeMounter::new()
            .with_umount_command(wrapper)
            .with_mount_root(mount_root.path());

        mounter
            .unmount(&mountpoint)
            .expect("DAIA-owned mountpoint should unmount");

        assert!(!mountpoint.exists());

        let arguments = fs::read_to_string(arguments).expect("umount arguments should exist");

        assert_eq!(
            arguments.lines().collect::<Vec<_>>(),
            vec![
                "--",
                mountpoint
                    .to_str()
                    .expect("test mountpoint should be UTF-8"),
            ]
        );
    }

    #[test]
    fn refuses_to_unmount_mountpoint_outside_daia_mount_root() {
        let (_directory, command) = command_script(
            r#"#!/bin/sh
exit 99
"#,
        );

        let mount_root = TempDir::new().expect("temporary mount root should be created");

        let mounter = LinuxContentVolumeMounter::new()
            .with_umount_command(command)
            .with_mount_root(mount_root.path());

        let error = mounter
            .unmount(std::path::Path::new("/media/existing"))
            .expect_err("non-DAIA mountpoint should be rejected");

        assert!(
            error
                .to_string()
                .contains("refusing to unmount non-DAIA content mountpoint")
        );
    }

    #[test]
    fn reports_failed_unmount() {
        let (_directory, command) = command_script(
            r#"#!/bin/sh
echo 'content unmount failed' >&2
exit 1
"#,
        );

        let mount_root = TempDir::new().expect("temporary mount root should be created");
        let mountpoint = mount_root.path().join("sdb1");

        fs::create_dir(&mountpoint).expect("test mountpoint should be created");

        let mounter = LinuxContentVolumeMounter::new()
            .with_umount_command(command)
            .with_mount_root(mount_root.path());

        let error = mounter
            .unmount(&mountpoint)
            .expect_err("failed unmount should be reported");

        assert!(
            error.to_string().contains("content unmount failed"),
            "unexpected unmount error: {error}"
        );
        assert!(mountpoint.exists());
    }

    #[test]
    fn reports_failed_read_only_mount() {
        let (_directory, command) = command_script(
            r#"#!/bin/sh
echo 'read-only mount failed' >&2
exit 1
"#,
        );

        let mount_root = TempDir::new().expect("temporary mount root should be created");

        let volume = DiscoveredContentVolume::new("/dev/sdb1", "/dev/sdb", "ext4");

        let mounter = LinuxContentVolumeMounter::new()
            .with_command(command)
            .with_mount_root(mount_root.path());

        let error = mounter
            .mount_read_only(&volume)
            .expect_err("failed mount should be reported");

        assert!(
            error.to_string().contains("read-only mount failed"),
            "unexpected mount error: {error}"
        );
    }
}
