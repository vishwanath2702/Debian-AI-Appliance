use std::{fs, path::Path};

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

    /// Creates a registry from one provider YAML file.
    ///
    /// # Errors
    ///
    /// Returns a [`RegistryError`] if the file cannot be read, the
    /// YAML cannot be parsed, or the provider definition is invalid.
    pub fn from_yaml_file(path: impl AsRef<Path>) -> Result<Self, RegistryError> {
        let yaml = fs::read_to_string(path)?;

        Self::from_yaml_str(&yaml)
    }

    /// Creates a registry from all provider YAML files in a directory.
    ///
    /// Files ending in `.yaml` or `.yml` are loaded. All other files are
    /// ignored.
    ///
    /// # Errors
    ///
    /// Returns a [`RegistryError`] if the directory cannot be read, a
    /// provider file cannot be read, the YAML cannot be parsed, or a
    /// provider definition is invalid.
    pub fn from_directory(path: impl AsRef<Path>) -> Result<Self, RegistryError> {
        let mut providers = Vec::new();

        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let path = entry.path();

            if !path.is_file() {
                continue;
            }

            let Some(extension) = path.extension() else {
                continue;
            };

            if extension != "yaml" && extension != "yml" {
                continue;
            }

            let registry = Self::from_yaml_file(&path)?;

            providers.extend(registry.providers);
        }

        Ok(Self::from_providers(providers))
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
    use std::fs;

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

    const SSH_PROVIDER_YAML: &str = r"
id: ssh
capability: remote-access
steps:
  - install_package_manifest: ssh
  - enable_service: ssh
";

    #[test]
    fn directory_loads_multiple_providers() {
        let directory = tempfile::tempdir().expect("temporary directory should exist");

        fs::write(directory.path().join("desktop.yaml"), DESKTOP_PROVIDER_YAML)
            .expect("desktop provider YAML should be written");

        fs::write(directory.path().join("ssh.yml"), SSH_PROVIDER_YAML)
            .expect("SSH provider YAML should be written");

        let registry =
            Registry::from_directory(directory.path()).expect("provider directory should load");

        assert_eq!(registry.providers().len(), 2);

        let desktop = registry
            .provider_for(&Capability::new("desktop"))
            .expect("desktop provider should exist");

        assert_eq!(desktop.id, ProviderId::new("desktop"));

        let ssh = registry
            .provider_for(&Capability::new("remote-access"))
            .expect("SSH provider should exist");

        assert_eq!(ssh.id, ProviderId::new("ssh"));
        assert_eq!(
            ssh.steps,
            vec![
                PlanStep::new(Action::InstallPackageManifest("ssh".to_owned(),)),
                PlanStep::new(Action::EnableService("ssh".to_owned())),
            ]
        );
    }

    #[test]
    fn malformed_provider_in_directory_returns_parse_error() {
        let directory = tempfile::tempdir().expect("temporary directory should exist");

        fs::write(
            directory.path().join("desktop.yaml"),
            "id: desktop\ncapability: [",
        )
        .unwrap();

        let error =
            Registry::from_directory(directory.path()).expect_err("invalid YAML should fail");

        assert!(matches!(error, RegistryError::Parse(_)));
    }

    #[test]
    fn missing_directory_returns_io_error() {
        let directory = tempfile::tempdir().expect("temporary directory should exist");

        let path = directory.path().join("does-not-exist");

        let error = Registry::from_directory(path).expect_err("missing directory should fail");

        assert!(matches!(error, RegistryError::Io(_)));
    }

    #[test]
    fn non_yaml_files_are_ignored() {
        let directory = tempfile::tempdir().expect("temporary directory should exist");

        fs::write(directory.path().join("README.md"), "# Example").unwrap();

        fs::write(directory.path().join("desktop.yaml"), DESKTOP_PROVIDER_YAML).unwrap();

        let registry = Registry::from_directory(directory.path()).expect("directory should load");

        assert_eq!(registry, Registry::built_in());
    }

    #[test]
    fn empty_directory_creates_empty_registry() {
        let directory = tempfile::tempdir().expect("temporary directory should exist");

        let registry =
            Registry::from_directory(directory.path()).expect("empty directory should load");

        assert!(registry.providers().is_empty());
    }

    #[test]
    fn registry_can_be_created_from_directory() {
        let directory = tempfile::tempdir().expect("temporary directory should exist");

        fs::write(directory.path().join("desktop.yaml"), DESKTOP_PROVIDER_YAML)
            .expect("provider YAML should be written");

        let registry =
            Registry::from_directory(directory.path()).expect("directory should load successfully");

        assert_eq!(registry, Registry::built_in());
    }

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
    fn registry_can_be_created_from_yaml_file() {
        let directory = tempfile::tempdir().expect("temporary directory should exist");
        let path = directory.path().join("desktop.yaml");

        fs::write(&path, DESKTOP_PROVIDER_YAML).expect("provider YAML file should be written");

        let registry =
            Registry::from_yaml_file(&path).expect("desktop provider YAML file should be valid");

        assert_eq!(registry, Registry::built_in());
    }

    #[test]
    fn missing_yaml_file_returns_io_error() {
        let directory = tempfile::tempdir().expect("temporary directory should exist");
        let path = directory.path().join("missing.yaml");

        let error = Registry::from_yaml_file(path).expect_err("missing provider file should fail");

        assert!(matches!(error, RegistryError::Io(_)));
    }

    #[test]
    fn invalid_yaml_file_returns_parse_error() {
        let directory = tempfile::tempdir().expect("temporary directory should exist");
        let path = directory.path().join("invalid.yaml");

        fs::write(&path, "id: desktop\ncapability: [")
            .expect("invalid YAML file should be written");

        let error = Registry::from_yaml_file(path).expect_err("invalid provider YAML should fail");

        assert!(matches!(error, RegistryError::Parse(_)));
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
