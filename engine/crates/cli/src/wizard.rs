//! Wizard state for interactive DAIA appliance configuration.
use model::{
    ApplianceConfiguration, ContentImportIntent, ContentRepository, ContentRepositoryId,
    DiscoveredStorage, DiscoveredStorageId, ExternalContentItem, ExternalContentItemId,
    InstallationIntent, StorageKind,
};
use registry::{ApplianceProfileRepository, ContentRepositoryRepository};
/// State accumulated while configuring an appliance through the wizard.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct WizardState {
    profile_name: Option<String>,
    content_repositories: Vec<ContentRepository>,
    selected_content_repository: Option<ContentRepositoryId>,
    external_content_items: Vec<ExternalContentItem>,
    selected_external_content: Vec<ExternalContentItemId>,
    discovered_storage: Vec<DiscoveredStorage>,
    selected_storage: Option<DiscoveredStorageId>,
}
impl WizardState {
    /// Creates an empty wizard state.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            profile_name: None,
            content_repositories: Vec::new(),
            selected_content_repository: None,
            external_content_items: Vec::new(),
            selected_external_content: Vec::new(),
            discovered_storage: Vec::new(),
            selected_storage: None,
        }
    }
    /// Sets the selected appliance profile.
    pub fn set_profile_name(&mut self, profile_name: impl Into<String>) {
        self.profile_name = Some(profile_name.into());
    }

    /// Returns the selected appliance profile name.
    #[must_use]
    pub fn profile_name(&self) -> Option<&str> {
        self.profile_name.as_deref()
    }
    /// Replaces the content repositories available to the wizard.
    pub fn set_content_repositories(&mut self, repositories: Vec<ContentRepository>) {
        if let Some(selected) = self.selected_content_repository.as_ref() {
            let previous_repository = self
                .content_repositories
                .iter()
                .find(|repository| repository.id() == selected);

            let replacement_repository = repositories
                .iter()
                .find(|repository| repository.id() == selected);

            if previous_repository != replacement_repository {
                self.external_content_items.clear();
                self.selected_external_content.clear();
            }

            if replacement_repository.is_none() {
                self.selected_content_repository = None;
            }
        }

        self.content_repositories = repositories;
    }

    /// Returns the content repositories available to the wizard.
    #[must_use]
    pub fn content_repositories(&self) -> &[ContentRepository] {
        &self.content_repositories
    }
    /// Selects a content repository available to the wizard.
    pub fn select_content_repository(&mut self, repository_id: ContentRepositoryId) {
        if self
            .content_repositories
            .iter()
            .any(|repository| repository.id() == &repository_id)
        {
            if self.selected_content_repository.as_ref() != Some(&repository_id) {
                self.external_content_items.clear();
                self.selected_external_content.clear();
            }

            self.selected_content_repository = Some(repository_id);
        }
    }

    /// Returns the selected content repository identifier.
    #[must_use]
    pub fn selected_content_repository(&self) -> Option<&ContentRepositoryId> {
        self.selected_content_repository.as_ref()
    }
    /// Replaces the importable external content items for the current wizard session.
    pub fn set_external_content_items(&mut self, items: Vec<ExternalContentItem>) {
        if self
            .selected_external_content
            .iter()
            .any(|selected| !items.iter().any(|item| item.id() == selected))
        {
            self.selected_external_content.clear();
        }

        self.external_content_items = items;
    }

    /// Returns importable external content items for the current wizard session.
    #[must_use]
    pub fn external_content_items(&self) -> &[ExternalContentItem] {
        &self.external_content_items
    }
    /// Replaces the external content selected for import.
    pub fn select_external_content(&mut self, items: Vec<ExternalContentItemId>) {
        if items.iter().all(|item_id| {
            self.external_content_items
                .iter()
                .any(|item| item.id() == item_id)
        }) {
            self.selected_external_content = items;
        }
    }

    /// Returns the external content selected for import.
    #[must_use]
    pub fn selected_external_content(&self) -> &[ExternalContentItemId] {
        &self.selected_external_content
    }
    /// Replaces the storage discovered for the current system.
    pub fn set_discovered_storage(&mut self, storage: Vec<DiscoveredStorage>) {
        if self.selected_storage.as_ref().is_some_and(|selected| {
            !storage
                .iter()
                .any(|device| device.id() == selected && device.kind() != StorageKind::System)
        }) {
            self.selected_storage = None;
        }

        self.discovered_storage = storage;
    }

    /// Returns storage devices that may be selected as installation targets.
    pub fn selectable_storage(&self) -> impl Iterator<Item = &DiscoveredStorage> {
        self.discovered_storage
            .iter()
            .filter(|storage| storage.kind() != StorageKind::System)
    }

    /// Selects storage by its stable DAIA identifier.
    pub fn select_storage(&mut self, storage_id: DiscoveredStorageId) {
        if self
            .selectable_storage()
            .any(|storage| storage.id() == &storage_id)
        {
            self.selected_storage = Some(storage_id);
        }
    }

    /// Returns the selected storage identifier.
    #[must_use]
    pub const fn selected_storage(&self) -> Option<&DiscoveredStorageId> {
        self.selected_storage.as_ref()
    }

    /// Resolves the selected storage device.
    #[must_use]
    pub fn selected_storage_device(&self) -> Option<&DiscoveredStorage> {
        let selected = self.selected_storage.as_ref()?;

        self.discovered_storage
            .iter()
            .find(|storage| storage.id() == selected)
    }
    /// Converts the completed wizard state into a confirmed configuration.

    pub fn into_config(self) -> Option<WizardConfig> {
        Some(WizardConfig {
            profile_name: self.profile_name?,
            content_repository_id: self.selected_content_repository?,
            external_content: self.selected_external_content,
            storage_id: self.selected_storage?,
        })
    }
}

/// Confirmed wizard configuration ready for planning or execution.
#[derive(Clone, Debug, Eq, PartialEq)]

pub struct WizardConfig {
    profile_name: String,
    content_repository_id: ContentRepositoryId,
    external_content: Vec<ExternalContentItemId>,
    storage_id: DiscoveredStorageId,
}
impl WizardConfig {
    /// Returns the selected appliance profile name.
    #[must_use]
    pub fn profile_name(&self) -> &str {
        &self.profile_name
    }
    /// Returns the selected content repository identifier.
    #[must_use]
    pub const fn content_repository_id(&self) -> &ContentRepositoryId {
        &self.content_repository_id
    }
    /// Resolves the selected content repository from a repository collection.
    #[must_use]
    pub fn content_repository<'a>(
        &self,
        repository: &'a ContentRepositoryRepository,
    ) -> Option<&'a ContentRepository> {
        repository.repository(&self.content_repository_id)
    }
    /// Returns the external content selected for import.
    #[must_use]
    pub fn external_content(&self) -> &[ExternalContentItemId] {
        &self.external_content
    }
    /// Returns the selected storage identifier.
    #[must_use]
    pub const fn storage_id(&self) -> &DiscoveredStorageId {
        &self.storage_id
    }
    /// Resolves the selected appliance profile from a repository.
    #[must_use]
    pub fn profile<'a>(
        &self,
        repository: &'a ApplianceProfileRepository,
    ) -> Option<&'a model::ApplianceProfile> {
        repository.profile(&self.profile_name)
    }
    /// Builds the confirmed appliance configuration.
    #[must_use]
    pub fn appliance_configuration(&self) -> ApplianceConfiguration {
        ApplianceConfiguration::from_selections(
            self.profile_name.clone(),
            self.content_repository_id.clone(),
            self.external_content.clone(),
            self.storage_id.clone(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::WizardState;
    use model::{
        ApplianceProfile, Capability, ContentRepository, ContentRepositoryId, ContentSourceId,
        DiscoveredStorage, DiscoveredStorageId, ExternalContentItem, ExternalContentItemId,
        StorageKind,
    };
    use registry::{ApplianceProfileRepository, ContentRepositoryRepository};

    fn select_test_storage(state: &mut WizardState) {
        state.set_discovered_storage(vec![DiscoveredStorage::new(
            "serial:usb-disk",
            StorageKind::Removable,
            "/dev/sdb",
        )]);
        state.select_storage(DiscoveredStorageId::new("serial:usb-disk"));
    }

    fn select_test_content_repository(state: &mut WizardState) {
        state.set_content_repositories(vec![ContentRepository::new(
            "local-models",
            "Models available on local storage",
        )]);
        state.select_content_repository(ContentRepositoryId::new("local-models"));
    }

    fn select_test_external_content(
        state: &mut WizardState,
        paths: &[&str],
    ) -> Vec<ExternalContentItemId> {
        let source_id = ContentSourceId::new("local-models-directory");

        let items = paths
            .iter()
            .map(|path| ExternalContentItem::new(source_id.clone(), *path))
            .collect::<Vec<_>>();

        let selected_ids = items
            .iter()
            .map(|item| item.id().clone())
            .collect::<Vec<_>>();

        state.set_external_content_items(items);
        state.select_external_content(selected_ids.clone());

        selected_ids
    }

    #[test]
    fn wizard_configuration_builds_appliance_configuration() {
        let item_id =
            ExternalContentItemId::new("local-models-directory:/media/daia/models/model.gguf");

        let mut state = WizardState::new();
        state.set_profile_name("desktop");
        select_test_content_repository(&mut state);
        state.set_external_content_items(vec![ExternalContentItem::new(
            ContentSourceId::new("local-models-directory"),
            "/media/daia/models/model.gguf",
        )]);
        state.select_external_content(vec![item_id.clone()]);
        select_test_storage(&mut state);

        let config = state
            .into_config()
            .expect("completed wizard state should build configuration");

        let appliance = config.appliance_configuration();

        assert_eq!(appliance.profile_name(), "desktop");
        assert_eq!(
            appliance.content_repository_id(),
            &ContentRepositoryId::new("local-models")
        );
        assert_eq!(appliance.content_import().items(), &[item_id]);
        assert_eq!(appliance.installation().profile_name(), "desktop");
        assert_eq!(
            appliance.installation().storage_id(),
            &DiscoveredStorageId::new("serial:usb-disk")
        );
    }

    #[test]
    fn wizard_configuration_resolves_content_repository() {
        let repositories =
            ContentRepositoryRepository::from_repositories(vec![ContentRepository::new(
                "local-models",
                "Models available on local storage",
            )])
            .expect("content repository collection should be valid");

        let mut state = WizardState::new();
        state.set_profile_name("desktop");
        select_test_content_repository(&mut state);
        select_test_storage(&mut state);

        let config = state
            .into_config()
            .expect("completed wizard state should build configuration");

        let repository = config
            .content_repository(&repositories)
            .expect("selected content repository should resolve");

        assert_eq!(repository.id(), config.content_repository_id());
        assert_eq!(
            repository.description(),
            "Models available on local storage"
        );
    }
    #[test]
    fn wizard_configuration_preserves_selected_external_content() {
        let mut state = WizardState::new();

        state.set_profile_name("desktop");
        select_test_content_repository(&mut state);
        select_test_external_content(&mut state, &["/media/daia/models/model.gguf"]);
        state.set_discovered_storage(vec![DiscoveredStorage::new(
            "serial:usb-disk",
            StorageKind::Removable,
            "/dev/sdb",
        )]);
        select_test_storage(&mut state);

        let config = state
            .into_config()
            .expect("completed wizard state should produce configuration");

        assert_eq!(
            config.external_content(),
            &[ExternalContentItemId::new(
                "local-models-directory:/media/daia/models/model.gguf"
            )]
        );
    }

    #[test]
    fn replacing_external_content_items_clears_unavailable_selection() {
        let mut state = WizardState::new();

        state.set_external_content_items(vec![ExternalContentItem::new(
            ContentSourceId::new("local-models-directory"),
            "/media/daia/models/model.gguf",
        )]);
        state.select_external_content(vec![ExternalContentItemId::new(
            "local-models-directory:/media/daia/models/model.gguf",
        )]);

        state.set_external_content_items(vec![ExternalContentItem::new(
            ContentSourceId::new("local-models-directory"),
            "/media/daia/models/tokenizer.json",
        )]);

        assert!(state.selected_external_content().is_empty());
    }

    #[test]
    fn unknown_external_content_cannot_be_selected() {
        let mut state = WizardState::new();

        state.set_external_content_items(vec![ExternalContentItem::new(
            ContentSourceId::new("local-models-directory"),
            "/media/daia/models/model.gguf",
        )]);

        state.select_external_content(vec![ExternalContentItemId::new(
            "local-models-directory:/media/daia/models/does-not-exist.gguf",
        )]);

        assert!(state.selected_external_content().is_empty());
    }

    #[test]
    fn wizard_state_stores_selected_external_content() {
        let mut state = WizardState::new();

        select_test_external_content(
            &mut state,
            &[
                "/media/daia/models/model.gguf",
                "/media/daia/models/tokenizer.json",
            ],
        );

        let selected = state.selected_external_content();

        assert_eq!(selected.len(), 2);
        assert_eq!(
            selected[0],
            ExternalContentItemId::new("local-models-directory:/media/daia/models/model.gguf")
        );
        assert_eq!(
            selected[1],
            ExternalContentItemId::new("local-models-directory:/media/daia/models/tokenizer.json")
        );
    }

    #[test]
    fn wizard_state_stores_external_content_items() {
        let mut state = WizardState::new();

        state.set_external_content_items(vec![ExternalContentItem::new(
            ContentSourceId::new("local-models-directory"),
            "/media/daia/models/model.gguf",
        )]);

        let items = state.external_content_items();

        assert_eq!(items.len(), 1);
        assert_eq!(
            items[0].source_id(),
            &ContentSourceId::new("local-models-directory")
        );
        assert_eq!(
            items[0].path(),
            std::path::Path::new("/media/daia/models/model.gguf")
        );
    }
    #[test]
    fn wizard_state_stores_content_repositories() {
        let mut state = WizardState::new();

        state.set_content_repositories(vec![
            ContentRepository::new("local-models", "Models available on local storage"),
            ContentRepository::new("offline-docs", "Offline documentation"),
        ]);

        let repositories = state.content_repositories();

        assert_eq!(repositories.len(), 2);
        assert_eq!(repositories[0].id().as_str(), "local-models");
        assert_eq!(repositories[1].id().as_str(), "offline-docs");
    }

    #[test]
    fn changing_selected_repository_definition_clears_derived_content() {
        let mut state = WizardState::new();

        let repository_id = ContentRepositoryId::new("local-models");

        state.set_content_repositories(vec![ContentRepository::with_sources(
            "local-models",
            "Models available on local storage",
            vec![model::ContentSource::new(
                "local-models-directory",
                repository_id.clone(),
                "/media/models",
            )],
        )]);
        state.select_content_repository(repository_id.clone());

        state.set_external_content_items(vec![ExternalContentItem::new(
            ContentSourceId::new("local-models-directory"),
            "/media/models/model.gguf",
        )]);

        state.select_external_content(vec![ExternalContentItemId::new(
            "local-models-directory:/media/models/model.gguf",
        )]);

        state.set_content_repositories(vec![ContentRepository::with_sources(
            "local-models",
            "Models available on local storage",
            vec![model::ContentSource::new(
                "local-models-directory",
                repository_id,
                "/mnt/models",
            )],
        )]);

        assert_eq!(
            state
                .selected_content_repository()
                .expect("selected repository should remain available")
                .as_str(),
            "local-models"
        );
        assert!(state.external_content_items().is_empty());
        assert!(state.selected_external_content().is_empty());
    }

    #[test]
    fn replacing_content_repositories_clears_derived_content() {
        let mut state = WizardState::new();

        state.set_content_repositories(vec![ContentRepository::new(
            "local-models",
            "Models available on local storage",
        )]);
        state.select_content_repository(ContentRepositoryId::new("local-models"));

        state.set_external_content_items(vec![ExternalContentItem::new(
            ContentSourceId::new("local-models-directory"),
            "/media/daia/models/model.gguf",
        )]);

        state.select_external_content(vec![ExternalContentItemId::new(
            "local-models-directory:/media/daia/models/model.gguf",
        )]);

        state.set_content_repositories(vec![ContentRepository::new(
            "offline-docs",
            "Offline documentation",
        )]);

        assert_eq!(state.selected_content_repository(), None);
        assert!(state.external_content_items().is_empty());
        assert!(state.selected_external_content().is_empty());
    }

    #[test]
    fn replacing_content_repositories_clears_unavailable_selection() {
        let mut state = WizardState::new();

        state.set_content_repositories(vec![ContentRepository::new(
            "local-models",
            "Models available on local storage",
        )]);
        state.select_content_repository(ContentRepositoryId::new("local-models"));

        state.set_content_repositories(vec![ContentRepository::new(
            "offline-docs",
            "Offline documentation",
        )]);

        assert_eq!(state.selected_content_repository(), None);
    }

    #[test]
    fn unknown_content_repository_cannot_be_selected() {
        let mut state = WizardState::new();

        state.set_content_repositories(vec![ContentRepository::new(
            "local-models",
            "Models available on local storage",
        )]);

        state.select_content_repository(ContentRepositoryId::new("does-not-exist"));

        assert_eq!(state.selected_content_repository(), None);
    }

    #[test]
    fn changing_content_repository_clears_derived_content() {
        let mut state = WizardState::new();

        state.set_content_repositories(vec![
            ContentRepository::new("local-models", "Models available on local storage"),
            ContentRepository::new("offline-docs", "Offline documentation"),
        ]);
        state.select_content_repository(ContentRepositoryId::new("local-models"));

        state.set_external_content_items(vec![ExternalContentItem::new(
            ContentSourceId::new("local-models-directory"),
            "/media/daia/models/model.gguf",
        )]);

        state.select_external_content(vec![ExternalContentItemId::new(
            "local-models-directory:/media/daia/models/model.gguf",
        )]);

        state.select_content_repository(ContentRepositoryId::new("offline-docs"));

        assert!(state.external_content_items().is_empty());
        assert!(state.selected_external_content().is_empty());
    }

    #[test]
    fn wizard_state_stores_selected_content_repository() {
        let mut state = WizardState::new();

        select_test_content_repository(&mut state);

        assert_eq!(
            state
                .selected_content_repository()
                .expect("selected content repository should exist")
                .as_str(),
            "local-models"
        );
    }

    #[test]
    fn wizard_configuration_resolves_appliance_profile() {
        let repository = ApplianceProfileRepository::from_profiles(vec![ApplianceProfile::new(
            "desktop",
            "Graphical desktop appliance",
            vec![Capability::new("desktop")],
        )])
        .expect("profile repository should be valid");

        let mut state = WizardState::new();
        state.set_profile_name("desktop");
        select_test_content_repository(&mut state);
        select_test_storage(&mut state);

        let config = state
            .into_config()
            .expect("completed wizard state should build configuration");

        let profile = config
            .profile(&repository)
            .expect("selected profile should resolve");

        assert_eq!(profile.name(), "desktop");
        assert_eq!(profile.capabilities(), &[Capability::new("desktop")]);
    }

    #[test]
    fn completed_wizard_state_builds_configuration() {
        let mut state = WizardState::new();

        state.set_profile_name("desktop");
        select_test_content_repository(&mut state);
        select_test_storage(&mut state);

        let config = state
            .into_config()
            .expect("completed wizard state should build configuration");

        assert_eq!(config.profile_name(), "desktop");
        assert_eq!(
            config.content_repository_id(),
            &ContentRepositoryId::new("local-models")
        );
        assert_eq!(
            config.storage_id(),
            &DiscoveredStorageId::new("serial:usb-disk")
        );
    }
    #[test]
    fn new_wizard_state_is_empty() {
        let state = WizardState::new();

        assert_eq!(state.selectable_storage().count(), 0);
    }

    #[test]
    fn wizard_state_stores_profile_name() {
        let mut state = WizardState::new();

        state.set_profile_name("desktop");

        assert_eq!(state.profile_name(), Some("desktop"));
    }

    #[test]
    fn wizard_state_stores_selected_storage() {
        let mut state = WizardState::new();

        select_test_storage(&mut state);

        assert_eq!(
            state.selected_storage(),
            Some(&DiscoveredStorageId::new("serial:usb-disk"))
        );
    }

    #[test]
    fn wizard_state_resolves_selected_storage_device() {
        let mut state = WizardState::new();

        state.set_discovered_storage(vec![
            DiscoveredStorage::new("wwn:system-disk", StorageKind::System, "/dev/sda"),
            DiscoveredStorage::new("serial:usb-disk", StorageKind::Removable, "/dev/sdb")
                .with_size_bytes(32_010_928_128),
        ]);
        state.select_storage(DiscoveredStorageId::new("serial:usb-disk"));

        let selected = state
            .selected_storage_device()
            .expect("selected storage device should resolve");

        assert_eq!(selected.id(), &DiscoveredStorageId::new("serial:usb-disk"));
        assert_eq!(selected.device_path(), std::path::Path::new("/dev/sdb"));
        assert_eq!(selected.size_bytes(), Some(32_010_928_128));
    }

    #[test]
    fn wizard_state_stores_discovered_storage() {
        let mut state = WizardState::new();

        state.set_discovered_storage(vec![
            DiscoveredStorage::new("wwn:system-disk", StorageKind::System, "/dev/sda"),
            DiscoveredStorage::new("serial:usb-disk", StorageKind::Removable, "/dev/sdb"),
        ]);

        let selectable = state.selectable_storage().collect::<Vec<_>>();

        assert_eq!(selectable.len(), 1);
        assert_eq!(selectable[0].kind(), StorageKind::Removable);
    }
    #[test]
    fn replacing_discovered_storage_clears_unavailable_selection() {
        let mut state = WizardState::new();

        state.set_discovered_storage(vec![DiscoveredStorage::new(
            "serial:usb-disk",
            StorageKind::Removable,
            "/dev/sdb",
        )]);
        state.select_storage(DiscoveredStorageId::new("serial:usb-disk"));

        state.set_discovered_storage(vec![DiscoveredStorage::new(
            "serial:other-disk",
            StorageKind::Removable,
            "/dev/sdc",
        )]);

        assert_eq!(state.selected_storage(), None);
    }

    #[test]
    fn replacing_discovered_storage_clears_unselectable_selection() {
        let mut state = WizardState::new();

        state.set_discovered_storage(vec![DiscoveredStorage::new(
            "serial:usb-disk",
            StorageKind::Removable,
            "/dev/sdb",
        )]);
        state.select_storage(DiscoveredStorageId::new("serial:usb-disk"));

        state.set_discovered_storage(vec![DiscoveredStorage::new(
            "serial:usb-disk",
            StorageKind::System,
            "/dev/sdb",
        )]);

        assert_eq!(state.selected_storage(), None);
    }

    #[test]
    fn system_storage_cannot_be_selected() {
        let mut state = WizardState::new();

        state.set_discovered_storage(vec![
            DiscoveredStorage::new("wwn:system-disk", StorageKind::System, "/dev/sda"),
            DiscoveredStorage::new("serial:usb-disk", StorageKind::Removable, "/dev/sdb"),
        ]);

        state.select_storage(DiscoveredStorageId::new("wwn:system-disk"));

        assert_eq!(state.selected_storage(), None);
    }

    #[test]
    fn system_storage_is_not_selectable() {
        let mut state = WizardState::new();

        state.set_discovered_storage(vec![
            DiscoveredStorage::new("wwn:system-disk", StorageKind::System, "/dev/sda"),
            DiscoveredStorage::new("wwn:secondary-disk", StorageKind::Secondary, "/dev/sdb"),
            DiscoveredStorage::new("serial:usb-disk", StorageKind::Removable, "/dev/sdc"),
        ]);

        let selectable = state.selectable_storage().collect::<Vec<_>>();

        assert_eq!(selectable.len(), 2);
        assert_eq!(selectable[0].kind(), StorageKind::Secondary);
        assert_eq!(selectable[1].kind(), StorageKind::Removable);
    }
}
