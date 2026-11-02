//! Immutable source snapshots for privileged exporter installation.
//!
//! Creating a sealed snapshot does not authenticate the source or authorize
//! privileged installation. Those checks belong to separate boundaries.

#[cfg(target_os = "linux")]
pub(crate) fn sealed_export_install_snapshot(
    source: &mut std::fs::File,
) -> std::io::Result<std::os::fd::OwnedFd> {
    use rustix::fs::{MemfdFlags, SealFlags};
    use std::io::{Read, Seek, SeekFrom, Write};
    use std::os::fd::AsFd;
    use std::os::unix::fs::MetadataExt;

    let metadata = source.metadata()?;
    if !metadata.is_file() {
        return Err(std::io::Error::other(
            "exporter snapshot source must be a regular file",
        ));
    }

    const MAX_SOURCE_BYTES: u64 = 128 * 1024 * 1024;

    if metadata.len() == 0 || metadata.len() > MAX_SOURCE_BYTES {
        return Err(std::io::Error::other(
            "exporter snapshot source size is invalid",
        ));
    }

    let descriptor = rustix::fs::memfd_create(
        "daia-export-install-snapshot",
        MemfdFlags::CLOEXEC | MemfdFlags::ALLOW_SEALING,
    )?;

    let mut snapshot = std::fs::File::from(descriptor);
    source.seek(SeekFrom::Start(0))?;

    let mut limited = source.take(MAX_SOURCE_BYTES + 1);
    let copied = std::io::copy(&mut limited, &mut snapshot)?;

    if copied != metadata.len() || copied > MAX_SOURCE_BYTES {
        return Err(std::io::Error::other(
            "exporter snapshot source changed during copying",
        ));
    }

    snapshot.flush()?;

    let after = source.metadata()?;
    if metadata.dev() != after.dev()
        || metadata.ino() != after.ino()
        || metadata.len() != after.len()
        || metadata.mtime() != after.mtime()
        || metadata.mtime_nsec() != after.mtime_nsec()
        || metadata.ctime() != after.ctime()
        || metadata.ctime_nsec() != after.ctime_nsec()
    {
        return Err(std::io::Error::other(
            "exporter snapshot source metadata changed during copying",
        ));
    }

    let required = SealFlags::WRITE | SealFlags::GROW | SealFlags::SHRINK | SealFlags::SEAL;

    rustix::fs::fcntl_add_seals(snapshot.as_fd(), required)?;

    let actual = rustix::fs::fcntl_get_seals(snapshot.as_fd())?;

    if !actual.contains(required) {
        return Err(std::io::Error::other(
            "exporter snapshot is not fully sealed",
        ));
    }

    snapshot.seek(SeekFrom::Start(0))?;
    Ok(snapshot.into())
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::sealed_export_install_snapshot;
    use rustix::fs::SealFlags;
    use std::io::{Read, Seek, SeekFrom, Write};

    #[test]
    fn snapshot_is_immutable_and_preserves_contents() {
        let mut source = tempfile::tempfile().unwrap();
        source.write_all(b"exporter-test-binary").unwrap();
        source.seek(SeekFrom::Start(0)).unwrap();

        let snapshot = sealed_export_install_snapshot(&mut source).unwrap();
        let required = SealFlags::WRITE | SealFlags::GROW | SealFlags::SHRINK | SealFlags::SEAL;

        assert!(
            rustix::fs::fcntl_get_seals(&snapshot)
                .unwrap()
                .contains(required)
        );

        let mut file = std::fs::File::from(snapshot);
        let mut contents = Vec::new();
        file.read_to_end(&mut contents).unwrap();
        assert_eq!(contents, b"exporter-test-binary");

        assert!(file.write_all(b"modified").is_err());
    }

    #[test]
    fn snapshot_remains_unchanged_after_source_modification() {
        let mut source = tempfile::tempfile().unwrap();
        source.write_all(b"original").unwrap();

        let snapshot = sealed_export_install_snapshot(&mut source).unwrap();

        source.seek(SeekFrom::Start(0)).unwrap();
        source.write_all(b"modified").unwrap();

        let mut sealed = std::fs::File::from(snapshot);
        let mut contents = Vec::new();
        sealed.read_to_end(&mut contents).unwrap();

        assert_eq!(contents, b"original");
    }

    #[test]
    fn snapshot_rejects_empty_source() {
        let mut source = tempfile::tempfile().unwrap();
        assert!(sealed_export_install_snapshot(&mut source).is_err());
    }

    #[test]
    fn snapshot_rejects_oversized_source() {
        let mut source = tempfile::tempfile().unwrap();
        source.set_len(128 * 1024 * 1024 + 1).unwrap();
        assert!(sealed_export_install_snapshot(&mut source).is_err());
    }
}
