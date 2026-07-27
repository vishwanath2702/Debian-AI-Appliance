//! ISO pipeline stages.

use std::{
    ffi::OsStr,
    fs, io,
    path::{Path, PathBuf},
};

use super::IsoContext;

/// Creates the ISO workspace layout.
pub struct WorkspaceStage;

impl WorkspaceStage {
    /// Creates the ISO workspace directories.
    ///
    /// # Errors
    ///
    /// Returns an error if the workspace layout cannot be created.
    pub fn run(context: &IsoContext) -> io::Result<()> {
        context.config.layout.create()
    }
}

/// Discovers the Linux kernel in the prepared root filesystem.
pub struct KernelStage;

impl KernelStage {
    /// Finds the Linux kernel under the root filesystem's `/boot` directory.
    ///
    /// # Errors
    ///
    /// Returns an error if the boot directory cannot be read, no kernel is
    /// found, or more than one kernel is present.
    pub fn run(context: &IsoContext) -> io::Result<PathBuf> {
        let boot_directory = context.config.layout.root().join("boot");
        find_kernel(&boot_directory)
    }
}
/// Discovers the initramfs in the prepared root filesystem.
pub struct InitramfsStage;

impl InitramfsStage {
    /// Finds the initramfs under the root filesystem's `/boot` directory.
    ///
    /// # Errors
    ///
    /// Returns an error if the boot directory cannot be read, no initramfs is
    /// found, or more than one initramfs is present.
    pub fn run(context: &IsoContext) -> io::Result<PathBuf> {
        let boot_directory = context.config.layout.root().join("boot");

        find_initramfs(&boot_directory)
    }
}

fn find_kernel(boot_directory: &Path) -> io::Result<PathBuf> {
    let mut kernels = fs::read_dir(boot_directory)?
        .filter_map(|entry| match entry {
            Ok(entry) if is_kernel_name(&entry.file_name()) => Some(Ok(entry.path())),
            Ok(_) => None,
            Err(error) => Some(Err(error)),
        })
        .collect::<io::Result<Vec<_>>>()?;

    kernels.sort();

    match kernels.as_slice() {
        [kernel] => Ok(kernel.clone()),
        [] => Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("no Linux kernel was found in {}", boot_directory.display()),
        )),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "multiple Linux kernels were found in {}",
                boot_directory.display()
            ),
        )),
    }
}

fn find_initramfs(boot_directory: &Path) -> io::Result<PathBuf> {
    let mut initramfs_images = fs::read_dir(boot_directory)?
        .filter_map(|entry| match entry {
            Ok(entry) if is_initramfs_name(&entry.file_name()) => Some(Ok(entry.path())),
            Ok(_) => None,
            Err(error) => Some(Err(error)),
        })
        .collect::<io::Result<Vec<_>>>()?;

    initramfs_images.sort();

    match initramfs_images.as_slice() {
        [initramfs] => Ok(initramfs.clone()),
        [] => Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("no initramfs was found in {}", boot_directory.display()),
        )),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "multiple initramfs images were found in {}",
                boot_directory.display()
            ),
        )),
    }
}

fn is_kernel_name(file_name: &OsStr) -> bool {
    file_name
        .to_str()
        .is_some_and(|name| name.starts_with("vmlinuz-"))
}

fn is_initramfs_name(file_name: &OsStr) -> bool {
    file_name
        .to_str()
        .is_some_and(|name| name.starts_with("initrd.img-"))
}

#[cfg(test)]
mod tests {
    use std::{
        fs::{self, File},
        path::{Path, PathBuf},
        sync::atomic::{AtomicUsize, Ordering},
    };

    use super::{find_initramfs, find_kernel};

    static NEXT_DIRECTORY_ID: AtomicUsize = AtomicUsize::new(0);

    struct TestDirectory {
        path: PathBuf,
    }

    impl TestDirectory {
        fn create() -> Self {
            let directory_id = NEXT_DIRECTORY_ID.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "daia-kernel-stage-{}-{directory_id}",
                std::process::id()
            ));

            fs::create_dir_all(&path).expect("test directory should be created");

            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }

        fn create_file(&self, name: &str) -> PathBuf {
            let path = self.path.join(name);

            File::create(&path).expect("test file should be created");

            path
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.path).expect("test directory should be removed");
        }
    }

    #[test]
    fn finds_single_kernel() {
        let boot_directory = TestDirectory::create();
        let expected = boot_directory.create_file("vmlinuz-6.12.0-amd64");

        boot_directory.create_file("initrd.img-6.12.0-amd64");
        boot_directory.create_file("config-6.12.0-amd64");

        let kernel = find_kernel(boot_directory.path()).expect("kernel should be found");

        assert_eq!(kernel, expected);
    }

    #[test]
    fn returns_error_when_kernel_is_missing() {
        let boot_directory = TestDirectory::create();

        boot_directory.create_file("initrd.img-6.12.0-amd64");

        let error =
            find_kernel(boot_directory.path()).expect_err("missing kernel should return an error");

        assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
    }

    #[test]
    fn returns_error_when_multiple_kernels_exist() {
        let boot_directory = TestDirectory::create();

        boot_directory.create_file("vmlinuz-6.12.0-amd64");
        boot_directory.create_file("vmlinuz-6.13.0-amd64");

        let error = find_kernel(boot_directory.path())
            .expect_err("multiple kernels should return an error");

        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
    }
    #[test]
    fn finds_single_initramfs() {
        let boot_directory = TestDirectory::create();
        let expected = boot_directory.create_file("initrd.img-6.12.0-amd64");

        boot_directory.create_file("vmlinuz-6.12.0-amd64");
        boot_directory.create_file("config-6.12.0-amd64");

        let initramfs = find_initramfs(boot_directory.path()).expect("initramfs should be found");

        assert_eq!(initramfs, expected);
    }

    #[test]
    fn returns_error_when_initramfs_is_missing() {
        let boot_directory = TestDirectory::create();

        boot_directory.create_file("vmlinuz-6.12.0-amd64");

        let error = find_initramfs(boot_directory.path())
            .expect_err("missing initramfs should return an error");

        assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
    }

    #[test]
    fn returns_error_when_multiple_initramfs_images_exist() {
        let boot_directory = TestDirectory::create();

        boot_directory.create_file("initrd.img-6.12.0-amd64");
        boot_directory.create_file("initrd.img-6.13.0-amd64");

        let error = find_initramfs(boot_directory.path())
            .expect_err("multiple initramfs images should return an error");

        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
    }
}
