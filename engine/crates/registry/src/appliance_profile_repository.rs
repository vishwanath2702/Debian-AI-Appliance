use std::{collections::HashSet, fs, path::Path};

use model::ApplianceProfile;

use crate::{RegistryError, appliance_profile::appliance_profile_from_yaml};

/// Collection of appliance profiles known to the DAIA engine.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ApplianceProfileRepository {
    profiles: Vec<ApplianceProfile>,
}

impl ApplianceProfileRepository {
    /// Creates an empty appliance profile repository.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            profiles: Vec::new(),
        }
    }

    /// Creates a repository from an existing collection of appliance profiles.
    ///
    /// # Errors
    ///
    /// Returns [`RegistryError::DuplicateApplianceProfile`] if two profiles use
    /// the same name.
    pub fn from_profiles(profiles: Vec<ApplianceProfile>) -> Result<Self, RegistryError> {
        let mut names = HashSet::new();

        for profile in &profiles {
            if !names.insert(profile.name().to_owned()) {
                return Err(RegistryError::DuplicateApplianceProfile(
                    profile.name().to_owned(),
                ));
            }
        }

        Ok(Self { profiles })
    }

    /// Creates a repository from one appliance profile YAML document.
    ///
    /// # Errors
    ///
    /// Returns a [`RegistryError`] if the YAML cannot be parsed or
    /// the profile definition is invalid.
    pub fn from_yaml_str(yaml: &str) -> Result<Self, RegistryError> {
        let profile = appliance_profile_from_yaml(yaml)?;

        Self::from_profiles(vec![profile])
    }

    /// Creates a repository from one appliance profile YAML file.
    ///
    /// # Errors
    ///
    /// Returns a [`RegistryError`] if the file cannot be read,
    /// parsed or validated.
    pub fn from_yaml_file(path: impl AsRef<Path>) -> Result<Self, RegistryError> {
        let yaml = fs::read_to_string(path)?;

        Self::from_yaml_str(&yaml)
    }

    /// Creates a repository from all appliance profile YAML files
    /// in a directory.
    ///
    /// Files ending in `.yaml` or `.yml` are loaded.
    ///
    /// # Errors
    ///
    /// Returns a [`RegistryError`] if loading fails.
    pub fn from_directory(path: impl AsRef<Path>) -> Result<Self, RegistryError> {
        let mut profiles = Vec::new();

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

            let repository = Self::from_yaml_file(&path)?;

            profiles.extend(repository.profiles);
        }

        Self::from_profiles(profiles)
    }

    /// Returns every appliance profile.
    #[must_use]
    pub fn profiles(&self) -> &[ApplianceProfile] {
        &self.profiles
    }

    /// Finds a profile by name.
    #[must_use]
    pub fn profile(&self, name: &str) -> Option<&ApplianceProfile> {
        self.profiles.iter().find(|profile| profile.name() == name)
    }
}
#[cfg(test)]
mod tests {
    use std::fs;

    use model::{ApplianceProfile, Capability};

    use crate::RegistryError;

    use super::ApplianceProfileRepository;

    const DESKTOP_PROFILE_YAML: &str = r"
name: desktop
description: Graphical desktop appliance
capabilities:
  - desktop
  - remote-access
";

    const SERVER_PROFILE_YAML: &str = r"
name: server
description: Headless server appliance
capabilities:
  - remote-access
";

    #[test]
    fn new_repository_is_empty() {
        let repository = ApplianceProfileRepository::new();

        assert!(repository.profiles().is_empty());
    }

    #[test]
    fn repository_can_be_created_from_profiles() {
        let profile = ApplianceProfile::new(
            "desktop",
            "Graphical desktop appliance",
            vec![Capability::new("desktop")],
        );

        let repository = ApplianceProfileRepository::from_profiles(vec![profile.clone()])
            .expect("valid appliance-profile repository");

        assert_eq!(repository.profiles(), &[profile]);
    }

    #[test]
    fn duplicate_profile_names_are_rejected() {
        let profiles = vec![
            ApplianceProfile::new("desktop", "First desktop", vec![Capability::new("desktop")]),
            ApplianceProfile::new(
                "desktop",
                "Second desktop",
                vec![Capability::new("remote-access")],
            ),
        ];

        let error = ApplianceProfileRepository::from_profiles(profiles)
            .expect_err("duplicate profile names should fail");

        assert!(matches!(
            error,
            RegistryError::DuplicateApplianceProfile(name)
                if name == "desktop"
        ));
    }

    #[test]
    fn repository_can_be_created_from_yaml() {
        let repository = ApplianceProfileRepository::from_yaml_str(DESKTOP_PROFILE_YAML)
            .expect("appliance profile YAML should load");

        let profile = repository
            .profile("desktop")
            .expect("desktop profile should exist");

        assert_eq!(profile.description(), "Graphical desktop appliance");
        assert_eq!(
            profile.capabilities(),
            &[Capability::new("desktop"), Capability::new("remote-access"),]
        );
    }

    #[test]
    fn directory_loads_multiple_profiles() {
        let directory = tempfile::tempdir().expect("temporary directory should exist");

        fs::write(directory.path().join("desktop.yaml"), DESKTOP_PROFILE_YAML)
            .expect("desktop profile should be written");

        fs::write(directory.path().join("server.yml"), SERVER_PROFILE_YAML)
            .expect("server profile should be written");

        let repository = ApplianceProfileRepository::from_directory(directory.path())
            .expect("profile directory should load");

        assert_eq!(repository.profiles().len(), 2);
        assert!(repository.profile("desktop").is_some());
        assert!(repository.profile("server").is_some());
    }

    #[test]
    fn profile_returns_none_for_unknown_name() {
        let repository = ApplianceProfileRepository::from_yaml_str(DESKTOP_PROFILE_YAML)
            .expect("appliance profile YAML should load");

        assert!(repository.profile("does-not-exist").is_none());
    }

    #[test]
    fn non_yaml_files_are_ignored() {
        let directory = tempfile::tempdir().expect("temporary directory should exist");

        fs::write(directory.path().join("README.md"), "# Profiles")
            .expect("README should be written");

        fs::write(directory.path().join("desktop.yaml"), DESKTOP_PROFILE_YAML)
            .expect("desktop profile should be written");

        let repository = ApplianceProfileRepository::from_directory(directory.path())
            .expect("profile directory should load");

        assert_eq!(repository.profiles().len(), 1);
    }

    #[test]
    fn empty_directory_creates_empty_repository() {
        let directory = tempfile::tempdir().expect("temporary directory should exist");

        let repository = ApplianceProfileRepository::from_directory(directory.path())
            .expect("empty directory should load");

        assert!(repository.profiles().is_empty());
    }
}
