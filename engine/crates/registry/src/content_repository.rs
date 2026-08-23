use serde::Deserialize;
use std::{collections::HashSet, fs, path::Path};

use model::{ContentRepository, ContentRepositoryId};

use crate::RegistryError;

#[derive(Debug, Deserialize)]
struct ContentRepositoryDocument {
    id: String,
    description: String,
}

/// Collection of content repositories known to the DAIA engine.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ContentRepositoryRepository {
    repositories: Vec<ContentRepository>,
}

impl ContentRepositoryRepository {
    /// Creates an empty content repository repository.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            repositories: Vec::new(),
        }
    }

    /// Loads content repositories from YAML.
    ///
    /// # Errors
    ///
    /// Returns [`RegistryError`] if the YAML cannot be parsed or contains
    /// duplicate repository identifiers.
    pub fn load_yaml(yaml: &str) -> Result<Self, RegistryError> {
        let documents: Vec<ContentRepositoryDocument> = serde_yaml::from_str(yaml)?;

        let repositories = documents
            .into_iter()
            .map(|document| ContentRepository::new(document.id, document.description))
            .collect();

        Self::from_repositories(repositories)
    }

    /// Loads content repositories from every YAML file in a directory.
    ///
    /// # Errors
    ///
    /// Returns [`RegistryError`] if the directory cannot be read, a YAML file
    /// cannot be read or parsed, or duplicate repository identifiers are found.
    pub fn load_directory(path: &Path) -> Result<Self, RegistryError> {
        let mut repositories = Vec::new();

        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let path = entry.path();

            if !path.is_file() {
                continue;
            }

            let extension = path.extension().and_then(|extension| extension.to_str());

            if !matches!(extension, Some("yaml" | "yml")) {
                continue;
            }

            let yaml = fs::read_to_string(&path)?;
            let repository = Self::load_yaml(&yaml)?;

            repositories.extend(repository.repositories);
        }

        Self::from_repositories(repositories)
    }

    /// Creates a repository from an existing collection of content repositories.
    ///
    /// # Errors
    ///
    /// Returns [`RegistryError::DuplicateContentRepository`] if two repositories
    /// use the same identifier.
    pub fn from_repositories(repositories: Vec<ContentRepository>) -> Result<Self, RegistryError> {
        let mut ids = HashSet::new();

        for repository in &repositories {
            if !ids.insert(repository.id().clone()) {
                return Err(RegistryError::DuplicateContentRepository(
                    repository.id().clone(),
                ));
            }
        }

        Ok(Self { repositories })
    }
    /// Returns every content repository.
    #[must_use]
    pub fn repositories(&self) -> &[ContentRepository] {
        &self.repositories
    }

    /// Finds a content repository by identifier.
    #[must_use]
    pub fn repository(&self, id: &ContentRepositoryId) -> Option<&ContentRepository> {
        self.repositories
            .iter()
            .find(|repository| repository.id() == id)
    }
}

#[cfg(test)]
mod tests {
    use model::{ContentRepository, ContentRepositoryId};

    use crate::RegistryError;

    use super::ContentRepositoryRepository;

    #[test]
    fn repository_content_repository_directory_contains_local_models() {
        let repository_directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../registry/content-repositories");

        let repository = ContentRepositoryRepository::load_directory(&repository_directory)
            .expect("repository content-repository directory should load");

        let local_models = repository
            .repository(&ContentRepositoryId::new("local-models"))
            .expect("local-models content repository should exist");

        assert_eq!(
            local_models.description(),
            "Models available on local storage"
        );
    }

    #[test]
    fn repository_loads_content_repositories_from_directory() {
        let temp = tempfile::tempdir().expect("temporary directory should be created");

        std::fs::write(
            temp.path().join("models.yaml"),
            r#"
- id: local-models
  description: Models available on local storage
"#,
        )
        .expect("models repository should be written");

        std::fs::write(
            temp.path().join("docs.yml"),
            r#"
- id: offline-docs
  description: Offline documentation
"#,
        )
        .expect("documentation repository should be written");

        std::fs::write(temp.path().join("ignored.txt"), "not yaml")
            .expect("non-YAML file should be written");

        let repository = ContentRepositoryRepository::load_directory(temp.path())
            .expect("content repository directory should load");

        assert_eq!(repository.repositories().len(), 2);

        assert!(
            repository
                .repository(&ContentRepositoryId::new("local-models"))
                .is_some()
        );

        assert!(
            repository
                .repository(&ContentRepositoryId::new("offline-docs"))
                .is_some()
        );
    }

    #[test]
    fn new_repository_is_empty() {
        let repository = ContentRepositoryRepository::new();

        assert!(repository.repositories().is_empty());
    }

    #[test]
    fn repository_loads_content_repositories_from_yaml() {
        let yaml = r#"
- id: local-models
  description: Models available on local storage
- id: offline-docs
  description: Offline documentation
"#;

        let repository =
            ContentRepositoryRepository::load_yaml(yaml).expect("valid content repository YAML");

        assert_eq!(repository.repositories().len(), 2);

        let local_models = repository
            .repository(&ContentRepositoryId::new("local-models"))
            .expect("local-models repository should exist");

        assert_eq!(
            local_models.description(),
            "Models available on local storage"
        );
    }

    #[test]
    fn repository_rejects_duplicate_ids_from_yaml() {
        let yaml = r#"
- id: local-models
  description: First repository
- id: local-models
  description: Second repository
"#;

        let error = ContentRepositoryRepository::load_yaml(yaml)
            .expect_err("duplicate repository identifiers should fail");

        assert!(matches!(
            error,
            RegistryError::DuplicateContentRepository(id)
                if id == ContentRepositoryId::new("local-models")
        ));
    }

    #[test]
    fn repository_can_be_created_from_content_repositories() {
        let content_repository =
            ContentRepository::new("local-models", "Models available on local storage");

        let repository =
            ContentRepositoryRepository::from_repositories(vec![content_repository.clone()])
                .expect("valid content repository");

        assert_eq!(repository.repositories(), &[content_repository]);
    }

    #[test]
    fn duplicate_repository_ids_are_rejected() {
        let repositories = vec![
            ContentRepository::new("local-models", "First repository"),
            ContentRepository::new("local-models", "Second repository"),
        ];

        let error = ContentRepositoryRepository::from_repositories(repositories)
            .expect_err("duplicate repository identifiers should fail");

        assert!(matches!(
            error,
            RegistryError::DuplicateContentRepository(id)
                if id == ContentRepositoryId::new("local-models")
        ));
    }

    #[test]
    fn repository_can_be_found_by_id() {
        let repository = ContentRepositoryRepository::from_repositories(vec![
            ContentRepository::new("local-models", "Models available on local storage"),
            ContentRepository::new("offline-docs", "Offline documentation"),
        ])
        .expect("valid content repository");

        let content_repository = repository
            .repository(&ContentRepositoryId::new("offline-docs"))
            .expect("offline-docs repository should exist");

        assert_eq!(content_repository.description(), "Offline documentation");
    }

    #[test]
    fn repository_returns_none_for_unknown_id() {
        let repository =
            ContentRepositoryRepository::from_repositories(vec![ContentRepository::new(
                "local-models",
                "Models available on local storage",
            )])
            .expect("valid content repository");

        assert!(
            repository
                .repository(&ContentRepositoryId::new("does-not-exist"))
                .is_none()
        );
    }
}
