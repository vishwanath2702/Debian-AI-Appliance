//! Execution plan generation for the DAIA engine.

use std::fmt;

use model::{Capability, Plan};
use resolver::{ResolveError, Resolver};

/// Errors that can occur while generating a plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlanError {
    /// Provider resolution failed.
    Resolve(ResolveError),
}

impl fmt::Display for PlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resolve(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for PlanError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resolve(error) => Some(error),
        }
    }
}

impl From<ResolveError> for PlanError {
    fn from(error: ResolveError) -> Self {
        Self::Resolve(error)
    }
}

/// Builds execution plans from requested capabilities.
#[derive(Clone, Debug)]
pub struct Planner {
    resolver: Resolver,
}

impl Planner {
    /// Creates a planner using the supplied resolver.
    #[must_use]
    pub const fn new(resolver: Resolver) -> Self {
        Self { resolver }
    }

    /// Builds an execution plan for a requested capability.
    ///
    /// # Errors
    ///
    /// Returns [`PlanError::Resolve`] when no provider can be
    /// resolved for the requested capability.
    pub fn build(&self, capability: &Capability) -> Result<Plan, PlanError> {
        let provider = self.resolver.resolve(capability)?;

        Ok(Plan {
            capability: capability.clone(),
            provider: provider.id,
            steps: provider.steps,
        })
    }
}

#[cfg(test)]
mod tests {
    use model::{Action, Capability, CapabilityId, PlanStep, Provider, ProviderId};
    use registry::Registry;
    use resolver::{ResolveError, Resolver};

    use super::{PlanError, Planner};

    fn desktop_registry() -> Registry {
        Registry::from_providers(vec![Provider {
            id: ProviderId::new("desktop"),
            capability: CapabilityId::new("desktop"),
            steps: vec![
                PlanStep::new(Action::InstallPackageManifest("desktop".to_owned())),
                PlanStep::new(Action::EnableService("display-manager".to_owned())),
            ],
        }])
        .expect("desktop test registry should be valid")
    }

    #[test]
    fn builds_plan_for_builtin_provider() {
        let registry = desktop_registry();
        let resolver = Resolver::new(registry);
        let planner = Planner::new(resolver);

        let plan = planner
            .build(&Capability::new("desktop"))
            .expect("desktop plan should build");

        assert_eq!(plan.capability, Capability::new("desktop"));
        assert_eq!(plan.provider, ProviderId::new("desktop"));

        assert_eq!(
            plan.steps,
            vec![
                PlanStep::new(Action::InstallPackageManifest("desktop".to_owned(),)),
                PlanStep::new(Action::EnableService("display-manager".to_owned(),)),
            ]
        );
    }

    #[test]
    fn returns_error_for_unknown_capability() {
        let registry = desktop_registry();
        let resolver = Resolver::new(registry);
        let planner = Planner::new(resolver);

        let result = planner.build(&Capability::new("unknown"));

        assert_eq!(
            result,
            Err(PlanError::Resolve(ResolveError::ProviderNotFound(
                Capability::new("unknown"),
            ),))
        );
    }

    #[test]
    fn formats_resolution_error() {
        let error = PlanError::Resolve(ResolveError::ProviderNotFound(Capability::new("unknown")));

        assert_eq!(
            error.to_string(),
            "no provider found for capability \"unknown\""
        );
    }

    #[test]
    fn builds_plan_from_custom_provider() {
        let provider = Provider {
            id: ProviderId::new("custom"),
            capability: CapabilityId::new("custom"),
            steps: vec![PlanStep::new(Action::EnableService("customd".to_owned()))],
        };

        let registry =
            Registry::from_providers(vec![provider]).expect("test registry should be valid");
        let resolver = Resolver::new(registry);
        let planner = Planner::new(resolver);

        let plan = planner
            .build(&Capability::new("custom"))
            .expect("custom plan should build");

        assert_eq!(plan.capability, Capability::new("custom"));
        assert_eq!(plan.provider, ProviderId::new("custom"));

        assert_eq!(
            plan.steps,
            vec![PlanStep::new(Action::EnableService("customd".to_owned(),))]
        );
    }
}
