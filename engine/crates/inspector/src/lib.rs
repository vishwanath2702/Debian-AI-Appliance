//! Inspection and identification of Debian installation media.

use std::path::{Path, PathBuf};

/// Metadata discovered from an installation ISO.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IsoMetadata {
    path: PathBuf,
    distribution: String,
    version: String,
    codename: String,
    architecture: String,
    media_type: String,
    boot_modes: Vec<BootMode>,
}

impl IsoMetadata {
    /// Creates discovered ISO metadata.
    #[must_use]
    pub const fn new(
        path: PathBuf,
        distribution: String,
        version: String,
        codename: String,
        architecture: String,
        media_type: String,
        boot_modes: Vec<BootMode>,
    ) -> Self {
        Self {
            path,
            distribution,
            version,
            codename,
            architecture,
            media_type,
            boot_modes,
        }
    }

    /// Returns the inspected ISO path.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Returns the distribution name.
    #[must_use]
    pub fn distribution(&self) -> &str {
        &self.distribution
    }

    /// Returns the distribution version.
    #[must_use]
    pub fn version(&self) -> &str {
        &self.version
    }

    /// Returns the distribution codename.
    #[must_use]
    pub fn codename(&self) -> &str {
        &self.codename
    }

    /// Returns the target architecture.
    #[must_use]
    pub fn architecture(&self) -> &str {
        &self.architecture
    }

    /// Returns the installation-media type.
    #[must_use]
    pub fn media_type(&self) -> &str {
        &self.media_type
    }

    /// Returns the boot modes supported by the ISO.
    #[must_use]
    pub fn boot_modes(&self) -> &[BootMode] {
        &self.boot_modes
    }
}

/// Boot mechanism supported by an installation ISO.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BootMode {
    /// Legacy PC BIOS boot.
    Bios,

    /// Unified Extensible Firmware Interface boot.
    Uefi,
}

/// Error returned when ISO inspection cannot be completed.
#[derive(Debug)]
pub enum InspectError {
    /// ISO inspection has not yet been implemented.
    Unsupported(PathBuf),
}

/// Inspects installation ISO metadata.
pub trait IsoInspector {
    /// Inspects the supplied ISO path.
    ///
    /// # Errors
    ///
    /// Returns an error when the ISO cannot be read or identified.
    fn inspect(&self, path: &Path) -> Result<IsoMetadata, InspectError>;
}

/// Placeholder inspector used while real Debian ISO parsing is introduced.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DebianIsoInspector;

impl IsoInspector for DebianIsoInspector {
    fn inspect(&self, path: &Path) -> Result<IsoMetadata, InspectError> {
        Err(InspectError::Unsupported(path.to_path_buf()))
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::{BootMode, DebianIsoInspector, InspectError, IsoInspector, IsoMetadata};

    #[test]
    fn iso_metadata_exposes_discovered_values() {
        let metadata = IsoMetadata::new(
            PathBuf::from("/tmp/debian.iso"),
            "Debian".to_owned(),
            "13.1.0".to_owned(),
            "trixie".to_owned(),
            "amd64".to_owned(),
            "netinst".to_owned(),
            vec![BootMode::Bios, BootMode::Uefi],
        );

        assert_eq!(metadata.path(), Path::new("/tmp/debian.iso"));
        assert_eq!(metadata.distribution(), "Debian");
        assert_eq!(metadata.version(), "13.1.0");
        assert_eq!(metadata.codename(), "trixie");
        assert_eq!(metadata.architecture(), "amd64");
        assert_eq!(metadata.media_type(), "netinst");
        assert_eq!(metadata.boot_modes(), &[BootMode::Bios, BootMode::Uefi]);
    }

    #[test]
    fn placeholder_inspector_returns_unsupported_error() {
        let path = Path::new("/tmp/debian.iso");

        let error = DebianIsoInspector
            .inspect(path)
            .expect_err("placeholder inspection should fail");

        match error {
            InspectError::Unsupported(unsupported_path) => {
                assert_eq!(unsupported_path, path);
            }
        }
    }
}
