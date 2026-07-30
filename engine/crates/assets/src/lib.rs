//! Asset storage abstractions for DAIA providers.

use std::{
    fs, io,
    path::{Path, PathBuf},
};

/// Reads provider-owned assets by relative path.
pub trait AssetStore {
    /// Reads an asset and returns its contents.
    ///
    /// # Errors
    ///
    /// Returns an I/O error when the asset cannot be read.
    fn read(&self, path: &Path) -> io::Result<Vec<u8>>;
}

/// Asset store backed by a directory on the local filesystem.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FilesystemAssetStore {
    root: PathBuf,
}

impl FilesystemAssetStore {
    /// Creates a filesystem asset store rooted at the supplied directory.
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Returns the directory containing this store's assets.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }
}

impl AssetStore for FilesystemAssetStore {
    fn read(&self, path: &Path) -> io::Result<Vec<u8>> {
        fs::read(self.root.join(path))
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, path::Path};

    use super::{AssetStore, FilesystemAssetStore};

    #[test]
    fn exposes_asset_root() {
        let store = FilesystemAssetStore::new("/tmp/provider-assets");

        assert_eq!(store.root(), Path::new("/tmp/provider-assets"));
    }

    #[test]
    fn reads_asset_relative_to_root() {
        let directory = tempfile::tempdir().expect("temporary directory should exist");
        let asset_path = directory.path().join("files").join("motd");

        fs::create_dir_all(
            asset_path
                .parent()
                .expect("asset path should have a parent directory"),
        )
        .expect("asset directory should be created");

        fs::write(&asset_path, b"Welcome to DAIA\n").expect("asset should be written");

        let store = FilesystemAssetStore::new(directory.path());
        let contents = store
            .read(Path::new("files/motd"))
            .expect("asset should be readable");

        assert_eq!(contents, b"Welcome to DAIA\n");
    }

    #[test]
    fn missing_asset_returns_io_error() {
        let directory = tempfile::tempdir().expect("temporary directory should exist");
        let store = FilesystemAssetStore::new(directory.path());

        let error = store
            .read(Path::new("files/missing"))
            .expect_err("missing asset should fail");

        assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
    }
}
