//! ISO build context.

use std::path::PathBuf;

use super::Layout;

/// Immutable configuration for an ISO build.
pub struct IsoConfig {
    pub rootfs: PathBuf,
    pub layout: Layout,
}

/// Shared state passed through the ISO pipeline.
pub struct IsoContext {
    pub config: IsoConfig,
}
