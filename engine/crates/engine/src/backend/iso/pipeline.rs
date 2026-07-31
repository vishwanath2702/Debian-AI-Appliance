//! ISO build pipeline.

use std::io;

use inspector::IsoInspector;

use super::{
    BootArtifactsStage, GrubConfigStage, InitramfsStage, InspectionStage, IsoContext,
    IsoImageStage, KernelStage, MetadataValidationStage, SourceIsoStage, SquashFsStage,
    ToolValidationStage, WorkspaceStage,
};
/// Coordinates the ISO build process.
pub struct IsoPipeline;

impl IsoPipeline {
    /// Runs the ISO build pipeline.
    ///
    /// # Errors
    ///
    /// Returns an error if an ISO build stage fails.
    pub fn run(context: &mut IsoContext, inspector: &dyn IsoInspector) -> io::Result<()> {
        SourceIsoStage::run(context)?;
        let metadata = InspectionStage::run(context, inspector)?;
        context.state.metadata = Some(metadata);
        MetadataValidationStage::run(context)?;
        ToolValidationStage::run(context)?;
        WorkspaceStage::run(context)?;

        let kernel = KernelStage::run(context)?;
        let initramfs = InitramfsStage::run(context)?;

        BootArtifactsStage::run(context, &kernel, &initramfs)?;

        context.state.kernel = Some(kernel);
        context.state.initramfs = Some(initramfs);

        let squashfs = SquashFsStage::run(context)?;
        context.state.squashfs = Some(squashfs);
        let grub_config = GrubConfigStage::run(context)?;
        context.state.grub_config = Some(grub_config);
        let iso_image = IsoImageStage::run(context)?;
        context.state.iso_image = Some(iso_image);
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use std::{fs, os::unix::fs::PermissionsExt, path::Path};

    use inspector::{BootMode, InspectError, IsoInspector, IsoMetadata};
    use tempfile::tempdir;

    use super::IsoPipeline;
    use crate::backend::iso::{
        GrubConfig, IsoConfig, IsoContext, IsoState, Layout, SquashFsConfig,
    };
    struct TestIsoInspector;

    impl IsoInspector for TestIsoInspector {
        fn inspect(&self, source_iso: &Path) -> Result<IsoMetadata, InspectError> {
            Ok(IsoMetadata::new(
                source_iso.to_path_buf(),
                "Debian".to_owned(),
                "13.1.0".to_owned(),
                "trixie".to_owned(),
                "amd64".to_owned(),
                "netinst".to_owned(),
                vec![BootMode::Bios, BootMode::Uefi],
            ))
        }
    }

    fn create_executable(path: &Path, contents: &str) {
        fs::write(path, contents).expect("test executable should be written");
        fs::set_permissions(path, fs::Permissions::from_mode(0o755))
            .expect("test executable should be executable");
    }

    fn create_test_context(temp: &std::path::Path) -> IsoContext {
        let source_iso = temp.join("source.iso");
        let output_iso = temp.join("output.iso");
        let rootfs = temp.join("rootfs");

        fs::write(&source_iso, b"source").expect("source ISO should exist");

        let boot = rootfs.join("boot");
        fs::create_dir_all(&boot).expect("boot directory should exist");

        fs::write(boot.join("vmlinuz-test"), b"kernel").expect("kernel should exist");

        fs::write(boot.join("initrd.img-test"), b"initramfs").expect("initramfs should exist");

        let mksquashfs = temp.join("mksquashfs");
        create_executable(
            &mksquashfs,
            r#"#!/bin/sh
if [ "$1" = "--version" ]; then
    echo "mksquashfs test version"
    exit 0
fi

printf squashfs > "$2"
exit 0
"#,
        );
        let xorriso = temp.join("xorriso");
        create_executable(
            &xorriso,
            r#"#!/bin/sh
if [ "$1" = "--version" ]; then
    echo "xorriso test version"
    exit 0
fi

while [ "$#" -gt 0 ]; do
    if [ "$1" = "-outdev" ]; then
        shift
        printf iso > "$1"
        exit 0
    fi
    shift
done

exit 1
"#,
        );
        IsoContext {
            config: IsoConfig {
                rootfs,
                source_iso,
                output_iso,
                mksquashfs_command: mksquashfs,
                xorriso_command: xorriso,
                layout: Layout::new(temp),
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
            state: IsoState::default(),
        }
    }
    #[test]
    fn pipeline_populates_build_state() {
        let temp = tempdir().expect("temporary directory should be created");
        let mut context = create_test_context(temp.path());

        IsoPipeline::run(&mut context, &TestIsoInspector).expect("ISO pipeline should complete");

        assert!(context.state.metadata.is_some());
        assert!(context.state.kernel.is_some());
        assert!(context.state.initramfs.is_some());
        assert!(context.state.squashfs.is_some());
        assert!(context.state.grub_config.is_some());
        assert!(context.state.iso_image.is_some());

        assert_eq!(
            fs::read(context.state.kernel.as_ref().unwrap()).expect("kernel should be readable"),
            b"kernel"
        );

        assert_eq!(
            fs::read(context.state.initramfs.as_ref().unwrap())
                .expect("initramfs should be readable"),
            b"initramfs"
        );
    }
}
