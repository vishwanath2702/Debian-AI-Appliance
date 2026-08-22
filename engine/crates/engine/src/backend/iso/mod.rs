//! ISO build backend.

mod context;
mod layout;
mod pipeline;
mod stage;

pub use context::{GrubConfig, IsoConfig, IsoContext, IsoState, SquashFsConfig};
pub use layout::Layout;
pub use pipeline::IsoPipeline;
pub use stage::{
    BootArtifactsStage, GrubConfigStage, GrubRescueStage, InitramfsStage, InspectionStage,
    KernelStage, MetadataValidationStage, SourceIsoStage, SquashFsStage, ToolValidationStage,
    WorkspaceStage,
};
use std::path::{Path, PathBuf};

use executor::ExecuteError;
use inspector::{DebianIsoInspector, IsoInspector};
use model::Plan;

use super::BuildBackend;
use crate::BuildContext;
/// Backend responsible for producing a bootable ISO image.
pub struct IsoBackend {
    rootfs: PathBuf,
    source_iso: PathBuf,
    work_directory: PathBuf,
    output_path: PathBuf,
    mksquashfs_command: PathBuf,
    xorriso_command: PathBuf,
    grub_mkrescue_command: PathBuf,
    grub: GrubConfig,
    squashfs: SquashFsConfig,
    inspector: Box<dyn IsoInspector>,
}
impl IsoBackend {
    /// Creates an ISO backend.
    #[must_use]
    pub fn new(
        rootfs: impl Into<PathBuf>,
        source_iso: impl Into<PathBuf>,
        work_directory: impl Into<PathBuf>,
        output_path: impl Into<PathBuf>,
    ) -> Self {
        Self {
            rootfs: rootfs.into(),
            source_iso: source_iso.into(),
            work_directory: work_directory.into(),
            output_path: output_path.into(),
            mksquashfs_command: PathBuf::from("mksquashfs"),
            xorriso_command: PathBuf::from("xorriso"),
            grub_mkrescue_command: PathBuf::from("grub-mkrescue"),
            grub: GrubConfig {
                menu_title: "Debian AI Appliance".to_owned(),
                timeout: 5,
kernel_command_line:
    "boot=live components username=daia user-fullname=\"DAIA Live User\" hostname=daia quiet"
        .to_owned(),
            },
            squashfs: SquashFsConfig {
                compression: "xz".to_owned(),
                exclusions: vec!["boot".to_owned()],
            },
            inspector: Box::new(DebianIsoInspector::new()),
        }
    }
    /// Creates an ISO backend from a shared build context.
    #[must_use]
    pub fn from_context(context: &BuildContext) -> Self {
        Self::new(
            context.rootfs(),
            context.source_iso(),
            context.work_directory(),
            context.output_iso(),
        )
    }
    /// Returns the prepared root filesystem path.
    #[must_use]
    pub fn rootfs(&self) -> &Path {
        &self.rootfs
    }

    /// Returns the source ISO path.
    #[must_use]
    pub fn source_iso(&self) -> &Path {
        &self.source_iso
    }

    /// Returns the ISO working-directory path.
    #[must_use]
    pub fn work_directory(&self) -> &Path {
        &self.work_directory
    }

    /// Returns the final ISO output path.
    #[must_use]
    pub fn output_path(&self) -> &Path {
        &self.output_path
    }

    /// Returns the layout of the intermediate ISO filesystem tree.
    #[must_use]
    pub fn layout(&self) -> Layout {
        Layout::new(self.work_directory.join("iso"))
    }
    /// Uses a custom `xorriso` executable.
    #[must_use]
    pub fn with_xorriso_command(mut self, command: impl Into<PathBuf>) -> Self {
        self.xorriso_command = command.into();
        self
    }

    /// Uses a custom `mksquashfs` executable.
    #[must_use]
    pub fn with_mksquashfs_command(mut self, command: impl Into<PathBuf>) -> Self {
        self.mksquashfs_command = command.into();
        self
    }

    /// Uses custom GRUB configuration.
    #[must_use]
    pub fn with_grub_config(mut self, config: GrubConfig) -> Self {
        self.grub = config;
        self
    }

    /// Uses custom `SquashFS` configuration.
    #[must_use]
    pub fn with_squashfs_config(mut self, config: SquashFsConfig) -> Self {
        self.squashfs = config;
        self
    }
    /// Uses a custom ISO inspector.
    #[must_use]
    pub fn with_iso_inspector(mut self, inspector: impl IsoInspector + 'static) -> Self {
        self.inspector = Box::new(inspector);
        self
    }
}

impl BuildBackend for IsoBackend {
    type Error = std::io::Error;

    fn build(&mut self, _plan: &Plan) -> Result<(), ExecuteError<Self::Error>> {
        let mut context = IsoContext {
            config: IsoConfig {
                rootfs: self.rootfs.clone(),
                source_iso: self.source_iso.clone(),
                output_iso: self.output_path.clone(),
                mksquashfs_command: self.mksquashfs_command.clone(),
                xorriso_command: self.xorriso_command.clone(),
                grub_mkrescue_command: self.grub_mkrescue_command.clone(),
                layout: self.layout(),
                grub: GrubConfig {
                    menu_title: self.grub.menu_title.clone(),
                    timeout: self.grub.timeout,
                    kernel_command_line: self.grub.kernel_command_line.clone(),
                },
                squashfs: SquashFsConfig {
                    compression: self.squashfs.compression.clone(),
                    exclusions: self.squashfs.exclusions.clone(),
                },
            },
            state: IsoState::default(),
        };
        IsoPipeline::run(&mut context, self.inspector.as_ref())
            .map_err(ExecuteError::Environment)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use inspector::{BootMode, InspectError, IsoInspector, IsoMetadata};
    use model::{Capability, Plan, ProviderId};
    use std::{fs, os::unix::fs::PermissionsExt, path::Path};
    use tempfile::{TempDir, tempdir};

    use super::*;
    use crate::{BootstrapConfig, BuildContext};
    struct TestIsoInspector;

    impl IsoInspector for TestIsoInspector {
        fn inspect(&self, path: &Path) -> Result<IsoMetadata, InspectError> {
            Ok(IsoMetadata::new(
                path.to_path_buf(),
                "Debian".to_owned(),
                "13.1.0".to_owned(),
                "trixie".to_owned(),
                "amd64".to_owned(),
                "netinst".to_owned(),
                vec![BootMode::Bios, BootMode::Uefi],
            ))
        }
    }

    fn create_fake_xorriso(path: &Path) {
        fs::write(
            path,
            concat!(
                "#!/bin/sh\n",
                "if [ \"$1\" = \"--version\" ]; then\n",
                "    exit 0\n",
                "fi\n",
                "while [ \"$#\" -gt 0 ]; do\n",
                "    if [ \"$1\" = \"-o\" ]; then\n",
                "        shift\n",
                "        : > \"$1\"\n",
                "        exit 0\n",
                "    fi\n",
                "    shift\n",
                "done\n",
                "exit 2\n",
            ),
        )
        .unwrap();

        let mut permissions = fs::metadata(path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).unwrap();
    }
    fn create_fake_mksquashfs(path: &Path) {
        fs::write(
            path,
            concat!(
                "#!/bin/sh\n",
                "if [ \"$1\" = \"--version\" ]; then\n",
                "    exit 0\n",
                "fi\n",
                "if [ \"$#\" -lt 2 ]; then\n",
                "    exit 2\n",
                "fi\n",
                "printf '%s\\n' \"$@\" > \"${0}.args\"\n",
                ": > \"$2\"\n",
                "exit 0\n",
            ),
        )
        .unwrap();

        let mut permissions = fs::metadata(path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).unwrap();
    }
    fn create_test_backend() -> (TempDir, IsoBackend) {
        let temp = tempdir().unwrap();
        let source_iso = temp.path().join("source.iso");
        let output_iso = temp.path().join("output").join("image.iso");
        let fake_mksquashfs = temp.path().join("mksquashfs");
        let fake_xorriso = temp.path().join("xorriso");

        fs::write(&source_iso, b"source ISO").unwrap();
        create_fake_mksquashfs(&fake_mksquashfs);
        create_fake_xorriso(&fake_xorriso);

        let backend = IsoBackend::new(
            temp.path().join("rootfs"),
            &source_iso,
            temp.path().join("work"),
            &output_iso,
        )
        .with_mksquashfs_command(&fake_mksquashfs)
        .with_xorriso_command(&fake_xorriso)
        .with_iso_inspector(TestIsoInspector);

        let boot_directory = backend.rootfs().join("boot");

        fs::create_dir_all(&boot_directory).unwrap();
        fs::write(boot_directory.join("vmlinuz-test"), b"test kernel").unwrap();
        fs::write(boot_directory.join("initrd.img-test"), b"test initramfs").unwrap();

        (temp, backend)
    }

    fn test_plan() -> Plan {
        Plan {
            capability: Capability::new("test"),
            provider: ProviderId::new("test"),
            steps: Vec::new(),
        }
    }
    #[test]
    fn creates_backend_from_build_context() {
        let bootstrap = BootstrapConfig::new(
            "bookworm",
            "amd64",
            "https://deb.debian.org/debian",
            vec!["main".to_owned()],
            "minbase",
        );

        let context = BuildContext::new(
            "build/rootfs",
            "images/source.iso",
            "build/work",
            "build/output.iso",
            "registry/assets",
            bootstrap,
        );
        let backend = IsoBackend::from_context(&context);

        assert_eq!(backend.rootfs(), context.rootfs());
        assert_eq!(backend.source_iso(), context.source_iso());
        assert_eq!(backend.work_directory(), context.work_directory());
        assert_eq!(backend.output_path(), context.output_iso());
    }
    #[test]
    fn build_creates_bootable_iso_workspace_and_output_image() {
        let (_temp, mut backend) = create_test_backend();

        backend
            .build(&test_plan())
            .expect("ISO build should succeed");
        let layout = backend.layout();

        assert!(backend.output_path().is_file());
        assert!(layout.boot_grub().is_dir());
        assert!(layout.efi_boot().is_dir());
        assert!(layout.live().is_dir());
        assert!(layout.staging().is_dir());
        assert!(layout.filesystem_squashfs().is_file());
        assert!(layout.live_kernel().is_file());
        assert!(layout.live_initramfs().is_file());
        assert!(layout.grub_config().is_file());

        let grub_contents =
            fs::read_to_string(layout.grub_config()).expect("GRUB config should be readable");

        assert!(grub_contents.contains("set default=0"));
        assert!(grub_contents.contains("set timeout=5"));
        assert!(grub_contents.contains("menuentry \"Debian AI Appliance\""));
        assert!(grub_contents.contains(
    "linux /live/vmlinuz boot=live components username=daia user-fullname=\"DAIA Live User\" hostname=daia quiet"
));
        assert!(grub_contents.contains("initrd /live/initrd.img"));
    }
    #[test]
    fn build_uses_custom_squashfs_configuration() {
        let (temp, backend) = create_test_backend();
        let mut backend = backend.with_squashfs_config(SquashFsConfig {
            compression: "gzip".to_owned(),
            exclusions: vec![
                "boot".to_owned(),
                "usr/share/doc".to_owned(),
                "var/cache".to_owned(),
            ],
        });

        backend.build(&test_plan()).unwrap();

        assert_eq!(
            fs::read_to_string(temp.path().join("mksquashfs.args")).unwrap(),
            format!(
                "{}\n{}\n-comp\ngzip\n-noappend\n-e\nboot\nusr/share/doc\nvar/cache\n",
                backend.rootfs().display(),
                backend.layout().filesystem_squashfs().display(),
            )
        );
    }
}
