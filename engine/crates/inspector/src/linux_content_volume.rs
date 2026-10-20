//! Linux external-content volume discovery backed by `lsblk`.

use std::{
    path::{Path, PathBuf},
    process::Command,
};

use model::DiscoveredContentVolume;

use crate::{
    StorageInspectError,
    lsblk::{LsblkDevice, LsblkOutput},
};

/// Discovers filesystem volumes that can supply external DAIA content.
pub trait ContentVolumeInspector {
    /// Discovers filesystem volumes currently visible to the system.
    ///
    /// # Errors
    ///
    /// Returns an error when system storage cannot be inspected.
    fn inspect(&self) -> Result<Vec<DiscoveredContentVolume>, StorageInspectError>;
}

/// Discovers Linux external-content volumes using `lsblk`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LinuxContentVolumeInspector {
    command: PathBuf,
    findmnt_command: PathBuf,
    post_install_discovery: bool,
    restricted_environment: bool,
}

impl ContentVolumeInspector for LinuxContentVolumeInspector {
    fn inspect(&self) -> Result<Vec<DiscoveredContentVolume>, StorageInspectError> {
        let system_disk_path = self.system_disk_path()?;
        let live_medium_disk_path = self.live_medium_disk_path()?;

        let output = self
            .subprocess(&self.command)
            .arg("--tree")
            .arg("--json")
            .arg("--paths")
            .arg("--bytes")
            .arg("--output")
            .arg("PATH,TYPE,RM,WWN,SERIAL,SIZE,FSTYPE,LABEL,MOUNTPOINT,PARTTYPE,UUID")
            .output()?;

        if !output.status.success() {
            return Err(StorageInspectError::ProcessFailed {
                command: self.command.display().to_string(),
                status: output.status,
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            });
        }

        let parsed: LsblkOutput = serde_json::from_slice(&output.stdout)
            .map_err(|error| StorageInspectError::InvalidOutput(error.to_string()))?;

        Ok(content_volumes(
            &parsed.blockdevices,
            system_disk_path.as_deref(),
            live_medium_disk_path.as_deref(),
            self.post_install_discovery,
        ))
    }
}

impl Default for LinuxContentVolumeInspector {
    fn default() -> Self {
        Self::new()
    }
}

impl LinuxContentVolumeInspector {
    /// Creates an `lsblk`-backed external-content volume inspector.
    #[must_use]
    pub fn new() -> Self {
        Self {
            command: PathBuf::from("lsblk"),
            findmnt_command: PathBuf::from("findmnt"),
            post_install_discovery: false,
            restricted_environment: false,
        }
    }

    /// Uses a custom `lsblk` executable.
    #[must_use]
    pub fn with_command(mut self, command: impl Into<PathBuf>) -> Self {
        self.command = command.into();
        self
    }

    /// Uses a custom `findmnt` executable.
    #[must_use]
    pub fn with_findmnt_command(mut self, command: impl Into<PathBuf>) -> Self {
        self.findmnt_command = command.into();
        self
    }

    /// Enables discovery of additional filesystems on the installed system disk.
    ///
    /// This mode is intended for post-installation content browsing.
    #[must_use]
    pub fn with_post_install_discovery(mut self) -> Self {
        self.post_install_discovery = true;
        self
    }

    /// Enables a minimal subprocess environment for privileged callers.
    #[must_use]
    pub fn with_restricted_environment(mut self) -> Self {
        self.restricted_environment = true;
        self
    }

    fn subprocess(&self, executable: &Path) -> Command {
        let mut command = Command::new(executable);

        if self.restricted_environment {
            command.env_clear();
            command.env("PATH", "/usr/bin:/bin");
            command.env("LC_ALL", "C");
        }

        command
    }

    /// Returns the configured executable.
    #[must_use]
    pub fn command(&self) -> &Path {
        &self.command
    }

    /// Returns the configured `findmnt` executable.
    #[must_use]
    pub fn findmnt_command(&self) -> &Path {
        &self.findmnt_command
    }

    fn system_disk_path(&self) -> Result<Option<PathBuf>, StorageInspectError> {
        let output = self
            .subprocess(&self.findmnt_command)
            .arg("--noheadings")
            .arg("--output")
            .arg("SOURCE")
            .arg("/")
            .output()?;

        if !output.status.success() {
            return Err(StorageInspectError::ProcessFailed {
                command: self.findmnt_command.display().to_string(),
                status: output.status,
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            });
        }

        let source = String::from_utf8_lossy(&output.stdout);
        let source = source.trim();

        if source.is_empty() {
            return Err(StorageInspectError::InvalidOutput(
                "findmnt returned an empty root filesystem source".to_owned(),
            ));
        }

        if !source.starts_with("/dev") {
            return Ok(None);
        }

        let output = self
            .subprocess(&self.command)
            .arg("--paths")
            .arg("--noheadings")
            .arg("--output")
            .arg("PKNAME")
            .arg(source)
            .output()?;

        if !output.status.success() {
            return Err(StorageInspectError::ProcessFailed {
                command: self.command.display().to_string(),
                status: output.status,
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            });
        }

        let parent = String::from_utf8_lossy(&output.stdout);
        let parent = parent.trim();

        if parent.is_empty() {
            return Err(StorageInspectError::InvalidOutput(
                "lsblk returned an empty root parent disk".to_owned(),
            ));
        }

        Ok(Some(PathBuf::from(parent)))
    }

    fn live_medium_disk_path(&self) -> Result<Option<PathBuf>, StorageInspectError> {
        let output = self
            .subprocess(&self.findmnt_command)
            .arg("--noheadings")
            .arg("--output")
            .arg("SOURCE")
            .arg("/run/live/medium")
            .output()?;

        if !output.status.success() {
            return Ok(None);
        }

        let source = String::from_utf8_lossy(&output.stdout);
        let source = source.trim();

        if source.is_empty() || !source.starts_with("/dev") {
            return Ok(None);
        }

        let output = self
            .subprocess(&self.command)
            .arg("--paths")
            .arg("--noheadings")
            .arg("--output")
            .arg("PKNAME")
            .arg(source)
            .output()?;

        if !output.status.success() {
            return Err(StorageInspectError::ProcessFailed {
                command: self.command.display().to_string(),
                status: output.status,
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            });
        }

        let parent = String::from_utf8_lossy(&output.stdout);
        let parent = parent.trim();

        if parent.is_empty() {
            return Err(StorageInspectError::InvalidOutput(
                "lsblk returned an empty live-medium parent disk".to_owned(),
            ));
        }

        Ok(Some(PathBuf::from(parent)))
    }
}

fn content_volumes(
    devices: &[LsblkDevice],
    system_disk_path: Option<&Path>,
    live_medium_disk_path: Option<&Path>,
    post_install_discovery: bool,
) -> Vec<DiscoveredContentVolume> {
    let mut volumes = Vec::new();

    for disk in devices.iter().filter(|device| device.device_type == "disk") {
        let disk_path = Path::new(&disk.path);

        if (!post_install_discovery
            && system_disk_path.is_some_and(|system_disk| disk_path == system_disk))
            || live_medium_disk_path.is_some_and(|live_medium_disk| disk_path == live_medium_disk)
        {
            continue;
        }

        collect_content_volumes(disk, disk_path, &mut volumes, post_install_discovery);
    }

    volumes
}

fn collect_content_volumes(
    device: &LsblkDevice,
    parent_disk: &Path,
    volumes: &mut Vec<DiscoveredContentVolume>,
    post_install_discovery: bool,
) {
    if post_install_discovery
        && device.mountpoint.as_deref().is_some_and(|mountpoint| {
            let path = Path::new(mountpoint);

            path == Path::new("/")
                || [
                    "/boot", "/efi", "/var", "/usr", "/etc", "/home", "/opt", "/srv", "/run",
                    "/proc", "/sys", "/dev", "/tmp",
                ]
                .iter()
                .any(|system_path| path == Path::new(system_path) || path.starts_with(system_path))
        })
    {
        return;
    }

    // Protected boot and recovery partitions are not content sources.
    // Mount authorization independently validates partition metadata.
    const PROTECTED_PARTITION_TYPES: &[&str] = &[
        "c12a7328-f81f-11d2-ba4b-00a0c93ec93b", // EFI
        "21686148-6449-6e6f-744e-656564454649", // BIOS boot
        "de94bba4-06d1-4d40-a16a-bfd50179d6ac", // Microsoft recovery
        "a19d880f-05fc-4d3b-a006-743f0f84911e", // Windows recovery
    ];

    let protected_partition = device
        .partition_type
        .as_deref()
        .is_some_and(|partition_type| {
            PROTECTED_PARTITION_TYPES
                .iter()
                .any(|protected| partition_type.eq_ignore_ascii_case(protected))
        });

    // A whole-disk filesystem is eligible only when the disk has no
    // partition, mapping, or other child block devices.
    if !protected_partition
        && (device.device_type != "disk" || device.children.is_empty())
        && let Some(filesystem_type) = device.filesystem_type.as_deref().filter(|filesystem_type| {
            matches!(
                *filesystem_type,
                "ext4" | "ext3" | "xfs" | "btrfs" | "vfat" | "exfat" | "ntfs"
            )
        })
    {
        let mut volume = DiscoveredContentVolume::new(&device.path, parent_disk, filesystem_type);

        if let Some(label) = device.label.as_deref().filter(|label| !label.is_empty()) {
            volume = volume.with_label(label);
        }

        if let Some(mountpoint) = device
            .mountpoint
            .as_deref()
            .filter(|mountpoint| !mountpoint.is_empty())
        {
            volume = volume.with_mountpoint(mountpoint);
        }

        volumes.push(volume);
    }

    for child in &device.children {
        collect_content_volumes(child, parent_disk, volumes, post_install_discovery);
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

    use tempfile::TempDir;

    use super::{ContentVolumeInspector, LinuxContentVolumeInspector};

    #[test]
    fn restricted_subprocess_uses_minimal_environment() {
        let inspector = LinuxContentVolumeInspector::new().with_restricted_environment();
        let command = inspector.subprocess(std::path::Path::new("/usr/bin/true"));

        assert!(command.get_envs().any(|(key, value)| {
            key == "PATH" && value == Some(std::ffi::OsStr::new("/usr/bin:/bin"))
        }));
        assert!(
            command.get_envs().any(|(key, value)| {
                key == "LC_ALL" && value == Some(std::ffi::OsStr::new("C"))
            })
        );
        assert!(!command.get_envs().any(|(key, _)| key == "HOME"));
    }

    #[test]
    fn restricted_subprocess_does_not_inherit_environment() {
        let inspector = LinuxContentVolumeInspector::new().with_restricted_environment();
        let mut command = inspector.subprocess(std::path::Path::new("/usr/bin/env"));

        command.env("DAIA_TEST_MARKER", "present");
        let output = command
            .output()
            .expect("environment command should execute");

        assert!(output.status.success());

        let environment = String::from_utf8(output.stdout).expect("environment should be UTF-8");

        let entries: Vec<&str> = environment.lines().collect();

        assert!(entries.contains(&"PATH=/usr/bin:/bin"));
        assert!(entries.contains(&"LC_ALL=C"));
        assert!(entries.contains(&"DAIA_TEST_MARKER=present"));
        assert_eq!(entries.len(), 3, "unexpected inherited environment");
    }

    fn command_script(contents: &str) -> (TempDir, std::path::PathBuf) {
        let directory = TempDir::new().expect("temporary directory should be created");
        let command = directory.path().join("lsblk");
        let temporary_command = directory.path().join("lsblk.tmp");

        let mut file =
            File::create(&temporary_command).expect("temporary test command should be created");

        file.write_all(contents.as_bytes())
            .expect("test command should be written");

        file.sync_all()
            .expect("test command should be synchronized");

        drop(file);

        let mut permissions = fs::metadata(&temporary_command)
            .expect("temporary test command metadata should be readable")
            .permissions();

        permissions.set_mode(0o755);

        fs::set_permissions(&temporary_command, permissions)
            .expect("temporary test command should be executable");

        fs::rename(&temporary_command, &command)
            .expect("temporary test command should be installed");

        thread::sleep(Duration::from_millis(10));

        (directory, command)
    }

    fn overlay_findmnt_command() -> (TempDir, std::path::PathBuf) {
        command_script(
            r"#!/bin/sh
echo 'overlay'
",
        )
    }

    #[test]
    fn content_discovery_requests_explicit_lsblk_tree() {
        let (_directory, command) = command_script(
            r#"#!/bin/sh
if [ "$1" != "--tree" ] ||
   [ "$2" != "--json" ] ||
   [ "$3" != "--paths" ] ||
   [ "$4" != "--bytes" ]; then
    echo "missing required lsblk tree arguments: $*" >&2
    exit 1
fi

cat <<'JSON'
{"blockdevices":[{"path":"/dev/sdb","type":"disk","rm":true,"size":61920509952,"children":[{"path":"/dev/sdb1","type":"part","rm":true,"size":61918412800,"fstype":"ext4","label":"DAIA-MODELS"}]}]}
JSON
"#,
        );

        let (_findmnt_directory, findmnt_command) = overlay_findmnt_command();

        let volumes = LinuxContentVolumeInspector::new()
            .with_command(command)
            .with_findmnt_command(findmnt_command)
            .inspect()
            .expect("content discovery must request explicit lsblk tree output");

        assert_eq!(volumes.len(), 1);
        assert_eq!(volumes[0].device_path(), std::path::Path::new("/dev/sdb1"));
        assert_eq!(volumes[0].label(), Some("DAIA-MODELS"));
    }

    #[test]
    fn excludes_efi_partition_but_keeps_fat_data_partition() {
        use crate::lsblk::LsblkDevice;

        fn partition(path: &str, partition_type: &str) -> LsblkDevice {
            LsblkDevice {
                path: path.to_owned(),
                device_type: "part".to_owned(),
                rm: true,
                wwn: None,
                serial: None,
                size: 100_000_000,
                filesystem_type: Some("vfat".to_owned()),
                label: None,
                mountpoint: None,
                partition_type: Some(partition_type.to_owned()),
                filesystem_uuid: Some("test-uuid".to_owned()),
                children: Vec::new(),
            }
        }

        let disk = LsblkDevice {
            path: "/dev/sdb".to_owned(),
            device_type: "disk".to_owned(),
            rm: true,
            wwn: None,
            serial: None,
            size: 300_000_000,
            filesystem_type: None,
            label: None,
            mountpoint: None,
            partition_type: None,
            filesystem_uuid: None,
            children: vec![
                partition("/dev/sdb1", "c12a7328-f81f-11d2-ba4b-00a0c93ec93b"),
                partition("/dev/sdb2", "ebd0a0a2-b9e5-4433-87c0-68b6b72699c7"),
            ],
        };

        let volumes = super::content_volumes(&[disk], None, None, true);
        let paths: Vec<_> = volumes
            .iter()
            .map(|volume| volume.device_path().to_path_buf())
            .collect();

        assert_eq!(paths, vec![std::path::PathBuf::from("/dev/sdb2")]);
    }

    #[test]
    fn discovers_whole_disk_content_filesystem() {
        use crate::lsblk::LsblkDevice;

        let disk = LsblkDevice {
            path: "/dev/sdb".to_owned(),
            device_type: "disk".to_owned(),
            rm: true,
            wwn: None,
            serial: None,
            size: 134_217_728,
            filesystem_type: Some("ext4".to_owned()),
            label: Some("DAIA_MODELS".to_owned()),
            mountpoint: None,
            partition_type: None,
            filesystem_uuid: None,
            children: Vec::new(),
        };

        let volumes = super::content_volumes(&[disk], None, None, false);

        assert_eq!(volumes.len(), 1);
        assert_eq!(volumes[0].device_path(), std::path::Path::new("/dev/sdb"));
        assert_eq!(
            volumes[0].parent_device_path(),
            std::path::Path::new("/dev/sdb")
        );
        assert_eq!(volumes[0].filesystem_type(), "ext4");
        assert_eq!(volumes[0].label(), Some("DAIA_MODELS"));
    }

    #[test]
    fn excludes_whole_disk_filesystem_when_disk_has_children() {
        use crate::lsblk::LsblkDevice;

        let partition = LsblkDevice {
            path: "/dev/sdb1".to_owned(),
            device_type: "part".to_owned(),
            rm: true,
            wwn: None,
            serial: None,
            size: 67_108_864,
            filesystem_type: Some("ext4".to_owned()),
            label: Some("PARTITION_MODELS".to_owned()),
            mountpoint: None,
            partition_type: None,
            filesystem_uuid: None,
            children: Vec::new(),
        };

        let disk = LsblkDevice {
            path: "/dev/sdb".to_owned(),
            device_type: "disk".to_owned(),
            rm: true,
            wwn: None,
            serial: None,
            size: 134_217_728,
            filesystem_type: Some("ext4".to_owned()),
            label: Some("WHOLE_DISK".to_owned()),
            mountpoint: None,
            partition_type: None,
            filesystem_uuid: None,
            children: vec![partition],
        };

        let volumes = super::content_volumes(&[disk], None, None, false);

        assert_eq!(volumes.len(), 1);
        assert_eq!(volumes[0].device_path(), std::path::Path::new("/dev/sdb1"));
        assert_eq!(volumes[0].label(), Some("PARTITION_MODELS"));
    }

    #[test]
    fn excludes_whole_disk_system_filesystem_before_installation() {
        use crate::lsblk::LsblkDevice;

        let disk = LsblkDevice {
            path: "/dev/sda".to_owned(),
            device_type: "disk".to_owned(),
            rm: false,
            wwn: None,
            serial: None,
            size: 134_217_728,
            filesystem_type: Some("ext4".to_owned()),
            label: Some("DAIA_SYSTEM".to_owned()),
            mountpoint: Some("/".to_owned()),
            partition_type: None,
            filesystem_uuid: None,
            children: Vec::new(),
        };

        let volumes =
            super::content_volumes(&[disk], Some(std::path::Path::new("/dev/sda")), None, false);

        assert!(volumes.is_empty());
    }

    #[test]
    fn discovers_content_filesystem_on_usb_disk() {
        let (_directory, command) = command_script(
            r#"#!/bin/sh
cat <<'JSON'
{
  "blockdevices": [
    {
      "path": "/dev/sdb",
      "type": "disk",
      "rm": true,
      "wwn": null,
      "serial": "KINGSTON001",
      "size": 61917364224,
      "children": [
        {
          "path": "/dev/sdb1",
          "type": "part",
          "rm": true,
          "wwn": null,
          "serial": null,
          "size": 61915267072,
          "fstype": "ext4",
          "label": "DAIA-MODELS",
          "mountpoint": null
        }
      ]
    }
  ]
}
JSON
"#,
        );

        let (_findmnt_directory, findmnt_command) = overlay_findmnt_command();

        let inspector = LinuxContentVolumeInspector::new()
            .with_command(command)
            .with_findmnt_command(findmnt_command);

        let volumes = inspector
            .inspect()
            .expect("content volumes should be discovered");

        assert_eq!(volumes.len(), 1);

        let volume = &volumes[0];

        assert_eq!(volume.device_path(), std::path::Path::new("/dev/sdb1"));
        assert_eq!(
            volume.parent_device_path(),
            std::path::Path::new("/dev/sdb")
        );
        assert_eq!(volume.filesystem_type(), "ext4");
        assert_eq!(volume.label(), Some("DAIA-MODELS"));
        assert_eq!(volume.mountpoint(), None);
    }

    #[test]
    fn excludes_filesystems_on_identified_system_disk() {
        let (_findmnt_directory, findmnt_command) = command_script(
            r"#!/bin/sh
echo '/dev/sda2'
",
        );

        let (_directory, command) = command_script(
            r#"#!/bin/sh
if [ "$4" = "PKNAME" ]; then
    echo '/dev/sda'
    exit 0
fi

cat <<'JSON'
{
  "blockdevices": [
    {
      "path": "/dev/sda",
      "type": "disk",
      "rm": false,
      "wwn": null,
      "serial": "SYSTEM001",
      "size": 100000000000,
      "children": [
        {
          "path": "/dev/sda2",
          "type": "part",
          "rm": false,
          "wwn": null,
          "serial": null,
          "size": 90000000000,
          "fstype": "ext4",
          "label": "DAIA-SYSTEM",
          "mountpoint": "/"
        }
      ]
    },
    {
      "path": "/dev/sdb",
      "type": "disk",
      "rm": true,
      "wwn": null,
      "serial": "KINGSTON001",
      "size": 61917364224,
      "children": [
        {
          "path": "/dev/sdb1",
          "type": "part",
          "rm": true,
          "wwn": null,
          "serial": null,
          "size": 61915267072,
          "fstype": "ext4",
          "label": "DAIA-MODELS",
          "mountpoint": null
        }
      ]
    }
  ]
}
JSON
"#,
        );

        let inspector = LinuxContentVolumeInspector::new()
            .with_command(command)
            .with_findmnt_command(findmnt_command);

        let volumes = inspector
            .inspect()
            .expect("eligible content volumes should be discovered");

        assert_eq!(volumes.len(), 1);
        assert_eq!(volumes[0].device_path(), std::path::Path::new("/dev/sdb1"));
        assert_eq!(
            volumes[0].parent_device_path(),
            std::path::Path::new("/dev/sdb")
        );
        assert_eq!(volumes[0].label(), Some("DAIA-MODELS"));
    }

    #[test]
    fn post_install_excludes_system_mountpoints_but_keeps_data_volumes() {
        use crate::lsblk::LsblkDevice;

        fn partition(name: &str, mountpoint: Option<&str>) -> LsblkDevice {
            LsblkDevice {
                path: format!("/dev/{name}"),
                device_type: "part".to_owned(),
                rm: false,
                wwn: None,
                serial: None,
                size: 1_000_000,
                filesystem_type: Some("ext4".to_owned()),
                label: None,
                mountpoint: mountpoint.map(str::to_owned),
                partition_type: None,
                filesystem_uuid: None,
                children: Vec::new(),
            }
        }

        let disk = LsblkDevice {
            path: "/dev/sda".to_owned(),
            device_type: "disk".to_owned(),
            rm: false,
            wwn: None,
            serial: None,
            size: 100_000_000,
            filesystem_type: None,
            label: None,
            mountpoint: None,
            partition_type: None,
            filesystem_uuid: None,
            children: vec![
                partition("sda1", Some("/")),
                partition("sda2", Some("/boot")),
                partition("sda3", Some("/boot/efi")),
                partition("sda4", Some("/var")),
                partition("sda5", Some("/media/models")),
                partition("sda6", None),
            ],
        };

        let volumes =
            super::content_volumes(&[disk], Some(std::path::Path::new("/dev/sda")), None, true);

        let paths = volumes
            .iter()
            .map(|volume| volume.device_path().to_path_buf())
            .collect::<Vec<_>>();

        assert_eq!(
            paths,
            vec![
                std::path::PathBuf::from("/dev/sda5"),
                std::path::PathBuf::from("/dev/sda6"),
            ],
            "post-install discovery must exclude mounted system filesystems"
        );
    }

    #[test]
    fn post_install_discovery_includes_secondary_system_disk_partition() {
        let (_findmnt_directory, findmnt_command) = command_script(
            r#"#!/bin/sh
case "$*" in
    *"/run/live/medium"*)
        exit 1
        ;;
    *" /")
        echo '/dev/sda2'
        ;;
    *)
        exit 1
        ;;
esac
"#,
        );

        let (_directory, command) = command_script(
            r#"#!/bin/sh
if [ "$4" = "PKNAME" ]; then
    echo '/dev/sda'
    exit 0
fi

cat <<'JSON'
{
  "blockdevices": [
    {
      "path": "/dev/sda",
      "type": "disk",
      "rm": false,
      "wwn": null,
      "serial": "SYSTEM001",
      "size": 100000000000,
      "children": [
        {
          "path": "/dev/sda2",
          "type": "part",
          "rm": false,
          "wwn": null,
          "serial": null,
          "size": 50000000000,
          "fstype": "ext4",
          "label": "DAIA-SYSTEM",
          "mountpoint": "/"
        },
        {
          "path": "/dev/sda3",
          "type": "part",
          "rm": false,
          "wwn": null,
          "serial": null,
          "size": 50000000000,
          "fstype": "ext4",
          "label": "LOCAL-MODELS",
          "mountpoint": "/media/models"
        }
      ]
    }
  ]
}
JSON
"#,
        );

        let inspector = LinuxContentVolumeInspector::new()
            .with_command(command)
            .with_findmnt_command(findmnt_command)
            .with_post_install_discovery();

        let volumes = inspector
            .inspect()
            .expect("post-installation volumes should be discovered");

        assert!(
            volumes
                .iter()
                .all(|volume| { volume.device_path() != std::path::Path::new("/dev/sda2") }),
            "active root filesystem must not be offered as external content"
        );

        assert!(
            volumes.iter().any(
                |volume| volume.device_path() == std::path::Path::new("/dev/sda3")
                    && volume.mountpoint() == Some(std::path::Path::new("/media/models"))
            ),
            "secondary system-disk filesystem should be discoverable"
        );
    }

    #[test]
    fn excludes_filesystems_on_live_installation_medium() {
        let (_findmnt_directory, findmnt_command) = command_script(
            r#"#!/bin/sh
case "$*" in
    *"/run/live/medium"*)
        echo '/dev/sdc1'
        ;;
    *" /")
        echo 'overlay'
        ;;
    *)
        exit 1
        ;;
esac
"#,
        );

        let (_directory, command) = command_script(
            r#"#!/bin/sh
if [ "$4" = "PKNAME" ] && [ "$5" = "/dev/sdc1" ]; then
    echo '/dev/sdc'
    exit 0
fi

cat <<'JSON'
{
  "blockdevices": [
    {
      "path": "/dev/sdb",
      "type": "disk",
      "rm": true,
      "wwn": null,
      "serial": "CONTENT001",
      "size": 61917364224,
      "children": [
        {
          "path": "/dev/sdb1",
          "type": "part",
          "rm": true,
          "wwn": null,
          "serial": null,
          "size": 61915267072,
          "fstype": "ext4",
          "label": "DAIA-MODELS",
          "mountpoint": null
        }
      ]
    },
    {
      "path": "/dev/sdc",
      "type": "disk",
      "rm": true,
      "wwn": null,
      "serial": "DAIA-LIVE001",
      "size": 16000000000,
      "children": [
        {
          "path": "/dev/sdc1",
          "type": "part",
          "rm": true,
          "wwn": null,
          "serial": null,
          "size": 15900000000,
          "fstype": "iso9660",
          "label": "DAIA-LIVE",
          "mountpoint": "/run/live/medium"
        }
      ]
    }
  ]
}
JSON
"#,
        );

        let inspector = LinuxContentVolumeInspector::new()
            .with_command(command)
            .with_findmnt_command(findmnt_command);

        let volumes = inspector
            .inspect()
            .expect("eligible content volumes should be discovered");

        assert_eq!(volumes.len(), 1);
        assert_eq!(volumes[0].device_path(), std::path::Path::new("/dev/sdb1"));
        assert_eq!(
            volumes[0].parent_device_path(),
            std::path::Path::new("/dev/sdb")
        );
        assert_eq!(volumes[0].label(), Some("DAIA-MODELS"));
    }

    #[test]
    fn reports_failed_lsblk_command() {
        let (_directory, command) = command_script(
            r"#!/bin/sh
echo 'content lsblk failed' >&2
exit 1
",
        );

        let (_findmnt_directory, findmnt_command) = overlay_findmnt_command();

        let inspector = LinuxContentVolumeInspector::new()
            .with_command(command)
            .with_findmnt_command(findmnt_command);

        let error = inspector
            .inspect()
            .expect_err("failed lsblk should fail content-volume inspection");

        assert!(error.to_string().contains("content lsblk failed"));
    }

    #[test]
    fn reports_invalid_lsblk_json() {
        let (_directory, command) = command_script(
            r"#!/bin/sh
echo 'not json'
",
        );

        let (_findmnt_directory, findmnt_command) = overlay_findmnt_command();

        let inspector = LinuxContentVolumeInspector::new()
            .with_command(command)
            .with_findmnt_command(findmnt_command);

        let error = inspector
            .inspect()
            .expect_err("invalid lsblk JSON should fail content-volume inspection");

        assert!(
            error
                .to_string()
                .contains("invalid storage inspection output")
        );
    }
}
