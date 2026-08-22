//! ISO build pipeline.

use std::io;

use inspector::IsoInspector;

use super::{
    BootArtifactsStage, GrubConfigStage, GrubRescueStage, InitramfsStage, InspectionStage,
    IsoContext, KernelStage, MetadataValidationStage, SourceIsoStage, SquashFsStage,
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

        let iso_image = GrubRescueStage::run(context)?;
        context.state.iso_image = Some(iso_image);

        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use std::{
        fs, io,
        os::unix::fs::PermissionsExt,
        path::{Path, PathBuf},
    };

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
    struct FailingIsoInspector;

    impl IsoInspector for FailingIsoInspector {
        fn inspect(&self, _source_iso: &Path) -> Result<IsoMetadata, InspectError> {
            Err(InspectError::Io(io::Error::other("inspection failed")))
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
    if [ "$1" = "-o" ]; then
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
                grub_mkrescue_command: PathBuf::from("grub-mkrescue"),
                layout: Layout::new(temp),
                grub: GrubConfig {
                    menu_title: "Debian AI Appliance".to_owned(),
                    timeout: 5,
                    kernel_command_line: "boot=live components username=daia user-fullname=\"DAIA Live User\" hostname=daia quiet".to_owned(),
                },
                squashfs: SquashFsConfig {
                    compression: "xz".to_owned(),
                    exclusions: vec!["boot".to_owned()],
                },
            },
            state: IsoState::default(),
        }
    }
    fn create_failing_mksquashfs(path: &Path) {
        create_executable(
            path,
            r#"#!/bin/sh
if [ "$1" = "--version" ]; then
    echo "mksquashfs test version"
    exit 0
fi

echo "squashfs failed" >&2
exit 1
"#,
        );
    }
    fn create_failing_grub_mkrescue(path: &Path) {
        create_executable(
            path,
            r#"#!/bin/sh
if [ "$1" = "--version" ]; then
    echo "grub-mkrescue test version"
    exit 0
fi

echo "grub failed" >&2
exit 1
"#,
        );
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
    #[test]
    fn inspection_failure_stops_pipeline() {
        let temp = tempdir().expect("temporary directory should be created");
        let mut context = create_test_context(temp.path());

        let error = IsoPipeline::run(&mut context, &FailingIsoInspector)
            .expect_err("inspection failure should stop pipeline");

        assert!(context.state.metadata.is_none());
        assert!(context.state.kernel.is_none());
        assert!(context.state.initramfs.is_none());
        assert!(context.state.squashfs.is_none());
        assert!(context.state.grub_config.is_none());
        assert!(context.state.iso_image.is_none());

        assert!(error.to_string().contains("inspection"));
    }
    #[test]
    fn squashfs_failure_stops_before_iso_creation() {
        let temp = tempdir().expect("temporary directory should be created");
        let mut context = create_test_context(temp.path());

        create_failing_mksquashfs(&context.config.mksquashfs_command);

        let error = IsoPipeline::run(&mut context, &TestIsoInspector)
            .expect_err("squashfs failure should stop pipeline");

        assert!(context.state.kernel.is_some());
        assert!(context.state.initramfs.is_some());

        assert!(context.state.squashfs.is_none());
        assert!(context.state.grub_config.is_none());
        assert!(context.state.iso_image.is_none());

        assert!(error.to_string().contains("mksquashfs"));
    }
    #[test]
    fn grub_rescue_failure_stops_pipeline() {
        let temp = tempdir().expect("temporary directory should be created");
        let mut context = create_test_context(temp.path());

        let failing_grub = temp.path().join("grub-mkrescue");
        create_failing_grub_mkrescue(&failing_grub);

        context.config.grub_mkrescue_command = failing_grub;

        let error = IsoPipeline::run(&mut context, &TestIsoInspector)
            .expect_err("grub-mkrescue failure should stop pipeline");

        assert!(context.state.kernel.is_some());
        assert!(context.state.initramfs.is_some());
        assert!(context.state.squashfs.is_some());
        assert!(context.state.grub_config.is_some());

        assert!(context.state.iso_image.is_none());

        assert!(error.to_string().contains("grub"));
    }
}
