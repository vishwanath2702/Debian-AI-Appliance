use model::{Action, Capability, CapabilityId, PlanStep, Provider, ProviderId};

/// Collection of providers known to the DAIA engine.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Registry {
    providers: Vec<Provider>,
}

impl Registry {
    /// Creates an empty provider registry.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            providers: Vec::new(),
        }
    }

    /// Creates a registry containing the built-in DAIA providers.
    #[must_use]
    pub fn built_in() -> Self {
        Self {
            providers: vec![desktop_provider()],
        }
    }

    /// Creates a registry from an existing collection of providers.
    #[must_use]
    pub const fn from_providers(providers: Vec<Provider>) -> Self {
        Self { providers }
    }

    /// Returns every provider in the registry.
    #[must_use]
    pub fn providers(&self) -> &[Provider] {
        &self.providers
    }

    /// Finds a provider that supplies the requested capability.
    #[must_use]
    pub fn provider_for(
        &self,
        capability: &Capability,
    ) -> Option<&Provider> {
        self.provider_for_id(capability.id())
    }

    /// Finds a provider that supplies the requested capability identifier.
    #[must_use]
    pub fn provider_for_id(
        &self,
        capability_id: &CapabilityId,
    ) -> Option<&Provider> {
        self.providers
            .iter()
            .find(|provider| &provider.capability == capability_id)
    }
}

/// Creates the built-in desktop provider.
fn desktop_provider() -> Provider {
    Provider {
        id: ProviderId::new("desktop"),
        capability: CapabilityId::new("desktop"),
        steps: vec![
            PlanStep::new(Action::InstallPackageManifest(
                "desktop".to_owned(),
            )),
            PlanStep::new(Action::EnableService(
                "display-manager".to_owned(),
            )),
        ],
    }
}

#[cfg(test)]
mod tests {
    use model::{
        Action, Capability, CapabilityId, PlanStep, Provider,
        ProviderId,
    };

    use super::Registry;

    #[test]
    fn new_registry_is_empty() {
        let registry = Registry::new();

        assert!(registry.providers().is_empty());
    }

    #[test]
    fn built_in_registry_contains_desktop_provider() {
        let registry = Registry::built_in();

        assert_eq!(registry.providers().len(), 1);

        let provider = &registry.providers()[0];

        assert_eq!(provider.id, ProviderId::new("desktop"));
        assert_eq!(
            provider.capability,
            CapabilityId::new("desktop")
        );
    }

    #[test]
    fn desktop_provider_contains_expected_steps() {
        let registry = Registry::built_in();

        let provider = registry
            .provider_for(&Capability::new("desktop"))
            .expect("desktop provider should exist");

        assert_eq!(
            provider.steps,
            vec![
                PlanStep::new(Action::InstallPackageManifest(
                    "desktop".to_owned(),
                )),
                PlanStep::new(Action::EnableService(
                    "display-manager".to_owned(),
                )),
            ]
        );
    }

    #[test]
    fn provider_for_finds_matching_provider() {
        let registry = Registry::built_in();
        let capability = Capability::new("desktop");

        let provider = registry
            .provider_for(&capability)
            .expect("desktop provider should exist");

        assert_eq!(provider.id, ProviderId::new("desktop"));
    }

    #[test]
    fn provider_for_id_finds_matching_provider() {
        let registry = Registry::built_in();
        let capability_id = CapabilityId::new("desktop");

        let provider = registry
            .provider_for_id(&capability_id)
            .expect("desktop provider should exist");

        assert_eq!(provider.id, ProviderId::new("desktop"));
    }

    #[test]
    fn provider_for_returns_none_for_unknown_capability() {
        let registry = Registry::built_in();
        let capability = Capability::new("unknown");

        assert!(registry.provider_for(&capability).is_none());
    }

    #[test]
    fn registry_can_be_created_from_providers() {
        let provider = Provider {
            id: ProviderId::new("custom-desktop"),
            capability: CapabilityId::new("desktop"),
            steps: Vec::new(),
        };

        let registry =
            Registry::from_providers(vec![provider.clone()]);

        assert_eq!(registry.providers(), &[provider]);
    }
}
