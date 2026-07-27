//! ISO workspace layout.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Directory layout used while building an ISO.
pub struct Layout {
    root: PathBuf,
}

impl Layout {
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn create(&self) -> io::Result<()> {
        fs::create_dir_all(self.boot_grub())?;
        fs::create_dir_all(self.efi_boot())?;
        fs::create_dir_all(self.live())?;
        fs::create_dir_all(self.staging())?;
        Ok(())
    }

    #[must_use]
    pub fn boot_grub(&self) -> PathBuf {
        self.root.join("boot").join("grub")
    }

    #[must_use]
    pub fn efi_boot(&self) -> PathBuf {
        self.root.join("EFI").join("BOOT")
    }

    #[must_use]
    pub fn live(&self) -> PathBuf {
        self.root.join("live")
    }

    #[must_use]
    pub fn staging(&self) -> PathBuf {
        self.root.join("staging")
    }
    #[must_use]
    pub fn filesystem_squashfs(&self) -> PathBuf {
        self.live().join("filesystem.squashfs")
    }
}
#[cfg(test)]
mod tests {
    use super::Layout;

    #[test]
    fn layout_builds_expected_paths() {
        let layout = Layout::new("/tmp/work/iso");

        assert_eq!(layout.root(), std::path::Path::new("/tmp/work/iso"));
        assert_eq!(
            layout.boot_grub(),
            std::path::Path::new("/tmp/work/iso/boot/grub")
        );
        assert_eq!(
            layout.efi_boot(),
            std::path::Path::new("/tmp/work/iso/EFI/BOOT")
        );
        assert_eq!(layout.live(), std::path::Path::new("/tmp/work/iso/live"));
        assert_eq!(
            layout.staging(),
            std::path::Path::new("/tmp/work/iso/staging")
        );
        assert_eq!(
            layout.filesystem_squashfs(),
            std::path::Path::new("/tmp/work/iso/live/filesystem.squashfs")
        );
    }
}
#[test]
fn create_builds_workspace_layout() {
    let temp = tempfile::tempdir().unwrap();

    let layout = Layout::new(temp.path());

    layout.create().unwrap();

    assert!(layout.boot_grub().is_dir());
    assert!(layout.efi_boot().is_dir());
    assert!(layout.live().is_dir());
    assert!(layout.staging().is_dir());
}
