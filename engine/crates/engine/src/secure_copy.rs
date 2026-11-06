//! Descriptor-based filesystem copying for trusted DAIA operations.
//!
//! Shared mechanisms belong here; installation and ISO staging retain
//! their own authorization, ownership, and integrity policies.

use std::io;

#[cfg(target_os = "linux")]
pub(crate) fn open_source_directory_nofollow(
    source: &std::path::Path,
) -> io::Result<std::os::fd::OwnedFd> {
    use rustix::fs::{Mode, OFlags};
    use std::path::Component;

    if !source.is_absolute() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "source directory must be absolute",
        ));
    }

    let mut directory = rustix::fs::open(
        "/",
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
    )?;

    for component in source.components() {
        let Component::Normal(name) = component else {
            if component == Component::RootDir {
                continue;
            }

            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid source directory component",
            ));
        };

        directory = rustix::fs::openat(
            &directory,
            name,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )?;
    }

    Ok(directory)
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::{open_or_create_directory_nofollow, open_source_directory_nofollow};
    use std::{fs, io, os::unix::fs::symlink, path::Path};

    #[test]
    fn opens_absolute_source_directory() {
        let temporary = tempfile::tempdir().unwrap();
        let source = temporary.path().join("payload");
        fs::create_dir(&source).unwrap();

        let descriptor =
            open_source_directory_nofollow(&source).expect("absolute source directory should open");

        let metadata = rustix::fs::fstat(&descriptor).unwrap();
        assert!(rustix::fs::FileType::from_raw_mode(metadata.st_mode).is_dir());
    }

    #[test]
    fn rejects_relative_source_directory() {
        let error = open_source_directory_nofollow(Path::new("payload"))
            .expect_err("relative source directory must be rejected");

        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    }

    #[test]
    fn rejects_symlinked_source_directory() {
        let temporary = tempfile::tempdir().unwrap();
        let source = temporary.path().join("payload");
        let linked = temporary.path().join("linked");

        fs::create_dir(&source).unwrap();
        symlink(&source, &linked).unwrap();

        open_source_directory_nofollow(&linked)
            .expect_err("symlinked source directory must be rejected");
    }

    #[test]
    fn rejects_symlinked_source_ancestor() {
        let temporary = tempfile::tempdir().unwrap();
        let actual = temporary.path().join("actual");
        let linked = temporary.path().join("linked");

        fs::create_dir(&actual).unwrap();
        fs::create_dir(actual.join("payload")).unwrap();
        symlink(&actual, &linked).unwrap();

        open_source_directory_nofollow(&linked.join("payload"))
            .expect_err("symlinked source ancestor must be rejected");
    }

    fn open_test_parent(path: &Path) -> std::os::fd::OwnedFd {
        rustix::fs::open(
            path,
            rustix::fs::OFlags::RDONLY
                | rustix::fs::OFlags::DIRECTORY
                | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        )
        .unwrap()
    }

    #[test]
    fn shared_destination_creates_and_reopens_directory() {
        let temporary = tempfile::tempdir().unwrap();
        let parent = open_test_parent(temporary.path());

        let first = open_or_create_directory_nofollow(&parent, c"payload").unwrap();
        let second = open_or_create_directory_nofollow(&parent, c"payload").unwrap();

        let first_metadata = rustix::fs::fstat(&first).unwrap();
        let second_metadata = rustix::fs::fstat(&second).unwrap();

        assert_eq!(first_metadata.st_ino, second_metadata.st_ino);
        assert_eq!(first_metadata.st_dev, second_metadata.st_dev);
        assert!(temporary.path().join("payload").is_dir());
    }

    #[test]
    fn shared_destination_rejects_symlink() {
        let temporary = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let parent = open_test_parent(temporary.path());

        symlink(outside.path(), temporary.path().join("linked")).unwrap();

        open_or_create_directory_nofollow(&parent, c"linked")
            .expect_err("destination symlink must be rejected");
    }

    #[test]
    fn shared_destination_rejects_regular_file() {
        let temporary = tempfile::tempdir().unwrap();
        let parent = open_test_parent(temporary.path());

        fs::write(temporary.path().join("occupied"), b"preserve").unwrap();

        open_or_create_directory_nofollow(&parent, c"occupied")
            .expect_err("regular file must not be opened as a directory");

        assert_eq!(
            fs::read(temporary.path().join("occupied")).unwrap(),
            b"preserve"
        );
    }

    #[test]
    fn shared_destination_rejects_symlink_to_missing_directory() {
        let temporary = tempfile::tempdir().unwrap();
        let parent = open_test_parent(temporary.path());

        symlink("missing", temporary.path().join("linked")).unwrap();

        open_or_create_directory_nofollow(&parent, c"linked")
            .expect_err("dangling destination symlink must be rejected");
    }
}

#[cfg(target_os = "linux")]
pub(crate) fn open_or_create_directory_nofollow(
    parent: &std::os::fd::OwnedFd,
    name: &std::ffi::CStr,
) -> io::Result<std::os::fd::OwnedFd> {
    use rustix::fs::{Mode, OFlags};

    let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC;

    let directory = match rustix::fs::openat(parent, name, flags, Mode::empty()) {
        Ok(directory) => directory,
        Err(rustix::io::Errno::NOENT) => {
            match rustix::fs::mkdirat(parent, name, Mode::from_raw_mode(0o755)) {
                Ok(()) | Err(rustix::io::Errno::EXIST) => {}
                Err(error) => return Err(error.into()),
            }

            rustix::fs::openat(parent, name, flags, Mode::empty())?
        }
        Err(error) => return Err(error.into()),
    };

    Ok(directory)
}

#[cfg(target_os = "linux")]
pub(crate) fn copy_regular_file_at(
    source_fd: std::os::fd::OwnedFd,
    destination_directory: &std::os::fd::OwnedFd,
    name: &std::ffi::CStr,
) -> io::Result<()> {
    use rustix::fs::{AtFlags, FileType, Mode, OFlags};
    use std::io::Write;

    let source_metadata = rustix::fs::fstat(&source_fd)?;

    if !FileType::from_raw_mode(source_metadata.st_mode).is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "runtime source entry is not a regular file",
        ));
    }

    match rustix::fs::statat(destination_directory, name, AtFlags::SYMLINK_NOFOLLOW) {
        Ok(metadata) => {
            if !FileType::from_raw_mode(metadata.st_mode).is_file() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "runtime destination entry is not a regular file",
                ));
            }
        }
        Err(rustix::io::Errno::NOENT) => {}
        Err(error) => return Err(error.into()),
    }

    let (temporary_name, temporary_fd) = {
        let mut created = None;

        for _ in 0..16 {
            let mut random = [0_u8; 16];
            let filled =
                rustix::rand::getrandom(&mut random, rustix::rand::GetRandomFlags::empty())?;

            if filled != random.len() {
                return Err(io::Error::other(
                    "incomplete runtime temporary-name randomness",
                ));
            }

            let suffix: String = random.iter().map(|byte| format!("{byte:02x}")).collect();
            let temporary_name = format!(".daia-runtime-{suffix}");

            match rustix::fs::openat(
                destination_directory,
                temporary_name.as_str(),
                OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::from_raw_mode(0o600),
            ) {
                Ok(fd) => {
                    created = Some((temporary_name, fd));
                    break;
                }
                Err(rustix::io::Errno::EXIST) => continue,
                Err(error) => return Err(error.into()),
            }
        }

        created.ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::AlreadyExists,
                "unable to allocate runtime temporary file",
            )
        })?
    };

    let result = (|| -> io::Result<()> {
        let mut source_file = std::fs::File::from(source_fd);
        let mut temporary_file = std::fs::File::from(temporary_fd);

        io::copy(&mut source_file, &mut temporary_file)?;
        temporary_file.flush()?;

        let mode = Mode::from_raw_mode(source_metadata.st_mode & 0o777);
        rustix::fs::fchmod(&temporary_file, mode)?;
        temporary_file.sync_all()?;

        match rustix::fs::statat(destination_directory, name, AtFlags::SYMLINK_NOFOLLOW) {
            Ok(metadata) => {
                if !FileType::from_raw_mode(metadata.st_mode).is_file() {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "runtime destination changed to an unsupported file type",
                    ));
                }
            }
            Err(rustix::io::Errno::NOENT) => {}
            Err(error) => return Err(error.into()),
        }

        rustix::fs::renameat(
            destination_directory,
            temporary_name.as_str(),
            destination_directory,
            name,
        )?;

        Ok(())
    })();

    if result.is_err() {
        let _ = rustix::fs::unlinkat(
            destination_directory,
            temporary_name.as_str(),
            AtFlags::empty(),
        );
    }

    result
}

#[cfg(all(test, target_os = "linux"))]
mod shared_regular_file_copy_tests {
    use super::copy_regular_file_at;
    use rustix::fs::{Mode, OFlags};
    use std::os::unix::fs::{PermissionsExt, symlink};

    fn open_directory(path: &std::path::Path) -> std::os::fd::OwnedFd {
        rustix::fs::open(
            path,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .unwrap()
    }

    #[test]
    fn shared_file_copy_preserves_executable_mode() {
        let source = tempfile::tempdir().unwrap();
        let destination = tempfile::tempdir().unwrap();

        let file = source.path().join("tool");
        std::fs::write(&file, b"executable").unwrap();
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755)).unwrap();

        let source_directory = open_directory(source.path());
        let destination_directory = open_directory(destination.path());

        let source_fd = rustix::fs::openat(
            &source_directory,
            c"tool",
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .unwrap();

        copy_regular_file_at(source_fd, &destination_directory, c"tool").unwrap();

        let copied = destination.path().join("tool");
        assert_eq!(std::fs::read(&copied).unwrap(), b"executable");
        assert_eq!(
            std::fs::metadata(copied).unwrap().permissions().mode() & 0o777,
            0o755
        );
    }

    #[test]
    fn shared_file_copy_replaces_existing_regular_file() {
        let source = tempfile::tempdir().unwrap();
        let destination = tempfile::tempdir().unwrap();

        std::fs::write(source.path().join("config"), b"new").unwrap();
        std::fs::write(destination.path().join("config"), b"old").unwrap();

        let source_directory = open_directory(source.path());
        let destination_directory = open_directory(destination.path());

        let source_fd = rustix::fs::openat(
            &source_directory,
            c"config",
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .unwrap();

        copy_regular_file_at(source_fd, &destination_directory, c"config").unwrap();

        assert_eq!(
            std::fs::read(destination.path().join("config")).unwrap(),
            b"new"
        );
    }

    #[test]
    fn shared_file_copy_rejects_destination_symlink() {
        let source = tempfile::tempdir().unwrap();
        let destination = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();

        std::fs::write(source.path().join("model"), b"model-data").unwrap();

        let outside_file = outside.path().join("protected");
        std::fs::write(&outside_file, b"preserve").unwrap();

        symlink(&outside_file, destination.path().join("model")).unwrap();

        let source_directory = open_directory(source.path());
        let destination_directory = open_directory(destination.path());

        let source_fd = rustix::fs::openat(
            &source_directory,
            c"model",
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .unwrap();

        copy_regular_file_at(source_fd, &destination_directory, c"model")
            .expect_err("destination symlink must be rejected");

        assert_eq!(std::fs::read(outside_file).unwrap(), b"preserve");
        assert!(
            std::fs::symlink_metadata(destination.path().join("model"))
                .unwrap()
                .file_type()
                .is_symlink()
        );
    }

    #[test]
    fn shared_file_copy_rejects_source_directory() {
        let source = tempfile::tempdir().unwrap();
        let destination = tempfile::tempdir().unwrap();

        std::fs::create_dir(source.path().join("nested")).unwrap();

        let source_directory = open_directory(source.path());
        let destination_directory = open_directory(destination.path());

        let nested_fd = rustix::fs::openat(
            &source_directory,
            c"nested",
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .unwrap();

        copy_regular_file_at(nested_fd, &destination_directory, c"nested")
            .expect_err("source directory must not be copied as a file");

        assert!(!destination.path().join("nested").exists());
    }
}

#[cfg(target_os = "linux")]
pub(crate) fn copy_directory_contents_at<F>(
    source_directory: &std::os::fd::OwnedFd,
    destination_directory: &std::os::fd::OwnedFd,
    open_destination: &F,
) -> io::Result<()>
where
    F: Fn(&std::os::fd::OwnedFd, &std::ffi::CStr) -> io::Result<std::os::fd::OwnedFd>,
{
    use rustix::fs::{FileType, Mode, OFlags, RawDir};
    use std::mem::MaybeUninit;

    let mut buffer = [MaybeUninit::<u8>::uninit(); 8192];
    let mut entries = RawDir::new(source_directory, &mut buffer);

    while let Some(result) = entries.next() {
        let entry = result?;
        let name = entry.file_name();

        if name.to_bytes() == b"." || name.to_bytes() == b".." {
            continue;
        }

        let source_fd = rustix::fs::openat(
            source_directory,
            name,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::empty(),
        )?;

        let metadata = rustix::fs::fstat(&source_fd)?;

        match FileType::from_raw_mode(metadata.st_mode) {
            FileType::Directory => {
                let destination_fd = open_destination(destination_directory, name)?;

                copy_directory_contents_at(&source_fd, &destination_fd, open_destination)?;
            }
            FileType::RegularFile => {
                copy_regular_file_at(source_fd, destination_directory, name)?;
            }
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "runtime payload contains an unsupported file type",
                ));
            }
        }
    }

    Ok(())
}

#[cfg(all(test, target_os = "linux"))]
mod shared_recursive_copy_tests {
    use super::{copy_directory_contents_at, open_or_create_directory_nofollow};
    use rustix::fs::{Mode, OFlags};
    use std::{io, os::unix::fs::symlink};

    fn open_directory(path: &std::path::Path) -> std::os::fd::OwnedFd {
        rustix::fs::open(
            path,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .unwrap()
    }

    #[test]
    fn shared_recursive_copy_preserves_nested_files() {
        let source = tempfile::tempdir().unwrap();
        let destination = tempfile::tempdir().unwrap();

        std::fs::create_dir_all(source.path().join("etc/daia")).unwrap();
        std::fs::write(source.path().join("etc/daia/config"), b"shared-copy").unwrap();

        let source_fd = open_directory(source.path());
        let destination_fd = open_directory(destination.path());

        copy_directory_contents_at(
            &source_fd,
            &destination_fd,
            &open_or_create_directory_nofollow,
        )
        .unwrap();

        assert_eq!(
            std::fs::read(destination.path().join("etc/daia/config")).unwrap(),
            b"shared-copy"
        );
    }

    #[test]
    fn shared_recursive_copy_rejects_source_symlink() {
        let source = tempfile::tempdir().unwrap();
        let destination = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();

        let secret = outside.path().join("secret");
        std::fs::write(&secret, b"preserve").unwrap();

        symlink(&secret, source.path().join("linked")).unwrap();

        let source_fd = open_directory(source.path());
        let destination_fd = open_directory(destination.path());

        copy_directory_contents_at(
            &source_fd,
            &destination_fd,
            &open_or_create_directory_nofollow,
        )
        .expect_err("source symlink must be rejected");

        assert!(!destination.path().join("linked").exists());
        assert_eq!(std::fs::read(secret).unwrap(), b"preserve");
    }

    #[test]
    fn shared_recursive_copy_rejects_destination_symlink() {
        let source = tempfile::tempdir().unwrap();
        let destination = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();

        std::fs::create_dir(source.path().join("nested")).unwrap();
        std::fs::write(source.path().join("nested/config"), b"data").unwrap();

        symlink(outside.path(), destination.path().join("nested")).unwrap();

        let source_fd = open_directory(source.path());
        let destination_fd = open_directory(destination.path());

        copy_directory_contents_at(
            &source_fd,
            &destination_fd,
            &open_or_create_directory_nofollow,
        )
        .expect_err("destination directory symlink must be rejected");

        assert!(!outside.path().join("config").exists());
    }

    #[test]
    fn shared_recursive_copy_respects_destination_policy() {
        let source = tempfile::tempdir().unwrap();
        let destination = tempfile::tempdir().unwrap();

        std::fs::create_dir(source.path().join("nested")).unwrap();
        std::fs::write(source.path().join("nested/config"), b"data").unwrap();

        let source_fd = open_directory(source.path());
        let destination_fd = open_directory(destination.path());

        let deny = |_parent: &std::os::fd::OwnedFd,
                    _name: &std::ffi::CStr|
         -> io::Result<std::os::fd::OwnedFd> {
            Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "test destination policy denied creation",
            ))
        };

        let error = copy_directory_contents_at(&source_fd, &destination_fd, &deny)
            .expect_err("destination policy must be enforced");

        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
        assert!(!destination.path().join("nested").exists());
    }
}
