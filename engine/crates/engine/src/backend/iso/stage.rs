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
        let boot_directory = context.config.rootfs.join("boot");
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
        let boot_directory = context.config.rootfs.join("boot");

        find_initramfs(&boot_directory)
    }
}

fn find_kernel(boot_directory: &Path) -> io::Result<PathBuf> {
    find_single_file(boot_directory, "vmlinuz-", "Linux kernel")
}

fn find_initramfs(boot_directory: &Path) -> io::Result<PathBuf> {
    find_single_file(boot_directory, "initrd.img-", "initramfs")
}

fn find_single_file(directory: &Path, prefix: &str, artifact_name: &str) -> io::Result<PathBuf> {
    let mut matching_files = fs::read_dir(directory)?
        .filter_map(|entry| match entry {
            Ok(entry) if has_prefix(&entry.file_name(), prefix) => Some(Ok(entry.path())),
            Ok(_) => None,
            Err(error) => Some(Err(error)),
        })
        .collect::<io::Result<Vec<_>>>()?;

    matching_files.sort();

    match matching_files.as_slice() {
        [path] => Ok(path.clone()),
        [] => Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("no {artifact_name} was found in {}", directory.display()),
        )),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "multiple {artifact_name} files were found in {}",
                directory.display()
            ),
        )),
    }
}

fn has_prefix(file_name: &OsStr, prefix: &str) -> bool {
    file_name
        .to_str()
        .is_some_and(|name| name.starts_with(prefix))
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
