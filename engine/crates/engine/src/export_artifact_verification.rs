//! Descriptor-relative verification of declared export artifacts.
//!
//! This module does not authorize exports, enumerate unexpected entries,
//! establish staging isolation, or publish verified content.

#[cfg(target_os = "linux")]
pub fn verify_declared_export_artifacts(
    directory: &std::os::fd::OwnedFd,
    operation: &model::ModelExportOperation,
) -> std::io::Result<()> {
    use rustix::fs::{FileType, Mode, OFlags, ResolveFlags};
    use sha2::{Digest, Sha256};
    use std::io::{Error, ErrorKind, Read};

    let root = rustix::fs::fstat(directory)?;

    if FileType::from_raw_mode(root.st_mode) != FileType::Directory {
        return Err(Error::new(
            ErrorKind::InvalidData,
            "export root is not a directory",
        ));
    }

    for artifact in operation.artifacts() {
        let descriptor = rustix::fs::openat2(
            directory,
            artifact.relative_path(),
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
            Mode::empty(),
            ResolveFlags::BENEATH | ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_XDEV,
        )?;

        let metadata = rustix::fs::fstat(&descriptor)?;

        if FileType::from_raw_mode(metadata.st_mode) != FileType::RegularFile
            || metadata.st_dev != root.st_dev
            || metadata.st_nlink != 1
            || metadata.st_size < 0
            || metadata.st_size as u64 != artifact.size_bytes()
        {
            return Err(Error::new(
                ErrorKind::InvalidData,
                "export artifact metadata mismatch",
            ));
        }

        let mut reader = std::fs::File::from(descriptor);
        let mut hasher = Sha256::new();
        let mut buffer = [0_u8; 64 * 1024];
        let mut total = 0_u64;

        loop {
            let count = match reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(count) => count,
                Err(error) if error.kind() == ErrorKind::Interrupted => continue,
                Err(error) => return Err(error),
            };

            total = total
                .checked_add(count as u64)
                .ok_or_else(|| Error::new(ErrorKind::InvalidData, "artifact size overflow"))?;

            if total > artifact.size_bytes() {
                return Err(Error::new(
                    ErrorKind::InvalidData,
                    "artifact exceeds expected size",
                ));
            }

            hasher.update(&buffer[..count]);
        }

        if total != artifact.size_bytes() || format!("{:x}", hasher.finalize()) != artifact.sha256()
        {
            return Err(Error::new(
                ErrorKind::InvalidData,
                "artifact digest or size mismatch",
            ));
        }
    }

    verify_export_tree_entries(directory, operation)?;

    Ok(())
}

/// Checks the directory-entry structure against the declared artifact set.
///
/// This does not verify file content or establish exclusive staging access.
#[cfg(target_os = "linux")]
fn verify_export_tree_entries(
    directory: &std::os::fd::OwnedFd,
    operation: &model::ModelExportOperation,
) -> std::io::Result<()> {
    use rustix::fs::{FileType, Mode, OFlags, RawDir, ResolveFlags};
    use std::collections::HashSet;
    use std::io::{Error, ErrorKind};
    use std::mem::MaybeUninit;

    let root = rustix::fs::fstat(directory)?;

    if FileType::from_raw_mode(root.st_mode) != FileType::Directory {
        return Err(Error::new(
            ErrorKind::InvalidData,
            "export root is not a directory",
        ));
    }

    let mut files = HashSet::new();
    let mut directories = HashSet::new();

    for artifact in operation.artifacts() {
        let path = artifact.relative_path();
        files.insert(path.to_owned());

        for (index, byte) in path.bytes().enumerate() {
            if byte == b'/' {
                directories.insert(path[..index].to_owned());
            }
        }
    }

    fn inspect(
        directory: &std::os::fd::OwnedFd,
        prefix: &str,
        root_device: u64,
        files: &HashSet<String>,
        directories: &HashSet<String>,
        seen_files: &mut HashSet<String>,
        seen_directories: &mut HashSet<String>,
    ) -> std::io::Result<()> {
        let mut buffer = [MaybeUninit::<u8>::uninit(); 8192];
        let mut entries = RawDir::new(directory, &mut buffer);

        while let Some(result) = entries.next() {
            let entry = result?;
            let name = entry.file_name();
            let bytes = name.to_bytes();

            if bytes == b"." || bytes == b".." {
                continue;
            }

            let component = std::str::from_utf8(bytes)
                .map_err(|_| Error::new(ErrorKind::InvalidData, "non-UTF8 export entry"))?;

            let relative = if prefix.is_empty() {
                component.to_owned()
            } else {
                format!("{prefix}/{component}")
            };

            if directories.contains(&relative) {
                let child = rustix::fs::openat2(
                    directory,
                    name,
                    OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
                    Mode::empty(),
                    ResolveFlags::BENEATH | ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_XDEV,
                )?;

                let metadata = rustix::fs::fstat(&child)?;

                if FileType::from_raw_mode(metadata.st_mode) != FileType::Directory
                    || metadata.st_dev != root_device
                {
                    return Err(Error::new(
                        ErrorKind::InvalidData,
                        "unsafe export directory",
                    ));
                }

                seen_directories.insert(relative.clone());
                inspect(
                    &child,
                    &relative,
                    root_device,
                    files,
                    directories,
                    seen_files,
                    seen_directories,
                )?;
            } else if files.contains(&relative) {
                let file = rustix::fs::openat2(
                    directory,
                    name,
                    OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
                    Mode::empty(),
                    ResolveFlags::BENEATH | ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_XDEV,
                )?;

                let metadata = rustix::fs::fstat(&file)?;

                if FileType::from_raw_mode(metadata.st_mode) != FileType::RegularFile
                    || metadata.st_dev != root_device
                    || metadata.st_nlink != 1
                {
                    return Err(Error::new(ErrorKind::InvalidData, "unsafe export file"));
                }
                seen_files.insert(relative);
            } else {
                return Err(Error::new(
                    ErrorKind::InvalidData,
                    "unexpected export entry",
                ));
            }
        }

        Ok(())
    }

    let mut seen_files = HashSet::new();
    let mut seen_directories = HashSet::new();

    inspect(
        directory,
        "",
        root.st_dev,
        &files,
        &directories,
        &mut seen_files,
        &mut seen_directories,
    )?;

    if seen_files != files || seen_directories != directories {
        return Err(Error::new(
            ErrorKind::InvalidData,
            "missing declared export entries",
        ));
    }

    Ok(())
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::verify_declared_export_artifacts;
    use model::{
        ModelExportArtifact, ModelExportOperation, ModelExportOperationId, ModelExportSource,
    };
    use rustix::fs::{Mode, OFlags};
    use sha2::{Digest, Sha256};
    use std::fs;
    use std::os::fd::OwnedFd;
    use std::os::unix::fs::symlink;

    fn operation(
        path: &str,
        bytes: &[u8],
        size: u64,
        override_hash: Option<String>,
    ) -> ModelExportOperation {
        let hash = override_hash.unwrap_or_else(|| format!("{:x}", Sha256::digest(bytes)));

        ModelExportOperation::new(
            ModelExportOperationId::new("verification-test").unwrap(),
            ModelExportSource::Gguf,
            "verified-export",
            vec![ModelExportArtifact::new(path, hash, size).unwrap()],
        )
        .unwrap()
    }

    fn open_directory(path: &std::path::Path) -> OwnedFd {
        rustix::fs::open(
            path,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .unwrap()
    }

    #[test]
    fn accepts_deeply_nested_declared_artifact() {
        let root = tempfile::tempdir().unwrap();
        let nested = root.path().join("models/ollama/blobs");

        fs::create_dir_all(&nested).unwrap();
        fs::write(nested.join("model.gguf"), b"model-data").unwrap();

        let fd = open_directory(root.path());
        let request = operation("models/ollama/blobs/model.gguf", b"model-data", 10, None);

        verify_declared_export_artifacts(&fd, &request).unwrap();
    }

    #[test]
    fn rejects_unexpected_file_inside_declared_directory() {
        let root = tempfile::tempdir().unwrap();
        let nested = root.path().join("models/ollama");

        fs::create_dir_all(&nested).unwrap();
        fs::write(nested.join("model.gguf"), b"model-data").unwrap();
        fs::write(nested.join("unexpected.txt"), b"extra").unwrap();

        let fd = open_directory(root.path());
        let request = operation("models/ollama/model.gguf", b"model-data", 10, None);

        assert!(verify_declared_export_artifacts(&fd, &request).is_err());
    }

    #[test]
    fn rejects_unexpected_nested_directory() {
        let root = tempfile::tempdir().unwrap();
        let nested = root.path().join("models/ollama");

        fs::create_dir_all(nested.join("unexpected")).unwrap();
        fs::write(nested.join("model.gguf"), b"model-data").unwrap();

        let fd = open_directory(root.path());
        let request = operation("models/ollama/model.gguf", b"model-data", 10, None);

        assert!(verify_declared_export_artifacts(&fd, &request).is_err());
    }

    #[test]
    fn rejects_missing_nested_declared_artifact() {
        let root = tempfile::tempdir().unwrap();

        fs::create_dir_all(root.path().join("models/ollama")).unwrap();

        let fd = open_directory(root.path());
        let request = operation("models/ollama/model.gguf", b"model-data", 10, None);

        assert!(verify_declared_export_artifacts(&fd, &request).is_err());
    }

    #[test]
    fn rejects_hardlinked_declared_artifact() {
        let root = tempfile::tempdir().unwrap();
        let export = root.path().join("export");
        fs::create_dir(&export).unwrap();

        let model = export.join("model.gguf");
        fs::write(&model, b"model-data").unwrap();

        fs::hard_link(&model, root.path().join("outside-link")).unwrap();

        let fd = open_directory(&export);
        let request = operation("model.gguf", b"model-data", 10, None);

        assert!(verify_declared_export_artifacts(&fd, &request).is_err());
    }

    #[test]
    fn rejects_unexpected_fifo() {
        let root = tempfile::tempdir().unwrap();

        fs::write(root.path().join("model.gguf"), b"model-data").unwrap();

        let fd = open_directory(root.path());

        rustix::fs::mkfifoat(&fd, "unexpected-fifo", Mode::from_raw_mode(0o600)).unwrap();

        let request = operation("model.gguf", b"model-data", 10, None);

        assert!(verify_declared_export_artifacts(&fd, &request).is_err());
    }

    #[test]
    fn rejects_unexpected_export_file() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("model.gguf"), b"model-data").unwrap();
        fs::write(root.path().join("unexpected.txt"), b"extra").unwrap();

        let fd = open_directory(root.path());
        let request = operation("model.gguf", b"model-data", 10, None);

        assert!(verify_declared_export_artifacts(&fd, &request).is_err());
    }

    #[test]
    fn rejects_unexpected_export_directory() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("model.gguf"), b"model-data").unwrap();
        fs::create_dir(root.path().join("unexpected")).unwrap();

        let fd = open_directory(root.path());
        let request = operation("model.gguf", b"model-data", 10, None);

        assert!(verify_declared_export_artifacts(&fd, &request).is_err());
    }

    #[test]
    fn rejects_unexpected_export_symlink() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("model.gguf"), b"model-data").unwrap();
        symlink(
            root.path().join("model.gguf"),
            root.path().join("unexpected-link"),
        )
        .unwrap();

        let fd = open_directory(root.path());
        let request = operation("model.gguf", b"model-data", 10, None);

        assert!(verify_declared_export_artifacts(&fd, &request).is_err());
    }

    #[test]
    fn rejects_missing_declared_artifact() {
        let root = tempfile::tempdir().unwrap();

        let fd = open_directory(root.path());
        let request = operation("model.gguf", b"model-data", 10, None);

        assert!(verify_declared_export_artifacts(&fd, &request).is_err());
    }

    #[test]
    fn accepts_matching_artifact() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join("models")).unwrap();
        fs::write(root.path().join("models/model.gguf"), b"model-data").unwrap();

        let fd = open_directory(root.path());
        let request = operation("models/model.gguf", b"model-data", 10, None);

        verify_declared_export_artifacts(&fd, &request).unwrap();
    }

    #[test]
    fn rejects_digest_mismatch() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("model.gguf"), b"tampered").unwrap();

        let fd = open_directory(root.path());
        let request = operation("model.gguf", b"tampered", 8, Some("a".repeat(64)));

        assert!(verify_declared_export_artifacts(&fd, &request).is_err());
    }

    #[test]
    fn rejects_size_mismatch() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("model.gguf"), b"model-data").unwrap();

        let fd = open_directory(root.path());
        let request = operation("model.gguf", b"model-data", 11, None);

        assert!(verify_declared_export_artifacts(&fd, &request).is_err());
    }

    #[test]
    fn rejects_symlinked_artifact() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("real.gguf"), b"model-data").unwrap();

        symlink(
            root.path().join("real.gguf"),
            root.path().join("model.gguf"),
        )
        .unwrap();

        let fd = open_directory(root.path());
        let request = operation("model.gguf", b"model-data", 10, None);

        assert!(verify_declared_export_artifacts(&fd, &request).is_err());
    }
}

/// Validate one prerequisite for worker-private export staging.
///
/// This check does not authenticate the caller, validate directory ancestry,
/// establish a process security boundary, or authorize publication.
#[cfg(target_os = "linux")]
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "retained for isolated exporter staging integration; production export remains disabled"
    )
)]
pub(crate) fn validate_isolated_export_staging(
    directory: &std::os::fd::OwnedFd,
    worker_uid: u32,
    invoking_uid: u32,
) -> std::io::Result<()> {
    use rustix::fs::FileType;
    use std::io::{Error, ErrorKind};

    let metadata = rustix::fs::fstat(directory)?;

    if worker_uid == 0
        || invoking_uid == 0
        || worker_uid == invoking_uid
        || FileType::from_raw_mode(metadata.st_mode) != FileType::Directory
        || metadata.st_uid != worker_uid
        || (metadata.st_mode & 0o7777) != 0o700
    {
        return Err(Error::new(
            ErrorKind::PermissionDenied,
            "export staging does not satisfy isolated worker ownership",
        ));
    }

    Ok(())
}

#[cfg(all(test, target_os = "linux"))]
mod isolated_staging_tests {
    use super::validate_isolated_export_staging;
    use rustix::fs::{Mode, OFlags};
    use std::io::ErrorKind;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn accepts_private_worker_owned_directory_for_different_uid() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700)).unwrap();

        let fd = rustix::fs::open(
            directory.path(),
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .unwrap();

        let owner = rustix::fs::fstat(&fd).unwrap().st_uid;
        if owner == 0 || owner == u32::MAX {
            return;
        }

        validate_isolated_export_staging(&fd, owner, owner + 1).unwrap();
    }

    #[test]
    fn rejects_same_uid_worker_and_invoker() {
        let directory = tempfile::tempdir().unwrap();
        let fd = rustix::fs::open(
            directory.path(),
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .unwrap();

        let owner = rustix::fs::fstat(&fd).unwrap().st_uid;
        let error = validate_isolated_export_staging(&fd, owner, owner).unwrap_err();

        assert_eq!(error.kind(), ErrorKind::PermissionDenied);
    }

    #[test]
    fn rejects_nonprivate_staging_directory() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o750)).unwrap();

        let fd = rustix::fs::open(
            directory.path(),
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .unwrap();

        let owner = rustix::fs::fstat(&fd).unwrap().st_uid;
        if owner == 0 || owner == u32::MAX {
            return;
        }

        let error = validate_isolated_export_staging(&fd, owner, owner + 1).unwrap_err();

        assert_eq!(error.kind(), ErrorKind::PermissionDenied);
    }
}
