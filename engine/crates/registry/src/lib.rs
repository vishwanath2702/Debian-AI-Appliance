//! Declarative registry loading and validation.

use model::{Capability, PlanStep, Provider};

/// Registry of providers known to the engine.
#[derive(Debug)]
pub struct Registry {
    providers: Vec<Provider>,
}

impl Registry {
    /// Creates the built-in registry.
    #[must_use]
    pub fn built_in() -> Self {
        Self {
            providers: vec![Provider {
                name: "desktop".to_owned(),
                capability: Capability::new("desktop"),
                steps: vec![
                    PlanStep::InstallPackageManifest("desktop".to_owned()),
                    PlanStep::EnableService("display-manager".to_owned()),
                ],
            }],
        }
    }

    /// Returns all registered providers.
    #[must_use]
    pub fn providers(&self) -> &[Provider] {
        &self.providers
    }
    /// Returns the provider for a capability.
    #[must_use]
    pub fn provider_for(&self, capability: &Capability) -> Option<&Provider> {
        self.providers
            .iter()
            .find(|provider| provider.capability == *capability)
    }
}

#[cfg(test)]
mod tests {
    use super::Registry;
    use model::{Capability, PlanStep};

    #[test]
    fn finds_provider_by_capability() {
        let registry = Registry::built_in();

        let provider = registry
            .provider_for(&Capability::new("desktop"))
            .expect("desktop provider should exist");

        assert_eq!(provider.name, "desktop");
    }

    #[test]
    fn returns_none_for_unknown_capability() {
        let registry = Registry::built_in();

        assert!(registry.provider_for(&Capability::new("unknown")).is_none());
    }

    #[test]
    fn built_in_registry_contains_desktop_provider() {
        let registry = Registry::built_in();

        assert_eq!(registry.providers().len(), 1);

        let provider = &registry.providers()[0];

        assert_eq!(provider.name, "desktop");
        assert_eq!(provider.capability, Capability::new("desktop"));
        assert_eq!(
            provider.steps,
            vec![
                PlanStep::InstallPackageManifest("desktop".to_owned()),
                PlanStep::EnableService("display-manager".to_owned()),
            ]
        );
    }
}
