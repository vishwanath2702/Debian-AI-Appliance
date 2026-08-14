//! Wizard state for interactive DAIA appliance configuration.

use model::DiscoveredStorage;

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

    /// Returns storage discovered for the current system.
    #[must_use]
    pub fn discovered_storage(&self) -> &[DiscoveredStorage] {
        &self.discovered_storage
    }
}

#[cfg(test)]
mod tests {
    use model::{DiscoveredStorage, StorageKind};

    use super::WizardState;

    #[test]
    fn new_wizard_state_is_empty() {
        let state = WizardState::new();

        assert!(state.discovered_storage().is_empty());
    }

    #[test]
    fn wizard_state_stores_discovered_storage() {
        let mut state = WizardState::new();

        state.set_discovered_storage(vec![
            DiscoveredStorage::new("wwn:system-disk", StorageKind::System, "/dev/sda"),
            DiscoveredStorage::new("serial:usb-disk", StorageKind::Removable, "/dev/sdb"),
        ]);

        assert_eq!(state.discovered_storage().len(), 2);
        assert_eq!(state.discovered_storage()[0].kind(), StorageKind::System);
        assert_eq!(state.discovered_storage()[1].kind(), StorageKind::Removable);
    }
}
