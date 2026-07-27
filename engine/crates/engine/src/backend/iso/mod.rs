//! ISO build backend.

mod layout;

pub use layout::Layout;

use std::path::{Path, PathBuf};

/// Backend responsible for producing a bootable ISO image.
pub struct IsoBackend {
    rootfs: PathBuf,
    work_directory: PathBuf,
    output_path: PathBuf,
}

impl IsoBackend {
    /// Creates an ISO backend.
    #[must_use]
    pub fn layout(&self) -> Layout {
        Layout::new(self.work_directory.join("iso"))
    }
    #[must_use]
    pub fn new(
        rootfs: impl Into<PathBuf>,
        work_directory: impl Into<PathBuf>,
        output_path: impl Into<PathBuf>,
    ) -> Self {
        Self {
            rootfs: rootfs.into(),
            work_directory: work_directory.into(),
            output_path: output_path.into(),
        }
    }

    /// Returns the prepared root filesystem path.
    #[must_use]
    pub fn rootfs(&self) -> &Path {
        &self.rootfs
    }

    /// Returns the ISO working-directory path.
    #[must_use]
    pub fn work_directory(&self) -> &Path {
        &self.work_directory
    }

    /// Returns the final ISO output path.
    #[must_use]
    pub fn output_path(&self) -> &Path {
        &self.output_path
    }
}
