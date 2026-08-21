use std::path::PathBuf;

use registry::{Registry, RegistryError};

/// Loads the provider registry used by the CLI.
///
/// # Errors
///
/// Returns a [`RegistryError`] if the provider directory cannot be
/// read or contains an invalid provider definition.
pub fn load() -> Result<Registry, RegistryError> {
    Registry::from_directory(provider_directory())
}

fn provider_directory() -> PathBuf {
    let installed = PathBuf::from("/usr/share/daia/providers");

    if installed.is_dir() {
        installed
    } else {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../registry/providers")
    }
}

#[cfg(test)]
mod tests {
    use model::{Capability, ProviderId};

    use super::load;

    #[test]
    fn repository_provider_registry_loads() {
        let registry = load().expect("repository provider registry should load");

        let provider = registry
            .provider_for(&Capability::new("desktop"))
            .expect("desktop provider should exist");

        assert_eq!(provider.id, ProviderId::new("desktop"));
    }
}
