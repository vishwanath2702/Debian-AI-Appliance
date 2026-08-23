use std::collections::HashSet;

use model::{ContentRepository, ContentRepositoryId};

use crate::RegistryError;

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
    fn new_repository_is_empty() {
        let repository = ContentRepositoryRepository::new();

        assert!(repository.repositories().is_empty());
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
