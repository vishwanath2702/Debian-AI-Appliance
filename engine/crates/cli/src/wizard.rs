//! Wizard state for interactive DAIA appliance configuration.
use model::{DiscoveredStorage, StorageKind};

/// State accumulated while configuring an appliance through the wizard.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct WizardState {
    discovered_storage: Vec<DiscoveredStorage>,
}

impl WizardState {
    /// Creates an empty wizard state.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            discovered_storage: Vec::new(),
        }
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
}

#[cfg(test)]
mod tests {
    use model::{DiscoveredStorage, StorageKind};

    use super::WizardState;

    #[test]
    fn new_wizard_state_is_empty() {
        let state = WizardState::new();

        assert_eq!(state.selectable_storage().count(), 0);
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
