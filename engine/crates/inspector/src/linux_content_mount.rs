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

    /// Uses a custom DAIA-owned mount root.
    #[must_use]
    pub fn with_mount_root(mut self, mount_root: impl Into<PathBuf>) -> Self {
        self.mount_root = mount_root.into();
        self
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
    };

    use model::DiscoveredContentVolume;
    use tempfile::TempDir;

    use super::{ContentVolumeMounter, LinuxContentVolumeMounter};

    fn command_script(contents: &str) -> (TempDir, std::path::PathBuf) {
        let directory = TempDir::new().expect("temporary directory should be created");
        let command = directory.path().join("mount");

        let mut file = File::create(&command).expect("temporary mount command should be created");

        file.write_all(contents.as_bytes())
            .expect("temporary mount command should be written");

        file.sync_all()
            .expect("temporary mount command should be synchronized");

        drop(file);

        let mut permissions = fs::metadata(&command)
            .expect("temporary mount command metadata should be readable")
            .permissions();

        permissions.set_mode(0o755);

        fs::set_permissions(&command, permissions)
            .expect("temporary mount command should be executable");

        (directory, command)
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

        assert!(error.to_string().contains("content unmount failed"));
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

        assert!(error.to_string().contains("read-only mount failed"));
    }
}
