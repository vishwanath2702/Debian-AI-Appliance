//! ISO build context.

use std::path::PathBuf;

use inspector::IsoMetadata;

use super::Layout;

/// GRUB boot menu configuration.
pub struct GrubConfig {
    pub menu_title: String,
    pub timeout: u32,
    pub kernel_command_line: String,
}

/// `SquashFS` build configuration.
pub struct SquashFsConfig {
    pub compression: String,
    pub exclusions: Vec<String>,
}

/// Immutable configuration for an ISO build.
pub struct IsoConfig {
    pub rootfs: PathBuf,
    pub source_iso: PathBuf,
    pub output_iso: PathBuf,
    pub mksquashfs_command: PathBuf,
    pub xorriso_command: PathBuf,
    pub layout: Layout,
    pub grub: GrubConfig,
    pub squashfs: SquashFsConfig,
}

/// Mutable state produced while running the ISO pipeline.
#[derive(Default)]
pub struct IsoState {
    pub metadata: Option<IsoMetadata>,
    pub kernel: Option<PathBuf>,
    pub initramfs: Option<PathBuf>,
    pub squashfs: Option<PathBuf>,
    pub grub_config: Option<PathBuf>,
}
/// Shared state passed through the ISO pipeline.
pub struct IsoContext {
    pub config: IsoConfig,
    pub state: IsoState,
}
