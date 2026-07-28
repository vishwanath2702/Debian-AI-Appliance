//! ISO build backend.

mod context;
mod layout;
mod pipeline;
mod stage;

pub use context::{IsoConfig, IsoContext};
pub use layout::Layout;
pub use pipeline::IsoPipeline;
pub use stage::{
    BootArtifactsStage, GrubConfigStage, InitramfsStage, IsoImageStage, KernelStage,
    SourceIsoStage, SquashFsStage, ToolValidationStage, WorkspaceStage,
};
use std::path::{Path, PathBuf};

use executor::ExecuteError;
use model::Plan;

use super::BuildBackend;

/// Backend responsible for producing a bootable ISO image.
pub struct IsoBackend {
    rootfs: PathBuf,
    source_iso: PathBuf,
    work_directory: PathBuf,
    output_path: PathBuf,
    mksquashfs_command: PathBuf,
    xorriso_command: PathBuf,
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
        }
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

    #[cfg(test)]
    fn set_xorriso_command(&mut self, command: impl Into<PathBuf>) {
        self.xorriso_command = command.into();
    }
    #[cfg(test)]
    fn set_mksquashfs_command(&mut self, command: impl Into<PathBuf>) {
        self.mksquashfs_command = command.into();
    }
}

impl BuildBackend for IsoBackend {
    type Error = std::io::Error;

    fn build(&mut self, _plan: &Plan) -> Result<(), ExecuteError<Self::Error>> {
        let context = IsoContext {
            config: IsoConfig {
                rootfs: self.rootfs.clone(),
                source_iso: self.source_iso.clone(),
                output_iso: self.output_path.clone(),
                mksquashfs_command: self.mksquashfs_command.clone(),
                xorriso_command: self.xorriso_command.clone(),
                layout: self.layout(),
            },
        };

        IsoPipeline::run(&context).map_err(ExecuteError::Environment)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, os::unix::fs::PermissionsExt, path::Path};

    use model::{Capability, Plan, ProviderId};
    use tempfile::tempdir;

    use super::*;

    fn create_fake_xorriso(path: &Path) {
        fs::write(
            path,
            concat!(
                "#!/bin/sh\n",
                "while [ \"$#\" -gt 0 ]; do\n",
                "    if [ \"$1\" = \"-outdev\" ]; then\n",
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
                ": > \"$2\"\n",
                "exit 0\n",
            ),
        )
        .unwrap();

        let mut permissions = fs::metadata(path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).unwrap();
    }
    #[test]
    fn build_creates_bootable_iso_workspace_and_output_image() {
        let temp = tempdir().unwrap();
        let source_iso = temp.path().join("source.iso");
        let output_iso = temp.path().join("output").join("image.iso");
        let fake_mksquashfs = temp.path().join("mksquashfs");
        let fake_xorriso = temp.path().join("xorriso");

        fs::write(&source_iso, b"source ISO").unwrap();
        create_fake_mksquashfs(&fake_mksquashfs);
        create_fake_xorriso(&fake_xorriso);
        let mut backend = IsoBackend::new(
            temp.path().join("rootfs"),
            &source_iso,
            temp.path().join("work"),
            &output_iso,
        );

        backend.set_mksquashfs_command(&fake_mksquashfs);
        backend.set_xorriso_command(&fake_xorriso);

        let plan = Plan {
            capability: Capability::new("test"),
            provider: ProviderId::new("test"),
            steps: Vec::new(),
        };

        let boot_directory = backend.rootfs().join("boot");

        fs::create_dir_all(&boot_directory).unwrap();
        fs::write(boot_directory.join("vmlinuz-test"), b"test kernel").unwrap();
        fs::write(boot_directory.join("initrd.img-test"), b"test initramfs").unwrap();

        backend.build(&plan).unwrap();

        let layout = backend.layout();

        assert_eq!(backend.source_iso(), source_iso);
        assert_eq!(backend.output_path(), output_iso);

        assert!(layout.boot_grub().is_dir());
        assert!(layout.efi_boot().is_dir());
        assert!(layout.live().is_dir());
        assert!(layout.staging().is_dir());
        assert!(layout.filesystem_squashfs().is_file());
        assert!(layout.live_kernel().is_file());
        assert!(layout.live_initramfs().is_file());
        assert!(layout.grub_config().is_file());
        assert!(backend.output_path().is_file());

        assert_eq!(fs::read(layout.live_kernel()).unwrap(), b"test kernel");

        assert_eq!(
            fs::read(layout.live_initramfs()).unwrap(),
            b"test initramfs"
        );

        assert_eq!(
            fs::read_to_string(layout.grub_config()).unwrap(),
            concat!(
                "set default=0\n",
                "set timeout=5\n",
                "\n",
                "menuentry \"Debian AI Appliance\" {\n",
                "    linux /live/vmlinuz boot=live quiet\n",
                "    initrd /live/initrd.img\n",
                "}\n",
            )
        );
    }
}
