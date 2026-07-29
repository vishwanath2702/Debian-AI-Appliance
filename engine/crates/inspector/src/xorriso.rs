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
}
