//! Desired-state comparison and execution planning.

use model::{Capability, Plan, Provider};

/// Builds deterministic execution plans.
#[derive(Debug, Default)]
pub struct Planner;

impl Planner {
    /// Creates a planner.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Builds a plan from a requested capability and its resolved provider.
    #[must_use]
    pub fn plan(capability: Capability, provider: &Provider) -> Plan {
        Plan {
            capability,
            provider: provider.name.clone(),
            steps: provider.steps.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Planner;
    use model::{Capability, Plan, PlanStep, Provider};

    #[test]
    fn builds_plan_from_resolved_provider() {
        let capability = Capability::new("desktop");
        let provider = Provider {
            name: "desktop".to_owned(),
            capability: capability.clone(),
            steps: vec![
                PlanStep::InstallPackageManifest("desktop".to_owned()),
                PlanStep::EnableService("display-manager".to_owned()),
            ],
        };

        let plan = Planner::plan(capability.clone(), &provider);

        assert_eq!(
            plan,
            Plan {
                capability,
                provider: "desktop".to_owned(),
                steps: vec![
                    PlanStep::InstallPackageManifest("desktop".to_owned()),
                    PlanStep::EnableService("display-manager".to_owned()),
                ],
            }
        );
    }
}
