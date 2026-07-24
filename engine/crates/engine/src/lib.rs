//! High-level orchestration for the DAIA engine.

use model::{Capability, Plan};
use planner::Planner;
use registry::Registry;
use resolver::{ResolveError, Resolver};

/// High-level DAIA engine.
#[derive(Debug)]
pub struct Engine {
    registry: Registry,
}

impl Engine {
    /// Creates an engine backed by the supplied registry.
    #[must_use]
    pub const fn new(registry: Registry) -> Self {
        Self { registry }
    }

    /// Produces an execution plan for a requested capability.
    ///
    /// # Errors
    ///
    /// Returns [`ResolveError`] when no registered provider supplies the
    /// requested capability.
    pub fn plan(&self, capability: Capability) -> Result<Plan, ResolveError> {
        let resolver = Resolver::new(&self.registry);
        let provider = resolver.resolve(&capability)?;

        Ok(Planner::plan(capability, provider))
    }
}

#[cfg(test)]
mod tests {
    use super::Engine;
    use model::{Capability, PlanStep};
    use registry::Registry;
    use resolver::ResolveError;

    #[test]
    fn plans_desktop_capability_end_to_end() {
        let engine = Engine::new(Registry::built_in());

        let plan = engine
            .plan(Capability::new("desktop"))
            .expect("desktop plan should succeed");

        assert_eq!(plan.capability, Capability::new("desktop"));
        assert_eq!(plan.provider, "desktop");
        assert_eq!(
            plan.steps,
            vec![
                PlanStep::InstallPackageManifest("desktop".to_owned()),
                PlanStep::EnableService("display-manager".to_owned()),
            ]
        );
    }

    #[test]
    fn rejects_unknown_capability_end_to_end() {
        let engine = Engine::new(Registry::built_in());

        let error = engine
            .plan(Capability::new("unknown"))
            .expect_err("unknown capability should fail");

        assert_eq!(
            error,
            ResolveError::ProviderNotFound(Capability::new("unknown"))
        );
    }
}
