//! ISO workspace layout.

#[cfg(not(target_os = "linux"))]
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
        #[cfg(target_os = "linux")]
        {
            use rustix::fs::{Mode, OFlags};
            use std::os::unix::ffi::OsStrExt;
            use std::path::Component;

            if !self.root.is_absolute() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "ISO workspace must be absolute",
                ));
            }

            let mut root_fd = rustix::fs::open(
                "/",
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
                Mode::empty(),
            )?;

            let components: Vec<_> = self.root.components().collect();
            if !components
                .iter()
                .any(|component| matches!(component, Component::Normal(_)))
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "ISO workspace cannot be filesystem root",
                ));
            }

            let final_index = components.len().checked_sub(1).ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "ISO workspace root is empty")
            })?;

            for (index, component) in components.into_iter().enumerate() {
                match component {
                    Component::RootDir => {}
                    Component::Normal(name) => {
                        let name = std::ffi::CString::new(name.as_bytes()).map_err(|_| {
                            io::Error::new(
                                io::ErrorKind::InvalidInput,
                                "ISO workspace component contains NUL",
                            )
                        })?;

                        root_fd = if index == final_index {
                            open_or_create_private_directory(&root_fd, &name)?
                        } else {
                            let directory = crate::secure_copy::open_or_create_directory_nofollow(
                                &root_fd, &name,
                            )?;
                            validate_iso_workspace_ancestor(&directory).map_err(|error| {
                                io::Error::new(
                                    error.kind(),
                                    format!("ISO ancestor component {:?}: {error}", name),
                                )
                            })?;
                            directory
                        };
                    }
                    _ => {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidInput,
                            "ISO workspace contains unsupported path component",
                        ));
                    }
                }
            }

            for components in [
                &["boot", "grub"][..],
                &["EFI", "BOOT"][..],
                &["live"][..],
                &["staging"][..],
            ] {
                let mut directory_fd = rustix::io::dup(&root_fd)?;
                for name in components {
                    directory_fd = open_or_create_private_directory(
                        &directory_fd,
                        &std::ffi::CString::new(*name).expect("static directory name"),
                    )?;
                }
            }

            return Ok(());
        }

        #[cfg(not(target_os = "linux"))]
        {
            fs::create_dir_all(self.boot_grub())?;
            fs::create_dir_all(self.efi_boot())?;
            fs::create_dir_all(self.live())?;
            fs::create_dir_all(self.staging())?;
            Ok(())
        }
    }

    #[must_use]
    pub fn boot_grub(&self) -> PathBuf {
        self.root.join("boot").join("grub")
    }

    #[must_use]
    pub fn grub_config(&self) -> PathBuf {
        self.boot_grub().join("grub.cfg")
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
    pub fn daia_payload(&self) -> PathBuf {
        self.root.join("daia")
    }
    #[must_use]
    pub fn filesystem_squashfs(&self) -> PathBuf {
        self.live().join("filesystem.squashfs")
    }
    #[must_use]
    pub fn live_kernel(&self) -> PathBuf {
        self.live().join("vmlinuz")
    }

    #[must_use]
    pub fn live_initramfs(&self) -> PathBuf {
        self.live().join("initrd.img")
    }
}
#[cfg(target_os = "linux")]
fn validate_iso_workspace_ancestor(directory: &std::os::fd::OwnedFd) -> io::Result<()> {
    use rustix::fs::FileType;

    let metadata = rustix::fs::fstat(directory)?;
    let mode = metadata.st_mode;
    let effective_uid = rustix::process::geteuid().as_raw();

    let trusted_owner = metadata.st_uid == 0 || metadata.st_uid == effective_uid;
    let protected_directory = mode & 0o022 == 0;
    let protected_sticky_directory = metadata.st_uid == 0 && mode & 0o7777 == 0o1777;

    if FileType::from_raw_mode(mode) != FileType::Directory
        || !trusted_owner
        || !(protected_directory || protected_sticky_directory)
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!(
                "ISO workspace ancestor has unsafe ownership or permissions: uid={} mode={:#o} euid={}",
                metadata.st_uid,
                mode & 0o7777,
                effective_uid,
            ),
        ));
    }

    Ok(())
}

#[cfg(target_os = "linux")]
fn open_or_create_private_directory(
    parent: &std::os::fd::OwnedFd,
    name: &std::ffi::CStr,
) -> io::Result<std::os::fd::OwnedFd> {
    use rustix::fs::{FileType, Mode, OFlags};

    let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC;

    let directory = match rustix::fs::openat(parent, name, flags, Mode::empty()) {
        Ok(directory) => directory,
        Err(rustix::io::Errno::NOENT) => {
            rustix::fs::mkdirat(parent, name, Mode::from_raw_mode(0o700))?;
            rustix::fs::openat(parent, name, flags, Mode::empty())?
        }
        Err(error) => return Err(error.into()),
    };

    let metadata = rustix::fs::fstat(&directory)?;

    if FileType::from_raw_mode(metadata.st_mode) != FileType::Directory
        || metadata.st_uid != rustix::process::geteuid().as_raw()
        || metadata.st_mode & 0o077 != 0
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "ISO workspace directory has unsafe ownership or permissions",
        ));
    }

    Ok(directory)
}

#[cfg(test)]
mod tests {
    use super::Layout;

    #[cfg(target_os = "linux")]
    #[test]
    fn layout_rejects_filesystem_root() {
        let error = Layout::new("/").create().unwrap_err();

        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
        assert!(error.to_string().contains("filesystem root"));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn layout_rejects_group_writable_ancestor() {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempfile::tempdir().unwrap();
        let ancestor = temp.path().join("shared");
        let workspace = ancestor.join("workspace");

        std::fs::create_dir(&ancestor).unwrap();
        std::fs::set_permissions(&ancestor, std::fs::Permissions::from_mode(0o770)).unwrap();

        let result = Layout::new(&workspace).create();

        assert!(result.is_err());
        assert!(!workspace.exists());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn layout_root_is_created_privately() {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempfile::tempdir().unwrap();
        std::fs::set_permissions(temp.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        let workspace = temp.path().join("workspace");

        Layout::new(&workspace).create().unwrap();

        let mode = std::fs::metadata(&workspace).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o700);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn layout_rejects_existing_insecure_root_without_changing_it() {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempfile::tempdir().unwrap();
        let workspace = temp.path().join("workspace");

        std::fs::create_dir(&workspace).unwrap();
        std::fs::set_permissions(&workspace, std::fs::Permissions::from_mode(0o755)).unwrap();

        assert!(Layout::new(&workspace).create().is_err());

        let mode = std::fs::metadata(&workspace).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o755);
        assert!(!workspace.join("boot").exists());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn private_directory_is_created_with_restricted_permissions() {
        use rustix::fs::{Mode, OFlags};

        let temp = tempfile::tempdir().unwrap();
        let parent = rustix::fs::open(
            temp.path(),
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .unwrap();

        let directory = super::open_or_create_private_directory(&parent, c"private").unwrap();
        let metadata = rustix::fs::fstat(&directory).unwrap();

        assert_eq!(metadata.st_mode & 0o777, 0o700);
        assert_eq!(metadata.st_uid, rustix::process::geteuid().as_raw());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn private_directory_rejects_existing_insecure_mode() {
        use rustix::fs::{Mode, OFlags};
        use std::os::unix::fs::PermissionsExt;

        let temp = tempfile::tempdir().unwrap();
        let existing = temp.path().join("existing");

        std::fs::create_dir(&existing).unwrap();
        std::fs::set_permissions(&existing, std::fs::Permissions::from_mode(0o755)).unwrap();

        let parent = rustix::fs::open(
            temp.path(),
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .unwrap();

        assert!(super::open_or_create_private_directory(&parent, c"existing").is_err());

        let mode = std::fs::metadata(&existing).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o755);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn private_directory_rejects_symlink() {
        use rustix::fs::{Mode, OFlags};
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir().unwrap();
        std::fs::create_dir(temp.path().join("outside")).unwrap();
        symlink("outside", temp.path().join("alias")).unwrap();

        let parent = rustix::fs::open(
            temp.path(),
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .unwrap();

        assert!(super::open_or_create_private_directory(&parent, c"alias").is_err());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn rejects_symlinked_workspace_root() {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir().unwrap();
        let outside = temp.path().join("outside");
        let alias = temp.path().join("workspace");

        std::fs::create_dir(&outside).unwrap();
        symlink(&outside, &alias).unwrap();

        assert!(Layout::new(&alias).create().is_err());
        assert!(!outside.join("boot").exists());
        assert!(!outside.join("EFI").exists());
        assert!(!outside.join("live").exists());
        assert!(!outside.join("staging").exists());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn rejects_symlinked_workspace_ancestor() {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir().unwrap();
        let outside = temp.path().join("outside");
        let alias = temp.path().join("alias");

        std::fs::create_dir(&outside).unwrap();
        symlink(&outside, &alias).unwrap();

        assert!(Layout::new(alias.join("workspace")).create().is_err());
        assert!(!outside.join("workspace").exists());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn rejects_symlinked_workspace_subdirectory() {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir().unwrap();
        let workspace = temp.path().join("workspace");
        let outside = temp.path().join("outside");

        std::fs::create_dir(&workspace).unwrap();
        std::fs::create_dir(&outside).unwrap();
        symlink(&outside, workspace.join("boot")).unwrap();

        assert!(Layout::new(&workspace).create().is_err());
        assert!(!outside.join("grub").exists());
    }

    #[test]
    fn layout_builds_expected_paths() {
        let layout = Layout::new("/tmp/work/iso");

        assert_eq!(layout.root(), std::path::Path::new("/tmp/work/iso"));
        assert_eq!(
            layout.boot_grub(),
            std::path::Path::new("/tmp/work/iso/boot/grub")
        );
        assert_eq!(
            layout.grub_config(),
            std::path::Path::new("/tmp/work/iso/boot/grub/grub.cfg")
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
            layout.daia_payload(),
            std::path::Path::new("/tmp/work/iso/daia")
        );
        assert_eq!(
            layout.filesystem_squashfs(),
            std::path::Path::new("/tmp/work/iso/live/filesystem.squashfs")
        );
        assert_eq!(
            layout.live_kernel(),
            std::path::Path::new("/tmp/work/iso/live/vmlinuz")
        );

        assert_eq!(
            layout.live_initramfs(),
            std::path::Path::new("/tmp/work/iso/live/initrd.img")
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
