//! Linux storage discovery backed by `lsblk`.

use std::{
    collections::{HashMap, HashSet},
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
    findmnt_command: PathBuf,
}

impl StorageInspector for LinuxStorageInspector {
    fn inspect(&self) -> Result<Vec<DiscoveredStorage>, StorageInspectError> {
        let root_source = self.root_source()?;
        let live_medium_source = self.live_medium_source()?;

        if root_source == Path::new("overlay") && live_medium_source.is_none() {
            return Err(StorageInspectError::InvalidOutput(
                "overlay root has no identifiable live installation medium".to_owned(),
            ));
        }

        let output = Command::new(&self.command)
            .arg("--tree")
            .arg("--json")
            .arg("--paths")
            .arg("--bytes")
            .arg("--output")
            .arg("PATH,TYPE,RM,WWN,SERIAL,SIZE")
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

        let mut parents = HashMap::new();

        for device in &parsed.blockdevices {
            collect_device_parents(device, &mut parents);
        }

        let mut protected = HashSet::new();

        for source in [Some(root_source), live_medium_source]
            .into_iter()
            .flatten()
        {
            if !source.starts_with("/dev/") {
                continue;
            }

            let mut visited = HashSet::new();
            let mut pending = vec![source.to_string_lossy().into_owned()];
            let mut matched = false;

            while let Some(path) = pending.pop() {
                if !visited.insert(path.clone()) {
                    continue;
                }

                for device in &parsed.blockdevices {
                    if device.device_type == "disk" && device.path == path {
                        protected.insert(path.clone());
                        matched = true;
                    }
                }

                if let Some(backing) = parents.get(&path) {
                    pending.extend(backing.iter().cloned());
                }
            }

            if !matched {
                let optical_source = parsed.blockdevices.iter().any(|device| {
                    device.path == source.to_string_lossy() && device.device_type == "rom"
                });

                if !optical_source {
                    return Err(StorageInspectError::InvalidOutput(format!(
                        "no physical disk backs protected source {}",
                        source.display()
                    )));
                }
            }
        }

        Ok(parsed
            .blockdevices
            .into_iter()
            .filter(|device| device.device_type == "disk")
            .map(|device| {
                let kind = if protected.contains(&device.path) {
                    model::StorageKind::System
                } else if device.rm {
                    model::StorageKind::Removable
                } else {
                    model::StorageKind::Secondary
                };

                DiscoveredStorage::new(storage_identity(&device), kind, device.path)
                    .with_size_bytes(device.size)
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

    fn root_source(&self) -> Result<PathBuf, StorageInspectError> {
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

        Ok(PathBuf::from(source))
    }

    fn live_medium_source(&self) -> Result<Option<PathBuf>, StorageInspectError> {
        let output = Command::new(&self.findmnt_command)
            .arg("--noheadings")
            .arg("--output")
            .arg("SOURCE")
            .arg("/run/live/medium")
            .output()?;

        if !output.status.success() {
            if output.status.code() == Some(1) && output.stderr.is_empty() {
                return Ok(None);
            }

            return Err(StorageInspectError::ProcessFailed {
                command: self.findmnt_command.display().to_string(),
                status: output.status,
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            });
        }

        let source = String::from_utf8_lossy(&output.stdout);
        let source = source.trim();

        if !source.starts_with("/dev/") {
            return Err(StorageInspectError::InvalidOutput(
                "live medium source is not a block device".to_owned(),
            ));
        }

        Ok(Some(PathBuf::from(source)))
    }
}

fn collect_device_parents(device: &LsblkDevice, parents: &mut HashMap<String, HashSet<String>>) {
    for child in &device.children {
        parents
            .entry(child.path.clone())
            .or_default()
            .insert(device.path.clone());

        collect_device_parents(child, parents);
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
    use std::{
        fs::{self, File},
        io::Write,
        os::unix::fs::PermissionsExt,
        thread,
        time::Duration,
    };

    use super::{LinuxStorageInspector, storage_identity};
    use crate::{StorageInspectError, StorageInspector, lsblk::LsblkDevice};
    use model::StorageKind;
    use tempfile::TempDir;

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

    #[test]
    fn reports_failed_findmnt_command() {
        let (_findmnt_directory, findmnt_command) = command_script(
            r"#!/bin/sh
echo 'findmnt failed' >&2
exit 1
",
        );

        let inspector = LinuxStorageInspector::new().with_findmnt_command(findmnt_command);

        let error = inspector
            .root_source()
            .expect_err("root-source inspection should fail");

        assert!(
            matches!(error, StorageInspectError::ProcessFailed { .. }),
            "unexpected findmnt error: {error:?}"
        );
        assert!(error.to_string().contains("findmnt failed"));
    }

    #[test]
    fn discovers_root_filesystem_source() {
        let (_directory, command) = command_script(
            r"#!/bin/sh
echo '/dev/nvme0n1p2'
",
        );

        let inspector = LinuxStorageInspector::new().with_findmnt_command(command);

        assert_eq!(
            inspector
                .root_source()
                .expect("root source should be discovered"),
            std::path::PathBuf::from("/dev/nvme0n1p2")
        );
    }

    #[test]
    fn configures_findmnt_command() {
        let inspector = LinuxStorageInspector::new().with_findmnt_command("/custom/findmnt");

        assert_eq!(
            inspector.findmnt_command(),
            std::path::Path::new("/custom/findmnt")
        );
    }

    #[test]
    fn storage_identity_prefers_wwn() {
        let device = LsblkDevice {
            path: "/dev/nvme0n1".to_owned(),
            device_type: "disk".to_owned(),
            rm: false,
            wwn: Some("eui.2c3ebffff000220b".to_owned()),
            serial: Some("AA000000000000008715".to_owned()),
            size: 0,
            filesystem_type: None,
            label: None,
            mountpoint: None,
            children: Vec::new(),
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
            size: 0,
            filesystem_type: None,
            label: None,
            mountpoint: None,
            children: Vec::new(),
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
            size: 0,
            filesystem_type: None,
            label: None,
            mountpoint: None,
            children: Vec::new(),
        };

        assert_eq!(storage_identity(&device), "path:/dev/sdz");
    }

    #[test]
    fn discovers_disks_from_lsblk_json() {
        let (_findmnt_directory, findmnt_command) = command_script(
            r#"#!/bin/sh
case "$*" in
  *"/run/live/medium"*) exit 1 ;;
  *) echo '/dev/sda2' ;;
esac
"#,
        );

        let (_lsblk_directory, lsblk_command) = command_script(
            r#"#!/bin/sh

case "$*" in
  *"PKNAME"*)
    echo '/dev/sda'
    ;;
  *)
    cat <<'EOF'
{
  "blockdevices": [
    {
      "path": "/dev/sda",
      "type": "disk",
      "rm": false,
      "wwn": "0x5001b448bd521e4b",
      "serial": "223020803525",
      "size": 500107862016,
      "children": [
        {
          "path": "/dev/sda2",
          "type": "part",
          "rm": false,
          "wwn": null,
          "serial": null,
          "size": 400000000000
        }
      ]
    },
    {
      "path": "/dev/sdb",
      "type": "disk",
      "rm": false,
      "wwn": "0x5001b448bd999999",
      "serial": "SECONDARY001",
      "size": 1000204886016
    },
    {
      "path": "/dev/sdc",
      "type": "disk",
      "rm": true,
      "wwn": null,
      "serial": "USB001",
      "size": 32010928128
    }
  ]
}
EOF
    ;;
esac
"#,
        );

        let inspector = LinuxStorageInspector::new()
            .with_command(lsblk_command)
            .with_findmnt_command(findmnt_command);

        let storage = inspector.inspect().expect("storage should be discovered");

        assert_eq!(storage.len(), 3);

        assert_eq!(storage[0].device_path(), std::path::Path::new("/dev/sda"));
        assert_eq!(storage[0].kind(), StorageKind::System);

        assert_eq!(storage[1].device_path(), std::path::Path::new("/dev/sdb"));
        assert_eq!(storage[1].kind(), StorageKind::Secondary);
        assert_eq!(storage[1].size_bytes(), Some(1_000_204_886_016));

        assert_eq!(storage[2].device_path(), std::path::Path::new("/dev/sdc"));
        assert_eq!(storage[2].kind(), StorageKind::Removable);
    }

    #[test]
    fn discovers_installable_disk_when_root_is_live_overlay() {
        let (_findmnt_directory, findmnt_command) = command_script(
            r#"#!/bin/sh
case "$*" in
  *"/run/live/medium"*) echo '/dev/sr0' ;;
  *) echo 'overlay' ;;
esac
"#,
        );

        let (_lsblk_directory, lsblk_command) = command_script(
            r#"#!/bin/sh

case "$*" in
  *"PKNAME"*)
    echo 'lsblk must not inspect a non-device root source' >&2
    exit 32
    ;;
  *)
    cat <<'EOF'
{
  "blockdevices": [
    {
      "path": "/dev/sr0",
      "type": "rom",
      "rm": true,
      "wwn": null,
      "serial": null,
      "size": 4000000000
    },
    {
      "path": "/dev/vda",
      "type": "disk",
      "rm": false,
      "wwn": null,
      "serial": "INSTALL001",
      "size": 42949672960
    }
  ]
}
EOF
    ;;
esac
"#,
        );

        let inspector = LinuxStorageInspector::new()
            .with_command(lsblk_command)
            .with_findmnt_command(findmnt_command);

        let storage = inspector
            .inspect()
            .expect("live-root storage discovery should succeed");

        assert_eq!(storage.len(), 1);
        assert_eq!(storage[0].device_path(), std::path::Path::new("/dev/vda"));
        assert_eq!(storage[0].kind(), StorageKind::Secondary);
        assert_eq!(storage[0].size_bytes(), Some(42_949_672_960));
    }

    #[test]
    fn rejects_protected_root_with_unresolved_device_chain() {
        let (_findmnt_dir, findmnt) = command_script(
            r#"#!/bin/sh
case "$*" in
  *"/run/live/medium"*) exit 1 ;;
  *) echo '/dev/mapper/root' ;;
esac
"#,
        );

        let (_lsblk_dir, lsblk) = command_script(
            r#"#!/bin/sh
cat <<'EOF'
{
  "blockdevices": [
    {
      "path": "/dev/sda",
      "type": "disk",
      "rm": false,
      "wwn": null,
      "serial": "SYSTEM-DISK",
      "size": 1000000000
    },
    {
      "path": "/dev/sdb",
      "type": "disk",
      "rm": false,
      "wwn": null,
      "serial": "INSTALL-TARGET",
      "size": 2000000000
    }
  ]
}
EOF
"#,
        );

        let inspector = LinuxStorageInspector::new()
            .with_command(lsblk)
            .with_findmnt_command(findmnt);

        let error = inspector
            .inspect()
            .expect_err("unresolved root device must fail closed");

        assert!(
            matches!(error, StorageInspectError::InvalidOutput(_)),
            "unexpected error: {error:?}"
        );

        assert!(
            error
                .to_string()
                .contains("no physical disk backs protected source"),
            "unexpected error: {error:?}"
        );
    }

    #[test]
    fn protects_all_disks_backing_stacked_root() {
        let (_findmnt_dir, findmnt) = command_script(
            r#"#!/bin/sh
case "$*" in
  *"/run/live/medium"*) exit 1 ;;
  *) echo '/dev/mapper/root' ;;
esac
"#,
        );

        let (_lsblk_dir, lsblk) = command_script(
            r#"#!/bin/sh
case "$*" in
  *"PKNAME"*)
    echo '/dev/md0'
    ;;
  *)
    cat <<'EOF'
{
  "blockdevices": [
    {
      "path": "/dev/sda",
      "type": "disk",
      "rm": false,
      "wwn": null,
      "serial": "ROOT-A",
      "size": 1000000000,
      "children": [
        {
          "path": "/dev/sda1",
          "type": "part",
          "rm": false,
          "wwn": null,
          "serial": null,
          "size": 900000000,
          "children": [
            {
              "path": "/dev/md0",
              "type": "raid1",
              "rm": false,
              "wwn": null,
              "serial": null,
              "size": 800000000,
              "children": [
                {
                  "path": "/dev/mapper/root",
                  "type": "crypt",
                  "rm": false,
                  "wwn": null,
                  "serial": null,
                  "size": 700000000
                }
              ]
            }
          ]
        }
      ]
    },
    {
      "path": "/dev/sdb",
      "type": "disk",
      "rm": false,
      "wwn": null,
      "serial": "ROOT-B",
      "size": 1000000000,
      "children": [
        {
          "path": "/dev/sdb1",
          "type": "part",
          "rm": false,
          "wwn": null,
          "serial": null,
          "size": 900000000,
          "children": [
            {
              "path": "/dev/md0",
              "type": "raid1",
              "rm": false,
              "wwn": null,
              "serial": null,
              "size": 800000000
            }
          ]
        }
      ]
    },
    {
      "path": "/dev/sdc",
      "type": "disk",
      "rm": false,
      "wwn": null,
      "serial": "INSTALL-TARGET",
      "size": 2000000000
    }
  ]
}
EOF
    ;;
esac
"#,
        );

        let inspector = LinuxStorageInspector::new()
            .with_command(lsblk)
            .with_findmnt_command(findmnt);

        let disks = inspector
            .inspect()
            .expect("storage inspection should succeed");

        assert_eq!(disks.len(), 3);
        assert_eq!(disks[0].kind(), StorageKind::System);
        assert_eq!(disks[1].kind(), StorageKind::System);
        assert_eq!(disks[2].kind(), StorageKind::Secondary);
    }

    #[test]
    fn allows_installation_target_when_live_medium_is_optical() {
        let (_findmnt_dir, findmnt) = command_script(
            r#"#!/bin/sh
case "$*" in
  *"/run/live/medium"*) echo '/dev/sr0' ;;
  *) echo 'overlay' ;;
esac
"#,
        );

        let (_lsblk_dir, lsblk) = command_script(
            r#"#!/bin/sh
cat <<'EOF'
{
  "blockdevices": [
    {
      "path": "/dev/sr0",
      "type": "rom",
      "rm": true,
      "wwn": null,
      "serial": null,
      "size": 4000000000
    },
    {
      "path": "/dev/vda",
      "type": "disk",
      "rm": false,
      "wwn": null,
      "serial": "INSTALL-TARGET",
      "size": 42949672960
    }
  ]
}
EOF
"#,
        );

        let inspector = LinuxStorageInspector::new()
            .with_command(lsblk)
            .with_findmnt_command(findmnt);

        let disks = inspector
            .inspect()
            .expect("optical live media should not block disk discovery");

        assert_eq!(disks.len(), 1);
        assert_eq!(disks[0].id().as_str(), "serial:INSTALL-TARGET");
        assert_eq!(disks[0].kind(), StorageKind::Secondary);
    }

    #[test]
    fn rejects_overlay_root_without_live_medium() {
        let (_findmnt_dir, findmnt) = command_script(
            r#"#!/bin/sh
case "$*" in
  *"/run/live/medium"*) exit 1 ;;
  *) echo 'overlay' ;;
esac
"#,
        );

        let (_lsblk_dir, lsblk) = command_script(
            r#"#!/bin/sh
cat <<'EOF'
{
  "blockdevices": [
    {
      "path": "/dev/sda",
      "type": "disk",
      "rm": false,
      "wwn": null,
      "serial": "POTENTIAL-LIVE-DISK",
      "size": 1000000000
    }
  ]
}
EOF
"#,
        );

        let inspector = LinuxStorageInspector::new()
            .with_command(lsblk)
            .with_findmnt_command(findmnt);

        let error = inspector
            .inspect()
            .expect_err("overlay root without identifiable boot media must fail closed");

        assert!(
            matches!(error, StorageInspectError::InvalidOutput(_)),
            "unexpected error: {error:?}"
        );
    }

    #[test]
    fn protects_live_usb_while_preserving_installation_target() {
        let (_findmnt_dir, findmnt) = command_script(
            r#"#!/bin/sh
case "$*" in
  *"/run/live/medium"*) echo '/dev/sdc1' ;;
  *) echo 'overlay' ;;
esac
"#,
        );

        let (_lsblk_dir, lsblk) = command_script(
            r#"#!/bin/sh
case "$*" in
  *"PKNAME"*)
    echo '/dev/sdc'
    ;;
  *)
    cat <<'EOF'
{
  "blockdevices": [
    {
      "path": "/dev/sdc",
      "type": "disk",
      "rm": true,
      "wwn": null,
      "serial": "DAIA-LIVE",
      "size": 16000000000,
      "children": [
        {
          "path": "/dev/sdc1",
          "type": "part",
          "rm": true,
          "wwn": null,
          "serial": null,
          "size": 15000000000
        }
      ]
    },
    {
      "path": "/dev/vda",
      "type": "disk",
      "rm": false,
      "wwn": null,
      "serial": "INSTALL-TARGET",
      "size": 42949672960
    }
  ]
}
EOF
    ;;
esac
"#,
        );

        let inspector = LinuxStorageInspector::new()
            .with_command(lsblk)
            .with_findmnt_command(findmnt);

        let disks = inspector
            .inspect()
            .expect("live disks should be discovered");

        assert_eq!(disks.len(), 2);
        assert_eq!(disks[0].kind(), StorageKind::System);
        assert_eq!(disks[1].kind(), StorageKind::Secondary);
    }

    #[test]
    fn rejects_live_medium_findmnt_diagnostic_failure() {
        let (_findmnt_dir, findmnt) = command_script(
            r#"#!/bin/sh
case "$*" in
  *"/run/live/medium"*)
    echo 'live medium inspection failed' >&2
    exit 1
    ;;
  *)
    echo 'overlay'
    ;;
esac
"#,
        );

        let inspector = LinuxStorageInspector::new().with_findmnt_command(findmnt);

        let error = inspector
            .inspect()
            .expect_err("live-medium inspection failure must be rejected");

        assert!(matches!(error, StorageInspectError::ProcessFailed { .. }));
        assert!(error.to_string().contains("live medium inspection failed"));
    }

    #[test]
    fn rejects_live_medium_with_missing_parent_disk() {
        let (_findmnt_dir, findmnt) = command_script(
            r#"#!/bin/sh
case "$*" in
  *"/run/live/medium"*) echo '/dev/sdc1' ;;
  *) echo 'overlay' ;;
esac
"#,
        );

        let (_lsblk_dir, lsblk) = command_script(
            r"#!/bin/sh
exit 0
",
        );

        let inspector = LinuxStorageInspector::new()
            .with_command(lsblk)
            .with_findmnt_command(findmnt);

        let error = inspector
            .inspect()
            .expect_err("missing live-medium parent must fail inspection");

        assert!(matches!(error, StorageInspectError::InvalidOutput(_)));
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
