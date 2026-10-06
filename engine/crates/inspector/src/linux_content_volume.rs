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
}

impl ContentVolumeInspector for LinuxContentVolumeInspector {
    fn inspect(&self) -> Result<Vec<DiscoveredContentVolume>, StorageInspectError> {
        let system_disk_path = self.system_disk_path()?;

        let output = Command::new(&self.command)
            .arg("--json")
            .arg("--paths")
            .arg("--bytes")
            .arg("--output")
            .arg("PATH,TYPE,RM,WWN,SERIAL,SIZE,FSTYPE,LABEL,MOUNTPOINT")
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
        let output = Command::new(&self.findmnt_command)
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

        let output = Command::new(&self.command)
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
}

fn content_volumes(
    devices: &[LsblkDevice],
    system_disk_path: Option<&Path>,
) -> Vec<DiscoveredContentVolume> {
    let mut volumes = Vec::new();

    for disk in devices.iter().filter(|device| device.device_type == "disk") {
        let disk_path = Path::new(&disk.path);

        if system_disk_path.is_some_and(|system_disk| disk_path == system_disk) {
            continue;
        }

        for child in &disk.children {
            collect_content_volumes(child, disk_path, &mut volumes);
        }
    }

    volumes
}

fn collect_content_volumes(
    device: &LsblkDevice,
    parent_disk: &Path,
    volumes: &mut Vec<DiscoveredContentVolume>,
) {
    if let Some(filesystem_type) = device
        .filesystem_type
        .as_deref()
        .filter(|filesystem_type| !filesystem_type.is_empty())
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
        collect_content_volumes(child, parent_disk, volumes);
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
