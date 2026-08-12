use std::path::PathBuf;

use registry::{ApplianceProfileRepository, RegistryError};

/// Loads the appliance profile repository used by the CLI.
///
/// # Errors
///
/// Returns a [`RegistryError`] if the appliance-profile directory cannot be
/// read or contains an invalid appliance profile definition.
pub fn load() -> Result<ApplianceProfileRepository, RegistryError> {
    ApplianceProfileRepository::from_directory(appliance_profile_directory())
}

fn appliance_profile_directory() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../registry/appliance-profiles")
}

#[cfg(test)]
mod tests {
    use model::Capability;

    use super::load;

    #[test]
    fn repository_appliance_profiles_load() {
        let repository = load().expect("repository appliance profiles should load");

        let profile = repository
            .profile("desktop")
            .expect("desktop appliance profile should exist");

        assert_eq!(profile.name(), "desktop");
        assert_eq!(profile.capabilities(), &[Capability::new("desktop")]);
    }
}
