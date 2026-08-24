//! Local filesystem content discovery.

use std::path::Path;

use model::{ContentSource, DiscoveredContent, ExternalContentItem};

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
    fn items(
        &self,
        content: &DiscoveredContent,
    ) -> Result<Vec<ExternalContentItem>, ContentInspectError> {
        let mut items = Vec::new();

        for entry in std::fs::read_dir(content.path())? {
            let entry = entry?;

            if !entry.file_type()?.is_file() {
                continue;
            }

            items.push(ExternalContentItem::new(
                content.source_id().clone(),
                entry.path(),
            ));
        }

        Ok(items)
    }
}

#[cfg(test)]
mod tests {
    use model::{ContentRepositoryId, ContentSource};
    use tempfile::tempdir;

    use super::LocalFilesystemContentInspector;
    use crate::ContentInspector;

    #[test]
    fn enumerates_immediate_regular_files() {
        let directory = tempdir().expect("temporary directory should be created");

        let first = directory.path().join("model.gguf");
        let second = directory.path().join("manual.pdf");
        let nested = directory.path().join("nested");

        std::fs::write(&first, "model").expect("first content file should be written");
        std::fs::write(&second, "manual").expect("second content file should be written");
        std::fs::create_dir(&nested).expect("nested directory should be created");
        std::fs::write(nested.join("ignored.txt"), "nested")
            .expect("nested content file should be written");

        let source = ContentSource::new(
            "local-models",
            ContentRepositoryId::new("models"),
            directory.path().to_string_lossy(),
        );

        let inspector = LocalFilesystemContentInspector::new();

        let discovered = inspector
            .inspect(&source)
            .expect("local content inspection should succeed");

        let mut items = inspector
            .items(&discovered[0])
            .expect("local content enumeration should succeed");

        items.sort_by(|left, right| left.path().cmp(right.path()));

        let mut expected = vec![first, second];
        expected.sort();

        assert_eq!(items.len(), 2);
        assert_eq!(items[0].source_id(), source.id());
        assert_eq!(items[1].source_id(), source.id());
        assert_eq!(items[0].path(), expected[0].as_path());
        assert_eq!(items[1].path(), expected[1].as_path());
    }
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
