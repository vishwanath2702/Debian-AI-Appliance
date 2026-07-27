//! ISO build context.

use super::Layout;

/// Immutable configuration for an ISO build.
pub struct IsoConfig {
    pub layout: Layout,
}

/// Shared state passed through the ISO pipeline.
pub struct IsoContext {
    pub config: IsoConfig,
}
