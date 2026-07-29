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
        let output = Command::new(&self.command)
            .arg("-osirrox")
            .arg("on")
            .arg("-indev")
            .arg(&self.iso)
            .arg("-extract")
            .arg(iso_path)
            .arg("-")
            .output()?;

        if !output.status.success() {
            return Err(InspectError::ProcessFailed {
                command: self.command.display().to_string(),
                status: output.status,
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            });
        }

        Ok(output.stdout)
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
}
#[cfg(all(test, unix))]
mod tests {
    use std::{
        fs,
        os::unix::fs::PermissionsExt,
        path::{Path, PathBuf},
    };

    use tempfile::tempdir;

    use super::*;

    fn create_command(directory: &Path, name: &str, body: &str) -> PathBuf {
        let command = directory.join(name);

        fs::write(&command, format!("#!/bin/sh\n{body}\n"))
            .expect("mock xorriso command should be written");

        let mut permissions = fs::metadata(&command)
            .expect("mock xorriso metadata should be available")
            .permissions();

        permissions.set_mode(0o755);

        fs::set_permissions(&command, permissions)
            .expect("mock xorriso command should be executable");

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
}
