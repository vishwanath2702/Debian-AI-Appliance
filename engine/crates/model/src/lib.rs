//! Core domain types shared across the DAIA engine.

use std::{fmt, path::PathBuf};

/// Stable identifier for a DAIA asset.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct AssetId(String);

impl AssetId {
    /// Creates an asset identifier.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the asset identifier as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AssetId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Stable identifier for a DAIA capability.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct CapabilityId(String);

impl CapabilityId {
    /// Creates a capability identifier.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the capability identifier as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CapabilityId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Stable identifier for a DAIA provider.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ProviderId(String);

impl ProviderId {
    /// Creates a provider identifier.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the provider identifier as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ProviderId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}
/// A capability requested through desired state.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Capability {
    /// Stable capability identifier.
    pub id: CapabilityId,
}

impl Capability {
    /// Creates a capability.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self {
            id: CapabilityId::new(value),
        }
    }

    /// Returns the capability identifier.
    #[must_use]
    pub const fn id(&self) -> &CapabilityId {
        &self.id
    }
    /// Returns the capability identifier as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.id.as_str()
    }
}

impl fmt::Display for Capability {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.id.fmt(formatter)
    }
}

/// An operation that can be included in an execution plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Action {
    /// Installs packages declared by a package manifest.
    InstallPackageManifest(String),

    /// Copies a bundled asset to a destination path.
    CopyAsset {
        /// Stable identifier of the bundled asset.
        asset: AssetId,

        /// Destination path inside the target system.
        destination: PathBuf,
    },

    /// Enables a system service.
    EnableService(String),
}

impl fmt::Display for Action {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InstallPackageManifest(manifest) => {
                write!(formatter, "Install package manifest: {manifest}")
            }
            Self::CopyAsset { asset, destination } => {
                write!(
                    formatter,
                    "Copy asset: {asset} -> {}",
                    destination.display()
                )
            }
            Self::EnableService(service) => {
                write!(formatter, "Enable service: {service}")
            }
        }
    }
}
/// One ordered action in an execution plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanStep {
    /// Action performed by this plan step.
    pub action: Action,
}

impl PlanStep {
    /// Creates a plan step containing the supplied action.
    #[must_use]
    pub const fn new(action: Action) -> Self {
        Self { action }
    }
}

impl fmt::Display for PlanStep {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.action.fmt(formatter)
    }
}

/// A provider capable of satisfying a capability.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Provider {
    /// Stable provider identifier.
    pub id: ProviderId,

    /// Identifier of the capability supplied by this provider.
    pub capability: CapabilityId,

    /// Actions required to apply this provider.
    pub steps: Vec<PlanStep>,
}

/// A named collection of operating-system packages.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackageManifest {
    /// Logical name used to reference this manifest.
    pub name: String,

    /// Packages installed by this manifest.
    pub packages: Vec<String>,
}

impl PackageManifest {
    /// Creates a package manifest.
    #[must_use]
    pub fn new(name: impl Into<String>, packages: Vec<String>) -> Self {
        Self {
            name: name.into(),
            packages,
        }
    }

    /// Returns the manifest name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the packages declared by this manifest.
    #[must_use]
    pub fn packages(&self) -> &[String] {
        &self.packages
    }
}

/// A deterministic execution plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Plan {
    /// Capability requested by the user.
    pub capability: Capability,

    /// Provider selected for the capability.
    pub provider: ProviderId,

    /// Ordered actions required by the provider.
    pub steps: Vec<PlanStep>,
}
#[cfg(test)]
mod tests {
    use super::{Action, AssetId, Capability, CapabilityId, PackageManifest, PlanStep, ProviderId};
    use std::path::PathBuf;
    #[test]
    fn asset_id_exposes_its_identifier() {
        let asset_id = AssetId::new("desktop/files/lightdm.conf");

        assert_eq!(asset_id.as_str(), "desktop/files/lightdm.conf");
        assert_eq!(asset_id.to_string(), "desktop/files/lightdm.conf");
    }

    #[test]
    fn provider_id_exposes_its_identifier() {
        let provider_id = ProviderId::new("desktop");

        assert_eq!(provider_id.as_str(), "desktop");
        assert_eq!(provider_id.to_string(), "desktop");
    }

    #[test]
    fn capability_exposes_its_identifier() {
        let capability = Capability::new("desktop");

        assert_eq!(capability.id(), &CapabilityId::new("desktop"));
        assert_eq!(capability.as_str(), "desktop");
        assert_eq!(capability.to_string(), "desktop");
    }
    #[test]
    fn package_manifest_exposes_its_name_and_packages() {
        let manifest =
            PackageManifest::new("desktop", vec!["gnome-shell".to_owned(), "gdm3".to_owned()]);

        assert_eq!(manifest.name(), "desktop");
        assert_eq!(
            manifest.packages(),
            &["gnome-shell".to_owned(), "gdm3".to_owned()]
        );
    }
    #[test]
    fn actions_have_human_readable_descriptions() {
        assert_eq!(
            Action::InstallPackageManifest("desktop".to_owned()).to_string(),
            "Install package manifest: desktop"
        );

        assert_eq!(
            Action::CopyAsset {
                asset: AssetId::new("desktop/files/lightdm.conf"),
                destination: PathBuf::from("/etc/lightdm/lightdm.conf"),
            }
            .to_string(),
            "Copy asset: desktop/files/lightdm.conf -> /etc/lightdm/lightdm.conf"
        );

        assert_eq!(
            Action::EnableService("display-manager".to_owned()).to_string(),
            "Enable service: display-manager"
        );
    }
    #[test]
    fn plan_steps_delegate_display_to_their_actions() {
        let install_step = PlanStep::new(Action::InstallPackageManifest("desktop".to_owned()));

        let service_step = PlanStep::new(Action::EnableService("display-manager".to_owned()));

        assert_eq!(
            install_step.to_string(),
            "Install package manifest: desktop"
        );

        assert_eq!(service_step.to_string(), "Enable service: display-manager");
    }
}
