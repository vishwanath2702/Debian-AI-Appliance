//! ISO reader backed by the external `xorriso` command.

use std::{
    path::{Path, PathBuf},
    process::Command,
};

use crate::{InspectError, IsoReader};

/// Reads files from an ISO image using `xorriso`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XorrisoReader {
    iso: PathBuf,
    command: PathBuf,
}

impl XorrisoReader {
    /// Creates an `xorriso`-backed ISO reader.
    #[must_use]
    pub fn new(iso: impl Into<PathBuf>) -> Self {
        Self {
            iso: iso.into(),
            command: PathBuf::from("xorriso"),
        }
    }

    /// Uses a custom `xorriso` executable.
    #[must_use]
    pub fn with_command(mut self, command: impl Into<PathBuf>) -> Self {
        self.command = command.into();
        self
    }

    /// Returns the configured ISO path.
    #[must_use]
    pub fn iso(&self) -> &Path {
        &self.iso
    }

    /// Returns the configured executable.
    #[must_use]
    pub fn command(&self) -> &Path {
        &self.command
    }
}

impl IsoReader for XorrisoReader {
    fn read_file(&self, iso_path: &str) -> Result<Vec<u8>, InspectError> {
        let temp_directory = tempfile::tempdir().map_err(|error| InspectError::Io(error))?;

        let filename = Path::new(iso_path)
            .file_name()
            .ok_or(InspectError::InvalidDiskInfo {
                reason: "invalid ISO path",
            })?;

        let destination = temp_directory.path().join(filename);

        let output = Command::new(&self.command)
            .arg("-osirrox")
            .arg("on")
            .arg("-indev")
            .arg(&self.iso)
            .arg("-extract")
            .arg(iso_path)
            .arg(&destination)
            .output()?;

        if !output.status.success() {
            return Err(InspectError::ProcessFailed {
                command: self.command.display().to_string(),
                status: output.status,
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            });
        }

        Ok(std::fs::read(destination)?)
    }
    fn path_exists(&self, iso_path: &str) -> Result<bool, InspectError> {
        let output = Command::new(&self.command)
            .arg("-indev")
            .arg(&self.iso)
            .arg("-find")
            .arg("/")
            .arg("-wholename")
            .arg(iso_path)
            .arg("-exec")
            .arg("echo")
            .arg("--")
            .output()?;

        if !output.status.success() {
            return Err(InspectError::ProcessFailed {
                command: self.command.display().to_string(),
                status: output.status,
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            });
        }

        Ok(!output.stdout.is_empty())
    }
    fn list_files(&self, iso_path: &str) -> Result<Vec<String>, InspectError> {
        let output = Command::new(&self.command)
            .arg("-indev")
            .arg(&self.iso)
            .arg("-find")
            .arg(iso_path)
            .arg("-mindepth")
            .arg("1")
            .arg("-maxdepth")
            .arg("1")
            .arg("-exec")
            .arg("echo")
            .arg("--")
            .output()?;

        if !output.status.success() {
            return Err(InspectError::ProcessFailed {
                command: self.command.display().to_string(),
                status: output.status,
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            });
        }

        let stdout = std::str::from_utf8(&output.stdout)?;

        Ok(stdout
            .lines()
            .map(str::trim)
            .filter(|path| !path.is_empty())
            .map(str::to_owned)
            .collect())
    }
}
#[cfg(all(test, unix))]
mod tests {
    use std::{
        fs::{self, File},
        io::Write,
        os::unix::fs::PermissionsExt,
        path::{Path, PathBuf},
        thread,
        time::Duration,
    };
    use tempfile::tempdir;

    use super::*;

    fn create_command(directory: &Path, name: &str, body: &str) -> PathBuf {
        let command = directory.join(name);
        let temporary_command = directory.join(format!("{name}.tmp"));

        let mut file =
            File::create(&temporary_command).expect("temporary mock command should be created");

        writeln!(file, "#!/bin/sh\n{body}").expect("mock xorriso command should be written");

        file.sync_all()
            .expect("mock xorriso command should be synchronized");

        drop(file);

        let mut permissions = fs::metadata(&temporary_command)
            .expect("temporary mock command metadata should be available")
            .permissions();

        permissions.set_mode(0o755);

        fs::set_permissions(&temporary_command, permissions)
            .expect("temporary mock command should be executable");

        fs::rename(&temporary_command, &command)
            .expect("temporary mock command should be installed");
        thread::sleep(Duration::from_millis(10));
        command
    }

    #[test]
    fn path_exists_returns_true_when_xorriso_prints_a_match() {
        let directory = tempdir().expect("temporary directory should be created");
        let command = create_command(
            directory.path(),
            "xorriso-match",
            "printf '%s\\n' '/isolinux/isolinux.bin'",
        );

        let reader = XorrisoReader::new("debian.iso").with_command(command);

        let exists = reader
            .path_exists("/isolinux/isolinux.bin")
            .expect("path lookup should succeed");

        assert!(exists);
    }

    #[test]
    fn path_exists_returns_false_when_xorriso_prints_no_match() {
        let directory = tempdir().expect("temporary directory should be created");
        let command = create_command(directory.path(), "xorriso-no-match", "exit 0");

        let reader = XorrisoReader::new("debian.iso").with_command(command);

        let exists = reader
            .path_exists("/isolinux/isolinux.bin")
            .expect("path lookup should succeed");

        assert!(!exists);
    }
    #[test]
    fn list_files_returns_directory_entries() {
        let directory = tempdir().expect("temporary directory should be created");

        let command = create_command(
            directory.path(),
            "xorriso-list",
            "printf '%s\\n' '/dists' '/pool' '/README.html'",
        );

        let reader = XorrisoReader::new("debian.iso").with_command(command);

        let entries = reader
            .list_files("/")
            .expect("directory listing should succeed");

        assert_eq!(
            entries,
            vec![
                "/dists".to_owned(),
                "/pool".to_owned(),
                "/README.html".to_owned(),
            ]
        );
    }

    #[test]
    fn list_files_returns_empty_when_directory_has_no_entries() {
        let directory = tempdir().expect("temporary directory should be created");

        let command = create_command(directory.path(), "xorriso-empty", "exit 0");

        let reader = XorrisoReader::new("debian.iso").with_command(command);

        let entries = reader
            .list_files("/")
            .expect("directory listing should succeed");

        assert!(entries.is_empty());
    }
}
