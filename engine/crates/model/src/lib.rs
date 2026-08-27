//! Core domain types shared across the DAIA engine.

use std::{
    fmt,
    path::{Path, PathBuf},
};

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

/// Stable identifier for a DAIA content repository.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ContentRepositoryId(String);

impl ContentRepositoryId {
    /// Creates a content repository identifier.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the content repository identifier as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ContentRepositoryId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}
/// Destination for imported external content.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContentImportDestination {
    path: String,
}

impl ContentImportDestination {
    /// Creates a content import destination.
    #[must_use]
    pub fn new(path: impl Into<String>) -> Self {
        Self { path: path.into() }
    }

    /// Returns the destination path.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }
}
/// Describes a logical collection of content available to an appliance.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContentRepository {
    id: ContentRepositoryId,
    description: String,
    sources: Vec<ContentSource>,
}

impl ContentRepository {
    /// Returns the configured content sources.
    #[must_use]
    pub fn sources(&self) -> &[ContentSource] {
        &self.sources
    }
    /// Creates a content repository with configured content sources.
    #[must_use]
    pub fn with_sources(
        id: impl Into<String>,
        description: impl Into<String>,
        sources: Vec<ContentSource>,
    ) -> Self {
        Self {
            id: ContentRepositoryId::new(id),
            description: description.into(),
            sources,
        }
    }
    /// Creates a content repository.
    #[must_use]
    pub fn new(id: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            id: ContentRepositoryId::new(id),
            description: description.into(),
            sources: Vec::new(),
        }
    }
    /// Returns the repository identifier.
    #[must_use]
    pub const fn id(&self) -> &ContentRepositoryId {
        &self.id
    }

    /// Returns the repository description.
    #[must_use]
    pub fn description(&self) -> &str {
        &self.description
    }
}

/// Stable identifier for a DAIA content source.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ContentSourceId(String);

impl ContentSourceId {
    /// Creates a content source identifier.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the content source identifier as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ContentSourceId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Describes a source from which appliance content can be acquired.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContentSource {
    id: ContentSourceId,
    repository: ContentRepositoryId,
    locator: String,
}

impl ContentSource {
    /// Creates a content source.
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        repository: ContentRepositoryId,
        locator: impl Into<String>,
    ) -> Self {
        Self {
            id: ContentSourceId::new(id),
            repository,
            locator: locator.into(),
        }
    }

    /// Returns the content source identifier.
    #[must_use]
    pub const fn id(&self) -> &ContentSourceId {
        &self.id
    }

    /// Returns the repository supplied by this source.
    #[must_use]
    pub const fn repository(&self) -> &ContentRepositoryId {
        &self.repository
    }

    /// Returns the source locator.
    #[must_use]
    pub fn locator(&self) -> &str {
        &self.locator
    }
}

/// Describes content discovered from a configured DAIA content source.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiscoveredContent {
    source_id: ContentSourceId,
    path: PathBuf,
}

impl DiscoveredContent {
    /// Creates discovered content.
    #[must_use]
    pub fn new(source_id: ContentSourceId, path: impl Into<PathBuf>) -> Self {
        Self {
            source_id,
            path: path.into(),
        }
    }

    /// Returns the configured source that supplied the content.
    #[must_use]
    pub const fn source_id(&self) -> &ContentSourceId {
        &self.source_id
    }

    /// Returns the discovered content path.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// Stable identifier for an importable external content item.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ExternalContentItemId(String);

impl ExternalContentItemId {
    /// Creates an external content item identifier.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the external content item identifier as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ExternalContentItemId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}
/// Content item realized in a DAIA content destination.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImportedContentItem {
    source_item_id: ExternalContentItemId,
    path: PathBuf,
}

impl ImportedContentItem {
    /// Creates an imported content item.
    #[must_use]
    pub fn new(source_item_id: ExternalContentItemId, path: impl Into<PathBuf>) -> Self {
        Self {
            source_item_id,
            path: path.into(),
        }
    }

    /// Returns the external content item from which this item was imported.
    #[must_use]
    pub const fn source_item_id(&self) -> &ExternalContentItemId {
        &self.source_item_id
    }

    /// Returns the realized path of the imported content item.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}
/// Describes one importable item discovered in external content.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExternalContentItem {
    id: ExternalContentItemId,
    source_id: ContentSourceId,
    path: PathBuf,
}
impl ExternalContentItem {
    /// Creates an importable external content item.
    #[must_use]
    pub fn new(source_id: ContentSourceId, path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let id = ExternalContentItemId::new(format!("{}:{}", source_id, path.display()));

        Self {
            id,
            source_id,
            path,
        }
    }
    /// Returns the stable external content item identifier.
    #[must_use]
    pub const fn id(&self) -> &ExternalContentItemId {
        &self.id
    }
    /// Returns the content source that supplied this item.
    #[must_use]
    pub const fn source_id(&self) -> &ContentSourceId {
        &self.source_id
    }

    /// Returns the path of this importable item.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// Stable identifier for storage discovered by DAIA.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct DiscoveredStorageId(String);

impl DiscoveredStorageId {
    /// Creates a discovered storage identifier.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the discovered storage identifier as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for DiscoveredStorageId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Describes storage discovered by DAIA.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiscoveredStorage {
    id: DiscoveredStorageId,
    kind: StorageKind,
    device_path: PathBuf,
}

impl DiscoveredStorage {
    /// Creates discovered storage.
    #[must_use]
    pub fn new(id: impl Into<String>, kind: StorageKind, device_path: impl Into<PathBuf>) -> Self {
        Self {
            id: DiscoveredStorageId::new(id),
            kind,
            device_path: device_path.into(),
        }
    }

    /// Returns the current Linux device path.
    #[must_use]
    pub fn device_path(&self) -> &Path {
        &self.device_path
    }

    /// Returns the discovered storage identifier.
    #[must_use]
    pub const fn id(&self) -> &DiscoveredStorageId {
        &self.id
    }

    /// Returns the storage role.
    #[must_use]
    pub const fn kind(&self) -> StorageKind {
        self.kind
    }
}

/// Describes the role of a DAIA storage target.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StorageKind {
    /// Storage provided by the system's primary/native disk.
    System,

    /// Additional non-removable storage.
    Secondary,

    /// Removable storage such as a USB device.
    Removable,
}

impl fmt::Display for StorageKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::System => formatter.write_str("system"),
            Self::Secondary => formatter.write_str("secondary"),
            Self::Removable => formatter.write_str("removable"),
        }
    }
}

/// Stable identifier for a DAIA storage target.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct StorageTargetId(String);

impl StorageTargetId {
    /// Creates a storage target identifier.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the storage target identifier as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for StorageTargetId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Describes a logical storage target available to DAIA.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StorageTarget {
    id: StorageTargetId,
    description: String,
}

impl StorageTarget {
    /// Creates a storage target.
    #[must_use]
    pub fn new(id: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            id: StorageTargetId::new(id),
            description: description.into(),
        }
    }

    /// Returns the storage target identifier.
    #[must_use]
    pub const fn id(&self) -> &StorageTargetId {
        &self.id
    }

    /// Returns the storage target description.
    #[must_use]
    pub fn description(&self) -> &str {
        &self.description
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
/// A DAIA appliance profile describing a desired appliance configuration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApplianceProfile {
    /// Human-readable profile name.
    pub name: String,

    /// Description of the appliance purpose.
    pub description: String,

    /// Capabilities included in this appliance.
    pub capabilities: Vec<Capability>,
}

impl ApplianceProfile {
    /// Creates an appliance profile.
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        capabilities: Vec<Capability>,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            capabilities,
        }
    }

    /// Returns the profile name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the profile description.
    #[must_use]
    pub fn description(&self) -> &str {
        &self.description
    }

    /// Returns capabilities provided by this profile.
    #[must_use]
    pub fn capabilities(&self) -> &[Capability] {
        &self.capabilities
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

/// Describes confirmed external content selected for import.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContentImportIntent {
    items: Vec<ExternalContentItemId>,
}

impl ContentImportIntent {
    /// Creates a confirmed content import intent.
    #[must_use]
    pub const fn new(items: Vec<ExternalContentItemId>) -> Self {
        Self { items }
    }

    /// Returns the external content selected for import.
    #[must_use]
    pub fn items(&self) -> &[ExternalContentItemId] {
        &self.items
    }
}

/// Describes a confirmed DAIA installation intent.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstallationIntent {
    profile_name: String,
    storage_id: DiscoveredStorageId,
}

impl InstallationIntent {
    /// Creates a confirmed installation intent.
    #[must_use]
    pub fn new(profile_name: impl Into<String>, storage_id: DiscoveredStorageId) -> Self {
        Self {
            profile_name: profile_name.into(),
            storage_id,
        }
    }

    /// Returns the selected appliance profile name.
    #[must_use]
    pub fn profile_name(&self) -> &str {
        &self.profile_name
    }

    /// Returns the selected storage identifier.
    #[must_use]
    pub const fn storage_id(&self) -> &DiscoveredStorageId {
        &self.storage_id
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Action, AssetId, Capability, CapabilityId, ContentImportDestination, ContentImportIntent,
        ContentRepository, ContentRepositoryId, ContentSource, ContentSourceId, DiscoveredContent,
        DiscoveredStorage, DiscoveredStorageId, ExternalContentItem, ExternalContentItemId,
        ImportedContentItem, InstallationIntent, PackageManifest, PlanStep, ProviderId,
        StorageKind, StorageTarget, StorageTargetId,
    };
    use std::path::{Path, PathBuf};

    #[test]
    fn content_import_destination_exposes_path() {
        let destination = ContentImportDestination::new("/var/lib/daia/content");

        assert_eq!(destination.path(), "/var/lib/daia/content");
    }

    #[test]
    fn imported_content_item_exposes_source_item_and_path() {
        let source_item_id =
            ExternalContentItemId::new("local-models-directory:/media/daia/models/model.gguf");

        let item =
            ImportedContentItem::new(source_item_id.clone(), "/var/lib/daia/content/model.gguf");

        assert_eq!(item.source_item_id(), &source_item_id);
        assert_eq!(item.path(), Path::new("/var/lib/daia/content/model.gguf"));
    }
    #[test]
    fn content_import_intent_exposes_selected_items() {
        let intent = ContentImportIntent::new(vec![
            ExternalContentItemId::new("local-models-directory:/media/daia/models/model.gguf"),
            ExternalContentItemId::new("local-models-directory:/media/daia/models/tokenizer.json"),
        ]);

        assert_eq!(
            intent.items(),
            &[
                ExternalContentItemId::new("local-models-directory:/media/daia/models/model.gguf"),
                ExternalContentItemId::new(
                    "local-models-directory:/media/daia/models/tokenizer.json"
                ),
            ]
        );
    }
    #[test]
    fn external_content_item_id_exposes_value() {
        let id = ExternalContentItemId::new("local-models-directory:model.gguf");

        assert_eq!(id.as_str(), "local-models-directory:model.gguf");
        assert_eq!(id.to_string(), "local-models-directory:model.gguf");
    }

    #[test]
    fn content_repository_exposes_sources() {
        let source = ContentSource::new(
            "local-models-directory",
            ContentRepositoryId::new("local-models"),
            "/media/models",
        );

        let repository = ContentRepository::with_sources(
            "local-models",
            "Models available on local storage",
            vec![source.clone()],
        );

        assert_eq!(repository.sources(), &[source]);
    }

    #[test]
    fn installation_intent_exposes_profile_and_storage() {
        let intent =
            InstallationIntent::new("desktop", DiscoveredStorageId::new("serial:usb-disk"));

        assert_eq!(intent.profile_name(), "desktop");
        assert_eq!(
            intent.storage_id(),
            &DiscoveredStorageId::new("serial:usb-disk")
        );
    }

    #[test]
    fn discovered_content_exposes_source_and_path() {
        let content = DiscoveredContent::new(
            ContentSourceId::new("documents-usb"),
            "/media/usb/daia-content",
        );

        assert_eq!(content.source_id(), &ContentSourceId::new("documents-usb"));
        assert_eq!(content.path(), Path::new("/media/usb/daia-content"));
    }
    #[test]
    fn external_content_item_exposes_source_and_path() {
        let item = ExternalContentItem::new(
            ContentSourceId::new("documents-usb"),
            "/media/usb/daia-content/manual.pdf",
        );
        assert_eq!(
            item.id(),
            &ExternalContentItemId::new("documents-usb:/media/usb/daia-content/manual.pdf")
        );

        assert_eq!(item.source_id(), &ContentSourceId::new("documents-usb"));
        assert_eq!(item.path(), Path::new("/media/usb/daia-content/manual.pdf"));
    }

    #[test]
    fn discovered_storage_exposes_current_device_path() {
        let storage = DiscoveredStorage::new("disk-1", StorageKind::Secondary, "/dev/sdb");
        assert_eq!(storage.id(), &DiscoveredStorageId::new("disk-1"));
        assert_eq!(storage.kind(), StorageKind::Secondary);
        assert_eq!(storage.device_path(), "/dev/sdb");
    }

    #[test]
    fn discovered_storage_exposes_identity_and_kind() {
        let storage = DiscoveredStorage::new("disk-1", StorageKind::Secondary, "/dev/sdb");
        assert_eq!(storage.id(), &DiscoveredStorageId::new("disk-1"));
        assert_eq!(storage.kind(), StorageKind::Secondary);
    }

    #[test]
    fn discovered_storage_id_exposes_value() {
        let storage_id = DiscoveredStorageId::new("disk-1");

        assert_eq!(storage_id.as_str(), "disk-1");
        assert_eq!(storage_id.to_string(), "disk-1");
    }

    #[test]
    fn storage_kinds_describe_storage_roles() {
        assert_eq!(StorageKind::System.to_string(), "system");
        assert_eq!(StorageKind::Secondary.to_string(), "secondary");
        assert_eq!(StorageKind::Removable.to_string(), "removable");
    }

    #[test]
    fn storage_target_exposes_its_metadata() {
        let target = StorageTarget::new("secondary-disk", "Secondary storage disk");

        assert_eq!(target.id(), &StorageTargetId::new("secondary-disk"));
        assert_eq!(target.description(), "Secondary storage disk");
    }

    #[test]
    fn storage_target_id_exposes_value() {
        let target_id = StorageTargetId::new("secondary-disk");

        assert_eq!(target_id.as_str(), "secondary-disk");
        assert_eq!(target_id.to_string(), "secondary-disk");
    }

    #[test]
    fn content_source_exposes_acquisition_metadata() {
        let source = ContentSource::new(
            "wikipedia-en",
            ContentRepositoryId::new("wikipedia"),
            "https://example.invalid/wikipedia-en",
        );

        assert_eq!(source.id(), &ContentSourceId::new("wikipedia-en"));
        assert_eq!(source.repository(), &ContentRepositoryId::new("wikipedia"));
        assert_eq!(source.locator(), "https://example.invalid/wikipedia-en");
    }

    #[test]
    fn content_repository_exposes_its_metadata() {
        let repository =
            ContentRepository::new("documents", "Documents available to the appliance");

        assert_eq!(repository.id(), &ContentRepositoryId::new("documents"));
        assert_eq!(
            repository.description(),
            "Documents available to the appliance"
        );
    }

    #[test]
    fn content_repository_id_exposes_value() {
        let repository_id = ContentRepositoryId::new("documents");

        assert_eq!(repository_id.as_str(), "documents");
        assert_eq!(repository_id.to_string(), "documents");
    }

    #[test]
    fn asset_id_exposes_its_identifier() {
        let asset_id = AssetId::new("desktop/files/lightdm.conf");

        assert_eq!(asset_id.as_str(), "desktop/files/lightdm.conf");
        assert_eq!(asset_id.to_string(), "desktop/files/lightdm.conf");
    }

    #[test]
    fn content_source_id_exposes_value() {
        let source_id = ContentSourceId::new("documents-usb");

        assert_eq!(source_id.as_str(), "documents-usb");
        assert_eq!(source_id.to_string(), "documents-usb");
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
