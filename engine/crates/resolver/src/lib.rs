//! Provider resolution for the DAIA engine.

use std::fmt;

use model::{Capability, Provider};
use registry::Registry;

/// Errors that can occur during provider resolution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResolveError {
    /// No provider exists for the requested capability.
    ProviderNotFound(Capability),
}

impl fmt::Display for ResolveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ProviderNotFound(capability) => {
                write!(
                    formatter,
                    "no provider found for capability \"{capability}\""
                )
            }
        }
    }
}

impl std::error::Error for ResolveError {}

/// Resolves capabilities into providers.
#[derive(Clone, Debug)]
pub struct Resolver {
    registry: Registry,
}

impl Resolver {
    /// Creates a resolver using the supplied registry.
    #[must_use]
    pub const fn new(registry: Registry) -> Self {
        Self { registry }
    }

    /// Resolves a capability into a provider.
    ///
    /// # Errors
    ///
    /// Returns [`ResolveError::ProviderNotFound`] when the registry
    /// contains no provider for the requested capability.
    pub fn resolve(&self, capability: &Capability) -> Result<Provider, ResolveError> {
        self.registry
            .provider_for(capability)
            .cloned()
            .ok_or_else(|| ResolveError::ProviderNotFound(capability.clone()))
    }
}

#[cfg(test)]
mod tests {
    use model::{Capability, CapabilityId, PlanStep, Provider, ProviderId};
    use registry::Registry;

    use super::{ResolveError, Resolver};

    #[test]
    fn resolves_existing_provider() {
        let resolver = Resolver::new(Registry::built_in());

        let provider = resolver
            .resolve(&Capability::new("desktop"))
            .expect("desktop provider should resolve");

        assert_eq!(provider.id, ProviderId::new("desktop"));
        assert_eq!(provider.capability, CapabilityId::new("desktop"));
    }

    #[test]
    fn returns_error_for_unknown_capability() {
        let resolver = Resolver::new(Registry::built_in());

        let result = resolver.resolve(&Capability::new("unknown"));

        assert_eq!(
            result,
            Err(ResolveError::ProviderNotFound(Capability::new("unknown"),))
        );
    }

    #[test]
    fn formats_provider_not_found_error() {
        let error = ResolveError::ProviderNotFound(Capability::new("unknown"));

        assert_eq!(
            error.to_string(),
            "no provider found for capability \"unknown\""
        );
    }

    #[test]
    fn resolves_custom_registry_provider() {
        let provider = Provider {
            id: ProviderId::new("custom"),
            capability: CapabilityId::new("custom"),
            steps: Vec::<PlanStep>::new(),
        };

        let registry = Registry::from_providers(vec![provider.clone()]);

        let resolver = Resolver::new(registry);

        let resolved_provider = resolver
            .resolve(&Capability::new("custom"))
            .expect("custom provider should resolve");

        assert_eq!(resolved_provider, provider);
    }
}
