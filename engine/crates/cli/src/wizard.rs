//! Wizard state for interactive DAIA appliance configuration.
use model::{
    ContentRepository, ContentRepositoryId, DiscoveredStorage, DiscoveredStorageId,
    InstallationIntent, StorageKind,
};
use registry::ApplianceProfileRepository;
/// State accumulated while configuring an appliance through the wizard.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct WizardState {
    profile_name: Option<String>,
    content_repositories: Vec<ContentRepository>,
    selected_content_repository: Option<ContentRepositoryId>,
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
        self.content_repositories = repositories;
    }

    /// Returns the content repositories available to the wizard.
    #[must_use]
    pub fn content_repositories(&self) -> &[ContentRepository] {
        &self.content_repositories
    }
    /// Selects a content repository available to the wizard.
    pub fn select_content_repository(&mut self, repository_id: ContentRepositoryId) {
        self.selected_content_repository = Some(repository_id);
    }

    /// Returns the selected content repository identifier.
    #[must_use]
    pub fn selected_content_repository(&self) -> Option<&ContentRepositoryId> {
        self.selected_content_repository.as_ref()
    }
    /// Replaces the storage discovered for the current system.
    pub fn set_discovered_storage(&mut self, storage: Vec<DiscoveredStorage>) {
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
        self.selected_storage = Some(storage_id);
    }

    /// Returns the selected storage identifier.
    #[must_use]
    pub const fn selected_storage(&self) -> Option<&DiscoveredStorageId> {
        self.selected_storage.as_ref()
    }
    /// Converts the completed wizard state into a confirmed configuration.
    pub fn into_config(self) -> Option<WizardConfig> {
        Some(WizardConfig {
            profile_name: self.profile_name?,
            storage_id: self.selected_storage?,
        })
    }
}
/// Confirmed wizard configuration ready for planning or execution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WizardConfig {
    profile_name: String,
    storage_id: DiscoveredStorageId,
}

impl WizardConfig {
    /// Returns the selected appliance profile name.
    #[must_use]
    pub fn profile_name(&self) -> &str {
        &self.profile_name
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
    /// Converts the confirmed wizard configuration into installation intent.
    #[must_use]
    pub fn installation_intent(&self) -> InstallationIntent {
        InstallationIntent::new(self.profile_name.clone(), self.storage_id().clone())
    }
}

#[cfg(test)]
mod tests {
    use super::WizardState;
    use model::{
        ApplianceProfile, Capability, ContentRepository, ContentRepositoryId, DiscoveredStorage,
        DiscoveredStorageId, StorageKind,
    };
    use registry::ApplianceProfileRepository;

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
    fn wizard_state_stores_selected_content_repository() {
        let mut state = WizardState::new();

        state.select_content_repository(ContentRepositoryId::new("local-models"));

        assert_eq!(
            state
                .selected_content_repository()
                .expect("selected content repository should exist")
                .as_str(),
            "local-models"
        );
    }

    #[test]
    fn wizard_configuration_builds_installation_intent() {
        let mut state = WizardState::new();

        state.set_profile_name("desktop");
        state.select_storage(DiscoveredStorageId::new("serial:usb-disk"));

        let config = state
            .into_config()
            .expect("completed wizard state should build configuration");

        let intent = config.installation_intent();

        assert_eq!(intent.profile_name(), "desktop");
        assert_eq!(
            intent.storage_id(),
            &DiscoveredStorageId::new("serial:usb-disk")
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
        state.select_storage(DiscoveredStorageId::new("serial:usb-disk"));

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
        state.select_storage(DiscoveredStorageId::new("serial:usb-disk"));

        let config = state
            .into_config()
            .expect("completed wizard state should build configuration");

        assert_eq!(config.profile_name(), "desktop");
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

        state.select_storage(DiscoveredStorageId::new("serial:usb-disk"));

        assert_eq!(
            state.selected_storage(),
            Some(&DiscoveredStorageId::new("serial:usb-disk"))
        );
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
