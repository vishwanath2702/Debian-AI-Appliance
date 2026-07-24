//! Capability expansion and provider resolution.

use std::error::Error;
use std::fmt;

use model::{Capability, Provider};
use registry::Registry;

/// Failure to resolve a requested capability.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResolveError {
    /// No registered provider supplies the requested capability.
    ProviderNotFound(Capability),
}

impl fmt::Display for ResolveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ProviderNotFound(capability) => {
                write!(formatter, "no provider found for capability '{capability}'")
            }
        }
    }
}

impl Error for ResolveError {}

/// Resolves capabilities against a provider registry.
#[derive(Debug)]
pub struct Resolver<'registry> {
    registry: &'registry Registry,
}

impl<'registry> Resolver<'registry> {
    /// Creates a resolver backed by the supplied registry.
    #[must_use]
    pub const fn new(registry: &'registry Registry) -> Self {
        Self { registry }
    }

    /// Resolves a capability to its registered provider.
    ///
    /// # Errors
    ///
    /// Returns [`ResolveError::ProviderNotFound`] when no provider supplies the
    /// requested capability.
    pub fn resolve(&self, capability: &Capability) -> Result<&Provider, ResolveError> {
        self.registry
            .providers()
            .iter()
            .find(|provider| provider.capability == *capability)
            .ok_or_else(|| ResolveError::ProviderNotFound(capability.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::{ResolveError, Resolver};
    use model::Capability;
    use registry::Registry;

    #[test]
    fn resolves_desktop_capability() {
        let registry = Registry::built_in();
        let resolver = Resolver::new(&registry);

        let provider = resolver
            .resolve(&Capability::new("desktop"))
            .expect("desktop provider should exist");

        assert_eq!(provider.name, "desktop");
    }

    #[test]
    fn rejects_unknown_capability() {
        let registry = Registry::built_in();
        let resolver = Resolver::new(&registry);
        let capability = Capability::new("unknown");

        let error = resolver
            .resolve(&capability)
            .expect_err("unknown capability should fail");

        assert_eq!(
            error,
            ResolveError::ProviderNotFound(Capability::new("unknown"))
        );
        assert_eq!(
            error.to_string(),
            "no provider found for capability 'unknown'"
        );
    }
}
