//! Core domain types shared across the DAIA engine.

use std::fmt;

/// A capability requested through desired state.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Capability(String);

impl Capability {
    /// Creates a capability identifier.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the capability identifier.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Capability {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// One action in an execution plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlanStep {
    /// Installs packages declared by a package manifest.
    InstallPackageManifest(String),

    /// Enables a system service.
    EnableService(String),
}

impl fmt::Display for PlanStep {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InstallPackageManifest(manifest) => {
                write!(formatter, "Install package manifest: {manifest}")
            }
            Self::EnableService(service) => {
                write!(formatter, "Enable service: {service}")
            }
        }
    }
}

/// A provider capable of satisfying a capability.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Provider {
    /// Stable provider identifier.
    pub name: String,

    /// Capability supplied by this provider.
    pub capability: Capability,

    /// Actions required to apply this provider.
    pub steps: Vec<PlanStep>,
}

/// A deterministic execution plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Plan {
    /// Capability requested by the user.
    pub capability: Capability,

    /// Provider selected for the capability.
    pub provider: String,

    /// Ordered actions required by the provider.
    pub steps: Vec<PlanStep>,
}

#[cfg(test)]
mod tests {
    use super::{Capability, PlanStep};

    #[test]
    fn capability_exposes_its_identifier() {
        let capability = Capability::new("desktop");

        assert_eq!(capability.as_str(), "desktop");
        assert_eq!(capability.to_string(), "desktop");
    }

    #[test]
    fn plan_steps_have_human_readable_descriptions() {
        assert_eq!(
            PlanStep::InstallPackageManifest("desktop".to_owned()).to_string(),
            "Install package manifest: desktop"
        );
        assert_eq!(
            PlanStep::EnableService("display-manager".to_owned()).to_string(),
            "Enable service: display-manager"
        );
    }
}
