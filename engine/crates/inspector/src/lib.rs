//! Inspection and identification of Debian installation media.

mod debian;
mod error;
mod reader;
mod storage_error;
mod xorriso;

use std::path::{Path, PathBuf};

use model::DiscoveredStorage;

pub use debian::{DebianIsoInspector, parse_disk_info};
pub use error::InspectError;
pub use reader::IsoReader;
pub use storage_error::StorageInspectError;
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
    repositories: Vec<RepositoryInfo>,
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
            repositories: Vec::new(),
        }
    }
    pub(crate) fn set_path(&mut self, path: PathBuf) {
        self.path = path;
    }
    pub(crate) fn set_boot_modes(&mut self, boot_modes: Vec<BootMode>) {
        self.boot_modes = boot_modes;
    }
    pub(crate) fn set_repositories(&mut self, repositories: Vec<RepositoryInfo>) {
        self.repositories = repositories;
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
    /// Returns the Debian repositories discovered on the ISO.
    #[must_use]
    pub fn repositories(&self) -> &[RepositoryInfo] {
        &self.repositories
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

/// Debian repository discovered inside an installation ISO.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepositoryInfo {
    suite: String,
    components: Vec<String>,
    architectures: Vec<String>,
    indexes: Vec<String>,
}

impl RepositoryInfo {
    /// Creates repository information.
    #[must_use]
    pub const fn new(
        suite: String,
        components: Vec<String>,
        architectures: Vec<String>,
        indexes: Vec<String>,
    ) -> Self {
        Self {
            suite,
            components,
            architectures,
            indexes,
        }
    }

    /// Returns the repository suite.
    #[must_use]
    pub fn suite(&self) -> &str {
        &self.suite
    }

    /// Returns the repository components.
    #[must_use]
    pub fn components(&self) -> &[String] {
        &self.components
    }

    /// Returns the repository architectures.
    #[must_use]
    pub fn architectures(&self) -> &[String] {
        &self.architectures
    }

    /// Returns the repository index paths.
    #[must_use]
    pub fn indexes(&self) -> &[String] {
        &self.indexes
    }
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

/// Discovers storage available to DAIA.
pub trait StorageInspector {
    /// Discovers storage currently available on the system.
    ///
    /// # Errors
    ///
    /// Returns an error when system storage cannot be inspected.
    fn inspect(&self) -> Result<Vec<DiscoveredStorage>, StorageInspectError>;
}

#[cfg(test)]
mod tests {
    use super::{BootMode, IsoMetadata, RepositoryInfo};
    use std::path::{Path, PathBuf};

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
        assert!(metadata.repositories().is_empty());
    }
    #[test]
    fn repository_info_exposes_discovered_values() {
        let repository = RepositoryInfo::new(
            "trixie".to_owned(),
            vec![
                "main".to_owned(),
                "contrib".to_owned(),
                "non-free-firmware".to_owned(),
            ],
            vec!["amd64".to_owned(), "arm64".to_owned()],
            vec![
                "/dists/trixie/main/binary-amd64/Packages.xz".to_owned(),
                "/dists/trixie/main/source/Sources.xz".to_owned(),
            ],
        );

        assert_eq!(repository.suite(), "trixie");
        assert_eq!(
            repository.components(),
            &[
                "main".to_owned(),
                "contrib".to_owned(),
                "non-free-firmware".to_owned(),
            ]
        );
        assert_eq!(
            repository.architectures(),
            &["amd64".to_owned(), "arm64".to_owned()]
        );
        assert_eq!(
            repository.indexes(),
            &[
                "/dists/trixie/main/binary-amd64/Packages.xz".to_owned(),
                "/dists/trixie/main/source/Sources.xz".to_owned(),
            ]
        );
    }
}
