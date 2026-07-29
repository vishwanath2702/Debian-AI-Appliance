//! Shared appliance build context.

use std::path::{Path, PathBuf};

/// Immutable paths and inputs shared across an appliance build.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BuildContext {
    rootfs: PathBuf,
    source_iso: PathBuf,
    work_directory: PathBuf,
    output_iso: PathBuf,
}

impl BuildContext {
    /// Creates a build context.
    #[must_use]
    pub fn new(
        rootfs: impl Into<PathBuf>,
        source_iso: impl Into<PathBuf>,
        work_directory: impl Into<PathBuf>,
        output_iso: impl Into<PathBuf>,
    ) -> Self {
        Self {
            rootfs: rootfs.into(),
            source_iso: source_iso.into(),
            work_directory: work_directory.into(),
            output_iso: output_iso.into(),
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
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::BuildContext;

    #[test]
    fn exposes_build_paths() {
        let context = BuildContext::new(
            "build/rootfs",
            "images/source.iso",
            "build/work",
            "build/output.iso",
        );

        assert_eq!(context.rootfs(), Path::new("build/rootfs"));
        assert_eq!(context.source_iso(), Path::new("images/source.iso"));
        assert_eq!(context.work_directory(), Path::new("build/work"));
        assert_eq!(context.output_iso(), Path::new("build/output.iso"));
    }
}
