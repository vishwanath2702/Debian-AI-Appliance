//! ISO pipeline stages.

use std::{
    ffi::OsStr,
    fs, io,
    path::{Path, PathBuf},
    process::Command,
};

use super::IsoContext;
use inspector::{IsoInspector, IsoMetadata};
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
/// Inspects metadata from the source ISO.
pub struct InspectionStage;

impl InspectionStage {
    /// Inspects and identifies the configured source ISO.
    ///
    /// # Errors
    ///
    /// Returns an error if the source ISO cannot be inspected or identified.
    pub fn run(context: &IsoContext, inspector: &dyn IsoInspector) -> io::Result<IsoMetadata> {
        inspector
            .inspect(&context.config.source_iso)
            .map_err(|error| io::Error::other(error.to_string()))
    }
}
/// Validates inspected source ISO metadata.
pub struct MetadataValidationStage;

impl MetadataValidationStage {
    /// Validates metadata discovered from the configured source ISO.
    ///
    /// # Errors
    ///
    /// Returns an error if metadata is missing, inconsistent, or unsupported.
    pub fn run(context: &IsoContext) -> io::Result<()> {
        let metadata = context.state.metadata.as_ref().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "source ISO metadata is missing")
        })?;

        if metadata.path() != context.config.source_iso {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "inspected ISO path does not match configured source ISO: expected {}, found {}",
                    context.config.source_iso.display(),
                    metadata.path().display(),
                ),
            ));
        }

        if !metadata.distribution().eq_ignore_ascii_case("Debian") {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("unsupported ISO distribution: {}", metadata.distribution()),
            ));
        }

        validate_metadata_field("version", metadata.version())?;
        validate_metadata_field("codename", metadata.codename())?;
        validate_metadata_field("architecture", metadata.architecture())?;
        validate_metadata_field("media type", metadata.media_type())?;

        if metadata.boot_modes().is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "source ISO does not provide a supported boot mode",
            ));
        }

        Ok(())
    }
}

fn validate_metadata_field(name: &str, value: &str) -> io::Result<()> {
    if value.trim().is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("source ISO metadata field is empty: {name}"),
        ));
    }

    Ok(())
}
/// Validates that required external build tools are available.
pub struct ToolValidationStage;

impl ToolValidationStage {
    /// Ensures all required external commands report a successful version.
    ///
    /// # Errors
    ///
    /// Returns an error if any required tool cannot be executed or its
    /// `--version` command exits unsuccessfully.
    pub fn run(context: &IsoContext) -> io::Result<()> {
        validate_tool(&context.config.mksquashfs_command)?;
        validate_tool(&context.config.xorriso_command)?;
        Ok(())
    }
}

fn validate_tool(command: &Path) -> io::Result<()> {
    match Command::new(command).arg("--version").status() {
        Ok(status) if status.success() => Ok(()),
        Ok(status) => Err(io::Error::other(format!(
            "required tool failed with status {status}: {}",
            command.display()
        ))),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("required tool not found: {}", command.display()),
        )),
        Err(error) => Err(error),
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
        let grub = &context.config.grub;
        let contents = format!(
            concat!(
                "set default=0\n",
                "set timeout={}\n",
                "\n",
                "menuentry \"{}\" {{\n",
                "    linux /live/vmlinuz {}\n",
                "    initrd /live/initrd.img\n",
                "}}\n",
            ),
            grub.timeout, grub.menu_title, grub.kernel_command_line,
        );

        fs::write(&output, contents)?;

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
        let squashfs = &context.config.squashfs;
        let mut command = Command::new(&context.config.mksquashfs_command);

        command.arg(&context.config.rootfs).arg(&output).args([
            "-comp",
            &squashfs.compression,
            "-noappend",
        ]);

        if !squashfs.exclusions.is_empty() {
            command.arg("-e").args(&squashfs.exclusions);
        }

        let status = command.status()?;

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
    use inspector::{BootMode, InspectError, IsoInspector, IsoMetadata};
    use std::{
        fs::{self, File},
        os::unix::fs::PermissionsExt,
        path::{Path, PathBuf},
        sync::atomic::{AtomicUsize, Ordering},
    };

    use super::{
        InspectionStage, MetadataValidationStage, SourceIsoStage, find_initramfs, find_kernel,
        validate_tool,
    };
    use crate::backend::iso::{
        GrubConfig, IsoConfig, IsoContext, IsoState, Layout, SquashFsConfig,
    };

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

        fn create_executable(&self, name: &str, contents: &str) -> PathBuf {
            let path = self.path.join(name);

            fs::write(&path, contents).expect("test executable should be written");
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755))
                .expect("test executable should be made executable");

            path
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.path).expect("test directory should be removed");
        }
    }
    fn valid_metadata(path: &Path) -> IsoMetadata {
        IsoMetadata::new(
            path.to_path_buf(),
            "Debian".to_owned(),
            "13.1.0".to_owned(),
            "trixie".to_owned(),
            "amd64".to_owned(),
            "netinst".to_owned(),
            vec![BootMode::Bios, BootMode::Uefi],
        )
    }
    fn iso_context(source_iso: &Path, metadata: Option<IsoMetadata>) -> IsoContext {
        IsoContext {
            config: IsoConfig {
                rootfs: PathBuf::from("build/rootfs"),
                source_iso: source_iso.to_path_buf(),
                output_iso: PathBuf::from("build/output.iso"),
                mksquashfs_command: PathBuf::from("mksquashfs"),
                xorriso_command: PathBuf::from("xorriso"),
                layout: Layout::new("build/work/iso"),
                grub: GrubConfig {
                    menu_title: "Debian AI Appliance".to_owned(),
                    timeout: 5,
                    kernel_command_line: "boot=live quiet".to_owned(),
                },
                squashfs: SquashFsConfig {
                    compression: "xz".to_owned(),
                    exclusions: vec!["boot".to_owned()],
                },
            },
            state: IsoState {
                metadata,
                ..IsoState::default()
            },
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
    fn accepts_existing_source_iso_file() {
        let directory = TestDirectory::create();
        let source_iso = directory.create_file("source.iso");
        let context = iso_context(&source_iso, None);

        SourceIsoStage::run(&context).expect("existing source ISO should be accepted");
    }
    #[test]
    fn rejects_source_iso_directory() {
        let directory = TestDirectory::create();
        let context = iso_context(directory.path(), None);

        let error =
            SourceIsoStage::run(&context).expect_err("source ISO directory should be rejected");

        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
    }
    #[test]
    fn rejects_missing_source_iso() {
        let directory = TestDirectory::create();
        let source_iso = directory.path().join("missing.iso");
        let context = iso_context(&source_iso, None);

        let error =
            SourceIsoStage::run(&context).expect_err("missing source ISO should be rejected");

        assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
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
    #[test]
    fn accepts_tool_with_successful_version_command() {
        let directory = TestDirectory::create();
        let tool = directory.create_executable("tool", "#!/bin/sh\nexit 0\n");

        validate_tool(&tool).expect("successful version command should be accepted");
    }
    #[test]
    fn rejects_missing_tool() {
        let directory = TestDirectory::create();
        let tool = directory.path().join("missing-tool");

        let error = validate_tool(&tool).expect_err("missing tool should be rejected");

        assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
    }
    #[test]
    fn rejects_tool_with_failing_version_command() {
        let directory = TestDirectory::create();
        let tool = directory.create_executable("tool", "#!/bin/sh\nexit 1\n");

        let error = validate_tool(&tool).expect_err("failing version command should be rejected");

        assert_eq!(error.kind(), std::io::ErrorKind::Other);
        assert!(error.to_string().contains(&tool.display().to_string()));
    }
    #[test]
    fn rejects_non_executable_tool() {
        let directory = TestDirectory::create();
        let tool = directory.create_file("tool");

        let error = validate_tool(&tool).expect_err("non-executable tool should be rejected");

        assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
    }
    #[test]
    fn accepts_valid_metadata() {
        let source_iso = PathBuf::from("debian.iso");
        let metadata = valid_metadata(&source_iso);
        let context = iso_context(&source_iso, Some(metadata));

        MetadataValidationStage::run(&context).expect("valid metadata should be accepted");
    }
    #[test]
    fn rejects_missing_metadata() {
        let source_iso = PathBuf::from("debian.iso");
        let context = iso_context(&source_iso, None);

        let error = MetadataValidationStage::run(&context)
            .expect_err("missing metadata should be rejected");

        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
    }
    #[test]
    fn rejects_metadata_for_different_source_iso() {
        let source_iso = PathBuf::from("debian.iso");
        let metadata = valid_metadata(Path::new("other.iso"));
        let context = iso_context(&source_iso, Some(metadata));

        let error = MetadataValidationStage::run(&context)
            .expect_err("metadata for another source ISO should be rejected");

        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
    }
    #[test]
    fn rejects_non_debian_distribution() {
        let source_iso = PathBuf::from("debian.iso");
        let metadata = IsoMetadata::new(
            source_iso.clone(),
            "Ubuntu".to_owned(),
            "24.04".to_owned(),
            "noble".to_owned(),
            "amd64".to_owned(),
            "live".to_owned(),
            vec![BootMode::Uefi],
        );
        let context = iso_context(&source_iso, Some(metadata));

        let error = MetadataValidationStage::run(&context)
            .expect_err("non-Debian metadata should be rejected");

        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
    }
    #[test]
    fn rejects_empty_version() {
        let source_iso = PathBuf::from("debian.iso");
        let metadata = IsoMetadata::new(
            source_iso.clone(),
            "Debian".to_owned(),
            String::new(),
            "trixie".to_owned(),
            "amd64".to_owned(),
            "netinst".to_owned(),
            vec![BootMode::Bios, BootMode::Uefi],
        );
        let context = iso_context(&source_iso, Some(metadata));

        let error =
            MetadataValidationStage::run(&context).expect_err("empty version should be rejected");

        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
    }
    #[test]
    fn rejects_whitespace_only_version() {
        let source_iso = PathBuf::from("debian.iso");
        let metadata = IsoMetadata::new(
            source_iso.clone(),
            "Debian".to_owned(),
            "   ".to_owned(),
            "trixie".to_owned(),
            "amd64".to_owned(),
            "netinst".to_owned(),
            vec![BootMode::Bios, BootMode::Uefi],
        );
        let context = iso_context(&source_iso, Some(metadata));

        let error = MetadataValidationStage::run(&context)
            .expect_err("whitespace-only version should be rejected");

        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
    }
    #[test]
    fn rejects_empty_codename() {
        let source_iso = PathBuf::from("debian.iso");
        let metadata = IsoMetadata::new(
            source_iso.clone(),
            "Debian".to_owned(),
            "13.1.0".to_owned(),
            String::new(),
            "amd64".to_owned(),
            "netinst".to_owned(),
            vec![BootMode::Bios, BootMode::Uefi],
        );
        let context = iso_context(&source_iso, Some(metadata));

        let error =
            MetadataValidationStage::run(&context).expect_err("empty codename should be rejected");

        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
    }
    #[test]
    fn rejects_empty_architecture() {
        let source_iso = PathBuf::from("debian.iso");
        let metadata = IsoMetadata::new(
            source_iso.clone(),
            "Debian".to_owned(),
            "13.1.0".to_owned(),
            "trixie".to_owned(),
            String::new(),
            "netinst".to_owned(),
            vec![BootMode::Bios, BootMode::Uefi],
        );
        let context = iso_context(&source_iso, Some(metadata));

        let error = MetadataValidationStage::run(&context)
            .expect_err("empty architecture should be rejected");

        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
    }
    #[test]
    fn rejects_empty_media_type() {
        let source_iso = PathBuf::from("debian.iso");
        let metadata = IsoMetadata::new(
            source_iso.clone(),
            "Debian".to_owned(),
            "13.1.0".to_owned(),
            "trixie".to_owned(),
            "amd64".to_owned(),
            String::new(),
            vec![BootMode::Bios, BootMode::Uefi],
        );
        let context = iso_context(&source_iso, Some(metadata));

        let error = MetadataValidationStage::run(&context)
            .expect_err("empty media type should be rejected");

        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
    }
    #[test]
    fn rejects_empty_boot_modes() {
        let source_iso = PathBuf::from("debian.iso");
        let metadata = IsoMetadata::new(
            source_iso.clone(),
            "Debian".to_owned(),
            "13.1.0".to_owned(),
            "trixie".to_owned(),
            "amd64".to_owned(),
            "netinst".to_owned(),
            Vec::new(),
        );
        let context = iso_context(&source_iso, Some(metadata));

        let error = MetadataValidationStage::run(&context)
            .expect_err("empty boot modes should be rejected");

        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
    }
    struct SuccessfulInspector {
        metadata: IsoMetadata,
    }

    impl IsoInspector for SuccessfulInspector {
        fn inspect(&self, _path: &Path) -> Result<IsoMetadata, InspectError> {
            Ok(self.metadata.clone())
        }
    }

    struct FailingInspector;

    impl IsoInspector for FailingInspector {
        fn inspect(&self, _path: &Path) -> Result<IsoMetadata, InspectError> {
            Err(InspectError::InvalidDiskInfo {
                reason: "test failure",
            })
        }
    }
    #[test]
    fn returns_inspected_metadata() {
        let source_iso = PathBuf::from("debian.iso");
        let expected = valid_metadata(&source_iso);
        let context = iso_context(&source_iso, None);
        let inspector = SuccessfulInspector {
            metadata: expected.clone(),
        };

        let metadata =
            InspectionStage::run(&context, &inspector).expect("inspection should succeed");

        assert_eq!(metadata, expected);
    }
    #[test]
    fn converts_inspection_error_to_io_error() {
        let source_iso = PathBuf::from("debian.iso");
        let context = iso_context(&source_iso, None);

        let error = InspectionStage::run(&context, &FailingInspector)
            .expect_err("inspection failure should return an error");

        assert_eq!(error.kind(), std::io::ErrorKind::Other);
    }
}
