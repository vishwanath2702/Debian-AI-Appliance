//! Shared appliance build context.

use std::path::{Path, PathBuf};

use crate::BootstrapConfig;

/// Immutable paths and inputs shared across an appliance build.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BuildContext {
    rootfs: PathBuf,
    source_iso: PathBuf,
    work_directory: PathBuf,
    output_iso: PathBuf,
    bootstrap: BootstrapConfig,
}

impl BuildContext {
    /// Creates a build context.
    #[must_use]
    pub fn new(
        rootfs: impl Into<PathBuf>,
        source_iso: impl Into<PathBuf>,
        work_directory: impl Into<PathBuf>,
        output_iso: impl Into<PathBuf>,
        bootstrap: BootstrapConfig,
    ) -> Self {
        Self {
            rootfs: rootfs.into(),
            source_iso: source_iso.into(),
            work_directory: work_directory.into(),
            output_iso: output_iso.into(),
            bootstrap,
        }
    }

    /// Returns the prepared root filesystem path.
    #[must_use]
    pub fn rootfs(&self) -> &Path {
        &self.rootfs
    }

    /// Returns the source ISO path.
    #[must_use]
    pub fn source_iso(&self) -> &Path {
        &self.source_iso
    }

    /// Returns the build working-directory path.
    #[must_use]
    pub fn work_directory(&self) -> &Path {
        &self.work_directory
    }

    /// Returns the final ISO output path.
    #[must_use]
    pub fn output_iso(&self) -> &Path {
        &self.output_iso
    }

    /// Returns the root filesystem bootstrap configuration.
    #[must_use]
    pub const fn bootstrap(&self) -> &BootstrapConfig {
        &self.bootstrap
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::BuildContext;
    use crate::BootstrapConfig;

    #[test]
    fn exposes_build_inputs() {
        let bootstrap = BootstrapConfig::new(
            "bookworm",
            "amd64",
            "https://deb.debian.org/debian",
            vec!["main".to_owned()],
            "minbase",
        );

        let context = BuildContext::new(
            "build/rootfs",
            "images/source.iso",
            "build/work",
            "build/output.iso",
            bootstrap.clone(),
        );

        assert_eq!(context.rootfs(), Path::new("build/rootfs"));
        assert_eq!(context.source_iso(), Path::new("images/source.iso"));
        assert_eq!(context.work_directory(), Path::new("build/work"));
        assert_eq!(context.output_iso(), Path::new("build/output.iso"));
        assert_eq!(context.bootstrap(), &bootstrap);
    }
}
