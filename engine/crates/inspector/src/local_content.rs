//! Local filesystem content discovery.

use std::path::Path;

use model::{ContentSource, DiscoveredContent};

use crate::{ContentInspectError, ContentInspector};

/// Discovers content from local filesystem paths.
#[derive(Clone, Copy, Debug, Default)]
pub struct LocalFilesystemContentInspector;

impl LocalFilesystemContentInspector {
    /// Creates a local filesystem content inspector.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl ContentInspector for LocalFilesystemContentInspector {
    fn inspect(
        &self,
        source: &ContentSource,
    ) -> Result<Vec<DiscoveredContent>, ContentInspectError> {
        let path = Path::new(source.locator());

        if !path.exists() {
            return Ok(Vec::new());
        }

        Ok(vec![DiscoveredContent::new(
            source.id().clone(),
            path.to_path_buf(),
        )])
    }
}

#[cfg(test)]
mod tests {
    use model::{ContentRepositoryId, ContentSource};
    use tempfile::tempdir;

    use super::LocalFilesystemContentInspector;
    use crate::ContentInspector;

    #[test]
    fn discovers_existing_local_content_path() {
        let directory = tempdir().expect("temporary directory should be created");

        let source = ContentSource::new(
            "local-models",
            ContentRepositoryId::new("models"),
            directory.path().to_string_lossy(),
        );

        let inspector = LocalFilesystemContentInspector::new();

        let discovered = inspector
            .inspect(&source)
            .expect("local content inspection should succeed");

        assert_eq!(discovered.len(), 1);
        assert_eq!(discovered[0].source_id(), source.id());
        assert_eq!(discovered[0].path(), directory.path());
    }

    #[test]
    fn missing_local_content_path_is_not_discovered() {
        let directory = tempdir().expect("temporary directory should be created");
        let missing = directory.path().join("missing");

        let source = ContentSource::new(
            "local-models",
            ContentRepositoryId::new("models"),
            missing.to_string_lossy(),
        );

        let inspector = LocalFilesystemContentInspector::new();

        let discovered = inspector
            .inspect(&source)
            .expect("missing local content path should not fail inspection");

        assert!(discovered.is_empty());
    }
}
