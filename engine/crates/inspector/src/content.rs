//! External content discovery abstraction.

use model::{ContentSource, DiscoveredContent};

use crate::ContentInspectError;

/// Discovers external content for a configured DAIA content source.
pub trait ContentInspector {
    /// Inspects a configured content source for available content.
    ///
    /// # Errors
    ///
    /// Returns a [`ContentInspectError`] if the source cannot be inspected.
    fn inspect(
        &self,
        source: &ContentSource,
    ) -> Result<Vec<DiscoveredContent>, ContentInspectError>;
}
