// engine/crates/inspector/src/lib.rs

//! Inspection and identification of Debian installation media.

mod error;
mod reader;
mod xorriso;

use std::path::{Path, PathBuf};

pub use error::InspectError;
pub use reader::IsoReader;
pub use xorriso::XorrisoReader;

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

/// Inspects installation ISO metadata.
pub trait IsoInspector {
    /// Inspects the supplied ISO path.
    ///
    /// # Errors
    ///
    /// Returns an error when the ISO cannot be read or identified.
    fn inspect(&self, path: &Path) -> Result<IsoMetadata, InspectError>;
}
