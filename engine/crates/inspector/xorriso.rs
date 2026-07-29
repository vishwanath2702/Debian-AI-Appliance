// engine/crates/inspector/src/xorriso.rs

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

    /// Returns the ISO image read by this instance.
    #[must_use]
    pub fn iso(&self) -> &Path {
        &self.iso
    }

    /// Returns the configured `xorriso` executable.
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

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::Path,
    };

    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    use tempfile::TempDir;

    use super::XorrisoReader;
    use crate::{InspectError, IsoReader};

    #[cfg(unix)]
    fn create_executable(path: &Path, contents: &str) {
        fs::write(path, contents).expect("fake executable should be written");

        let mut permissions = fs::metadata(path)
            .expect("fake executable metadata should be readable")
            .permissions();

        permissions.set_mode(0o755);

        fs::set_permissions(path, permissions)
            .expect("fake executable should be made executable");
    }

    #[cfg(unix)]
    #[test]
    fn reads_file_contents_from_standard_output() {
        let temporary_directory =
            TempDir::new().expect("temporary directory should be created");

        let iso = temporary_directory.path().join("source.iso");
        let command = temporary_directory.path().join("xorriso");

        fs::write(&iso, []).expect("placeholder ISO should be created");

        create_executable(
            &command,
            concat!(
                "#!/usr/bin/env bash\n",
                "set -euo pipefail\n",
                "printf 'Debian GNU/Linux 13.1.0 \"Trixie\"'\n",
            ),
        );

        let reader = XorrisoReader::new(&iso).with_command(&command);

        let contents = reader
            .read_file("/.disk/info")
            .expect("ISO file should be read");

        assert_eq!(
            contents,
            b"Debian GNU/Linux 13.1.0 \"Trixie\""
        );
        assert_eq!(reader.iso(), iso);
        assert_eq!(reader.command(), command);
    }

    #[cfg(unix)]
    #[test]
    fn returns_process_error_for_unsuccessful_command() {
        let temporary_directory =
            TempDir::new().expect("temporary directory should be created");

        let iso = temporary_directory.path().join("source.iso");
        let command = temporary_directory.path().join("xorriso");

        fs::write(&iso, []).expect("placeholder ISO should be created");

        create_executable(
            &command,
            concat!(
                "#!/usr/bin/env bash\n",
                "set -euo pipefail\n",
                "printf 'requested ISO file was not found\\n' >&2\n",
                "exit 4\n",
            ),
        );

        let reader = XorrisoReader::new(iso).with_command(&command);

        let error = reader
            .read_file("/.disk/info")
            .expect_err("unsuccessful command should return an error");

        match error {
            InspectError::ProcessFailed {
                command: failed_command,
                status,
                stderr,
            } => {
                assert_eq!(failed_command, command.display().to_string());
                assert_eq!(status.code(), Some(4));
                assert_eq!(stderr, "requested ISO file was not found\n");
            }
            InspectError::Io(error) => {
                panic!("unexpected I/O error: {error}");
            }
        }
    }

    #[test]
    fn returns_io_error_when_command_cannot_be_started() {
        let temporary_directory =
            TempDir::new().expect("temporary directory should be created");

        let missing_command = temporary_directory.path().join("missing-xorriso");

        let reader = XorrisoReader::new(
            temporary_directory.path().join("source.iso"),
        )
        .with_command(missing_command);

        let error = reader
            .read_file("/.disk/info")
            .expect_err("missing executable should return an error");

        assert!(matches!(error, InspectError::Io(_)));
    }
}
