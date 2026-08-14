//! Linux storage discovery backed by `lsblk`.

use std::{
    path::{Path, PathBuf},
    process::Command,
};

use model::DiscoveredStorage;

use crate::{
    StorageInspectError, StorageInspector,
    lsblk::{LsblkDevice, LsblkOutput},
};

/// Discovers Linux storage using `lsblk`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LinuxStorageInspector {
    command: PathBuf,
}

impl StorageInspector for LinuxStorageInspector {
    fn inspect(&self) -> Result<Vec<DiscoveredStorage>, StorageInspectError> {
        let output = Command::new(&self.command)
            .arg("--json")
            .arg("--paths")
            .arg("--output")
            .arg("PATH,TYPE,RM,WWN,SERIAL")
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

        Ok(parsed
            .blockdevices
            .into_iter()
            .filter(|device| device.device_type == "disk")
            .map(|device| {
                let kind = if device.rm {
                    model::StorageKind::Removable
                } else {
                    model::StorageKind::Secondary
                };

                DiscoveredStorage::new(storage_identity(&device), kind, device.path)
            })
            .collect())
    }
}

impl Default for LinuxStorageInspector {
    fn default() -> Self {
        Self::new()
    }
}

impl LinuxStorageInspector {
    /// Creates an `lsblk`-backed storage inspector.
    #[must_use]
    pub fn new() -> Self {
        Self {
            command: PathBuf::from("lsblk"),
        }
    }

    /// Uses a custom `lsblk` executable.
    #[must_use]
    pub fn with_command(mut self, command: impl Into<PathBuf>) -> Self {
        self.command = command.into();
        self
    }

    /// Returns the configured executable.
    #[must_use]
    pub fn command(&self) -> &Path {
        &self.command
    }
}

fn storage_identity(device: &LsblkDevice) -> String {
    if let Some(wwn) = device.wwn.as_deref() {
        return format!("wwn:{wwn}");
    }

    if let Some(serial) = device.serial.as_deref() {
        return format!("serial:{serial}");
    }

    format!("path:{}", device.path)
}

#[cfg(test)]
mod tests {
    use std::{fs, os::unix::fs::PermissionsExt};

    use super::{LinuxStorageInspector, storage_identity};
    use crate::{StorageInspectError, StorageInspector, lsblk::LsblkDevice};
    use model::StorageKind;
    use tempfile::TempDir;

    fn command_script(contents: &str) -> (TempDir, std::path::PathBuf) {
        let directory = TempDir::new().expect("temporary directory should be created");
        let command = directory.path().join("lsblk");

        fs::write(&command, contents).expect("test command should be written");

        let mut permissions = fs::metadata(&command)
            .expect("test command metadata should be readable")
            .permissions();

        permissions.set_mode(0o755);

        fs::set_permissions(&command, permissions).expect("test command should be executable");

        (directory, command)
    }

    #[test]
    fn storage_identity_prefers_wwn() {
        let device = LsblkDevice {
            path: "/dev/nvme0n1".to_owned(),
            device_type: "disk".to_owned(),
            rm: false,
            wwn: Some("eui.2c3ebffff000220b".to_owned()),
            serial: Some("AA000000000000008715".to_owned()),
        };

        assert_eq!(storage_identity(&device), "wwn:eui.2c3ebffff000220b");
    }

    #[test]
    fn storage_identity_falls_back_to_serial() {
        let device = LsblkDevice {
            path: "/dev/sda".to_owned(),
            device_type: "disk".to_owned(),
            rm: true,
            wwn: None,
            serial: Some("E0D55E6B6466E78088300791".to_owned()),
        };

        assert_eq!(storage_identity(&device), "serial:E0D55E6B6466E78088300791");
    }

    #[test]
    fn storage_identity_uses_path_as_runtime_fallback() {
        let device = LsblkDevice {
            path: "/dev/sdz".to_owned(),
            device_type: "disk".to_owned(),
            rm: false,
            wwn: None,
            serial: None,
        };

        assert_eq!(storage_identity(&device), "path:/dev/sdz");
    }

    #[test]
    fn discovers_disks_from_lsblk_json() {
        let (_directory, command) = command_script(
            r#"#!/bin/sh
cat <<'EOF'
{
  "blockdevices": [
    {"path": "/dev/sda", "type": "disk", "rm": false},
    {"path": "/dev/sda1", "type": "part", "rm": false},
    {"path": "/dev/sdb", "type": "disk", "rm": true}
  ]
}
EOF
"#,
        );

        let inspector = LinuxStorageInspector::new().with_command(command);
        let storage = inspector.inspect().expect("storage should be discovered");

        assert_eq!(storage.len(), 2);

        assert_eq!(storage[0].device_path(), std::path::Path::new("/dev/sda"));
        assert_eq!(storage[0].kind(), StorageKind::Secondary);

        assert_eq!(storage[1].device_path(), std::path::Path::new("/dev/sdb"));
        assert_eq!(storage[1].kind(), StorageKind::Removable);
    }

    #[test]
    fn reports_failed_lsblk_command() {
        let (_directory, command) = command_script(
            r#"#!/bin/sh
echo "lsblk failed" >&2
exit 1
"#,
        );

        let inspector = LinuxStorageInspector::new().with_command(command);
        let error = inspector.inspect().expect_err("inspection should fail");

        assert!(
            matches!(error, StorageInspectError::ProcessFailed { .. }),
            "unexpected error: {error:?}"
        );
        assert!(error.to_string().contains("lsblk failed"));
    }

    #[test]
    fn reports_invalid_lsblk_json() {
        let (_directory, command) = command_script(
            r"#!/bin/sh
     echo 'not json'
     ",
        );

        let inspector = LinuxStorageInspector::new().with_command(command);
        let error = inspector.inspect().expect_err("inspection should fail");

        assert!(
            matches!(error, StorageInspectError::InvalidOutput(_)),
            "unexpected error: {error:?}"
        );
    }
}
