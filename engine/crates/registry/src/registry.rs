use model::{Action, Capability, CapabilityId, PlanStep, Provider, ProviderId};

use crate::{RegistryError, dto::provider_from_yaml};

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

    /// Creates a registry from one YAML provider document.
    ///
    /// # Errors
    ///
    /// Returns a [`RegistryError`] if the YAML cannot be parsed or
    /// the provider definition contains invalid data.
    pub fn from_yaml_str(yaml: &str) -> Result<Self, RegistryError> {
        let provider = provider_from_yaml(yaml)?;

        Ok(Self::from_providers(vec![provider]))
    }

    /// Returns every provider in the registry.
    #[must_use]
    pub fn providers(&self) -> &[Provider] {
        &self.providers
    }

    /// Finds a provider that supplies the requested capability.
    #[must_use]
    pub fn provider_for(&self, capability: &Capability) -> Option<&Provider> {
        self.provider_for_id(capability.id())
    }

    /// Finds a provider that supplies the requested capability identifier.
    #[must_use]
    pub fn provider_for_id(&self, capability_id: &CapabilityId) -> Option<&Provider> {
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
            PlanStep::new(Action::InstallPackageManifest("desktop".to_owned())),
            PlanStep::new(Action::EnableService("display-manager".to_owned())),
        ],
    }
}

#[cfg(test)]
mod tests {
    use model::{Action, Capability, CapabilityId, PlanStep, Provider, ProviderId};

    use crate::RegistryError;

    use super::Registry;

    const DESKTOP_PROVIDER_YAML: &str = r"
id: desktop
capability: desktop
steps:
  - install_package_manifest: desktop
  - enable_service: display-manager
";

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
        assert_eq!(provider.capability, CapabilityId::new("desktop"));
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
                PlanStep::new(Action::InstallPackageManifest("desktop".to_owned(),)),
                PlanStep::new(Action::EnableService("display-manager".to_owned(),)),
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

        let registry = Registry::from_providers(vec![provider.clone()]);

        assert_eq!(registry.providers(), &[provider]);
    }

    #[test]
    fn registry_can_be_created_from_yaml() {
        let registry = Registry::from_yaml_str(DESKTOP_PROVIDER_YAML)
            .expect("desktop provider YAML should be valid");

        assert_eq!(registry.providers().len(), 1);

        let provider = registry
            .provider_for(&Capability::new("desktop"))
            .expect("desktop provider should exist");

        assert_eq!(provider.id, ProviderId::new("desktop"));
        assert_eq!(provider.capability, CapabilityId::new("desktop"));
        assert_eq!(
            provider.steps,
            vec![
                PlanStep::new(Action::InstallPackageManifest("desktop".to_owned(),)),
                PlanStep::new(Action::EnableService("display-manager".to_owned(),)),
            ]
        );
    }

    #[test]
    fn yaml_provider_matches_builtin_provider() {
        let yaml_registry = Registry::from_yaml_str(DESKTOP_PROVIDER_YAML)
            .expect("desktop provider YAML should be valid");

        assert_eq!(yaml_registry, Registry::built_in());
    }

    #[test]
    fn malformed_yaml_returns_parse_error() {
        let error = Registry::from_yaml_str("id: desktop\ncapability: [")
            .expect_err("malformed YAML should fail");

        assert!(matches!(error, RegistryError::Parse(_)));
    }

    #[test]
    fn missing_required_field_returns_parse_error() {
        let error = Registry::from_yaml_str(
            r"
id: desktop
steps:
  - enable_service: display-manager
",
        )
        .expect_err("missing capability should fail");

        assert!(matches!(error, RegistryError::Parse(_)));
    }

    #[test]
    fn unknown_step_returns_parse_error() {
        let error = Registry::from_yaml_str(
            r"
id: desktop
capability: desktop
steps:
  - run_command: example
",
        )
        .expect_err("unknown step should fail");

        assert!(matches!(error, RegistryError::Parse(_)));
    }

    #[test]
    fn unknown_provider_field_returns_parse_error() {
        let error = Registry::from_yaml_str(
            r"
id: desktop
capability: desktop
description: Example provider
steps:
  - enable_service: display-manager
",
        )
        .expect_err("unknown provider field should fail");

        assert!(matches!(error, RegistryError::Parse(_)));
    }

    #[test]
    fn empty_provider_id_returns_validation_error() {
        let error = Registry::from_yaml_str(
            r#"
id: ""
capability: desktop
steps:
  - enable_service: display-manager
"#,
        )
        .expect_err("empty provider id should fail");

        assert!(matches!(error, RegistryError::InvalidProvider(_)));

        assert_eq!(
            error.to_string(),
            "invalid provider: provider id must not be empty"
        );
    }

    #[test]
    fn provider_without_steps_returns_validation_error() {
        let error = Registry::from_yaml_str(
            r"
id: desktop
capability: desktop
steps: []
",
        )
        .expect_err("provider without steps should fail");

        assert!(matches!(error, RegistryError::InvalidProvider(_)));

        assert_eq!(
            error.to_string(),
            concat!(
                "invalid provider: ",
                "provider must contain at least one step"
            )
        );
    }

    #[test]
    fn empty_step_value_returns_validation_error() {
        let error = Registry::from_yaml_str(
            r#"
id: desktop
capability: desktop
steps:
  - enable_service: ""
"#,
        )
        .expect_err("empty service name should fail");

        assert!(matches!(error, RegistryError::InvalidProvider(_)));

        assert_eq!(
            error.to_string(),
            "invalid provider: service name must not be empty"
        );
    }
}
