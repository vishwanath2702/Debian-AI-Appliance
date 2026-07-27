//! ISO pipeline stages.

use std::{
    ffi::OsStr,
    fs, io,
    path::{Path, PathBuf},
    process::Command,
};

use super::IsoContext;
/// Validates the source ISO before the build begins.
pub struct SourceIsoStage;

impl SourceIsoStage {
    /// Validates that the configured source ISO exists, is a regular file and
    /// is readable.
    ///
    /// # Errors
    ///
    /// Returns an error if the source ISO does not exist, is not a regular
    /// file, or cannot be opened for reading.
    pub fn run(context: &IsoContext) -> io::Result<()> {
        let source_iso = &context.config.source_iso;

        let metadata = fs::metadata(source_iso)?;

        if !metadata.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("source ISO is not a regular file: {}", source_iso.display()),
            ));
        }

        fs::File::open(source_iso)?;

        Ok(())
    }
}
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

/// Copies boot artifacts into the ISO live directory.
pub struct BootArtifactsStage;

impl BootArtifactsStage {
    /// Copies and renames the discovered kernel and initramfs.
    ///
    /// # Errors
    ///
    /// Returns an error if either boot artifact cannot be copied.
    pub fn run(context: &IsoContext, kernel: &Path, initramfs: &Path) -> io::Result<()> {
        fs::copy(kernel, context.config.layout.live_kernel())?;
        fs::copy(initramfs, context.config.layout.live_initramfs())?;

        Ok(())
    }
}
/// Generates the GRUB boot menu configuration.
pub struct GrubConfigStage;

impl GrubConfigStage {
    /// Writes the GRUB configuration into the ISO boot directory.
    ///
    /// # Errors
    ///
    /// Returns an error if the GRUB configuration cannot be written.
    pub fn run(context: &IsoContext) -> io::Result<PathBuf> {
        let output = context.config.layout.grub_config();

        fs::write(
            &output,
            concat!(
                "set default=0\n",
                "set timeout=5\n",
                "\n",
                "menuentry \"Debian AI Appliance\" {\n",
                "    linux /live/vmlinuz boot=live quiet\n",
                "    initrd /live/initrd.img\n",
                "}\n",
            ),
        )?;

        Ok(output)
    }
}
/// Determines the location of the `SquashFS` image.
/// Builds the compressed root filesystem image.
pub struct SquashFsStage;

impl SquashFsStage {
    /// Builds the `SquashFS` image from the prepared root filesystem.
    ///
    /// # Errors
    ///
    /// Returns an error if `mksquashfs` cannot be started or exits
    /// unsuccessfully.
    pub fn run(context: &IsoContext) -> io::Result<PathBuf> {
        let output = context.config.layout.filesystem_squashfs();

        let status = Command::new("mksquashfs")
            .arg(&context.config.rootfs)
            .arg(&output)
            .args(["-comp", "xz", "-noappend", "-e", "boot"])
            .status()?;

        if !status.success() {
            return Err(io::Error::other(format!(
                "mksquashfs failed with status {status}"
            )));
        }

        Ok(output)
    }
}
/// Produces the final ISO while replaying boot metadata from a source ISO.
pub struct IsoImageStage;

impl IsoImageStage {
    /// Builds the final ISO image from the prepared ISO workspace.
    ///
    /// The source ISO supplies the existing BIOS and UEFI boot metadata. The
    /// prepared workspace is mapped over the source ISO filesystem tree.
    ///
    /// # Errors
    ///
    /// Returns an error if the output directory cannot be prepared,
    /// `xorriso` cannot be started, `xorriso` exits unsuccessfully, or the
    /// command completes without creating the requested output image.
    pub fn run(context: &IsoContext) -> io::Result<PathBuf> {
        let output = &context.config.output_iso;

        if let Some(parent) = output
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent)?;
        }

        if output.exists() {
            fs::remove_file(output)?;
        }

        let status = Command::new(&context.config.xorriso_command)
            .arg("-indev")
            .arg(&context.config.source_iso)
            .arg("-outdev")
            .arg(output)
            .args(["-boot_image", "any", "replay"])
            .arg("-map")
            .arg(context.config.layout.root())
            .arg("/")
            .arg("-commit")
            .status()?;

        if !status.success() {
            return Err(io::Error::other(format!(
                "xorriso failed with status {status}"
            )));
        }

        if !output.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("xorriso completed without producing {}", output.display()),
            ));
        }

        Ok(output.clone())
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
