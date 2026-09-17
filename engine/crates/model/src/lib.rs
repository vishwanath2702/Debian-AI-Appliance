//! Core domain types shared across the DAIA engine.

use std::{
    fmt,
    path::{Path, PathBuf},
};

/// State coordinates against which a DAIA decision is evaluated.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct StateBasis {
    desired_generation: DesiredGeneration,
    current_revision: CurrentRevision,
}

impl StateBasis {
    /// Creates a State Basis from Desired and Current State coordinates.
    #[must_use]
    pub const fn new(
        desired_generation: DesiredGeneration,
        current_revision: CurrentRevision,
    ) -> Self {
        Self {
            desired_generation,
            current_revision,
        }
    }

    /// Returns the Desired State generation in this basis.
    #[must_use]
    pub const fn desired_generation(&self) -> DesiredGeneration {
        self.desired_generation
    }

    /// Returns the Current State revision in this basis.
    #[must_use]
    pub const fn current_revision(&self) -> CurrentRevision {
        self.current_revision
    }
}

/// Revision of accepted DAIA Current State.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CurrentRevision(u64);

impl CurrentRevision {
    /// Creates a Current State revision.
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the revision number.
    #[must_use]
    pub const fn value(self) -> u64 {
        self.0
    }
}

impl fmt::Display for CurrentRevision {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

/// Generation of accepted DAIA Desired State.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DesiredGeneration(u64);

impl DesiredGeneration {
    /// Creates a Desired State generation.
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the generation number.
    #[must_use]
    pub const fn value(self) -> u64 {
        self.0
    }
}

impl fmt::Display for DesiredGeneration {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

/// Stable identifier for a DAIA architectural component.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ArchitecturalComponentId(String);

impl ArchitecturalComponentId {
    /// Creates an architectural component identifier.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the architectural component identifier as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ArchitecturalComponentId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Stable revision identifier for the verification policy used by DAIA.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct VerificationPolicyRevision(String);

impl VerificationPolicyRevision {
    /// Creates a verification policy revision identifier.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the verification policy revision as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for VerificationPolicyRevision {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Stable identifier for a DAIA Verification Provider.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct VerificationProviderId(String);

impl VerificationProviderId {
    /// Creates a Verification Provider identifier.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the Verification Provider identifier as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for VerificationProviderId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Version of a DAIA Verification Provider.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct VerificationProviderVersion(String);

impl VerificationProviderVersion {
    /// Creates a Verification Provider version.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the Verification Provider version as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for VerificationProviderVersion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Version of a DAIA Verification Rule.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct VerificationRuleVersion(String);

impl VerificationRuleVersion {
    /// Creates a Verification Rule version.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the Verification Rule version as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for VerificationRuleVersion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Reference to the versioned Verification Rule applied to one condition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerificationRuleReference {
    condition_id: VerificationConditionId,
    version: VerificationRuleVersion,
}

impl VerificationRuleReference {
    /// Creates a reference to a versioned Verification Rule.
    #[must_use]
    pub fn new(condition_id: VerificationConditionId, version: VerificationRuleVersion) -> Self {
        Self {
            condition_id,
            version,
        }
    }

    /// Returns the condition evaluated by the rule.
    #[must_use]
    pub fn condition_id(&self) -> &VerificationConditionId {
        &self.condition_id
    }

    /// Returns the applied Verification Rule version.
    #[must_use]
    pub fn version(&self) -> &VerificationRuleVersion {
        &self.version
    }
}

/// Immutable conclusion produced by DAIA verification for one managed resource.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerificationResult {
    result_id: VerificationResultId,
    resource_id: ResourceId,
    resource_type: ResourceType,
    purpose: VerificationPurpose,
    state_basis: StateBasis,
    desired_generation: Option<DesiredGeneration>,
    policy_revision: VerificationPolicyRevision,
    rules: Vec<VerificationRuleReference>,
    evidence: Vec<VerificationEvidenceReference>,
    verified_at: VerificationTimestamp,
    conditions: Vec<VerificationConditionResult>,
    overall_result: VerificationOverallResult,
    reasons: Vec<String>,
    warnings: Vec<String>,
    provider_id: VerificationProviderId,
    provider_version: VerificationProviderVersion,
    result_schema_version: SchemaVersion,
}

impl VerificationResult {
    /// Creates an immutable Verification Result.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        result_id: VerificationResultId,
        resource_id: ResourceId,
        resource_type: ResourceType,
        purpose: VerificationPurpose,
        state_basis: StateBasis,
        desired_generation: Option<DesiredGeneration>,
        policy_revision: VerificationPolicyRevision,
        rules: Vec<VerificationRuleReference>,
        evidence: Vec<VerificationEvidenceReference>,
        verified_at: VerificationTimestamp,
        conditions: Vec<VerificationConditionResult>,
        overall_result: VerificationOverallResult,
        reasons: Vec<String>,
        warnings: Vec<String>,
        provider_id: VerificationProviderId,
        provider_version: VerificationProviderVersion,
        result_schema_version: SchemaVersion,
    ) -> Self {
        Self {
            result_id,
            resource_id,
            resource_type,
            purpose,
            state_basis,
            desired_generation,
            policy_revision,
            rules,
            evidence,
            verified_at,
            conditions,
            overall_result,
            reasons,
            warnings,
            provider_id,
            provider_version,
            result_schema_version,
        }
    }

    #[must_use]
    pub fn result_id(&self) -> &VerificationResultId {
        &self.result_id
    }

    #[must_use]
    pub fn resource_id(&self) -> &ResourceId {
        &self.resource_id
    }

    #[must_use]
    pub fn resource_type(&self) -> &ResourceType {
        &self.resource_type
    }

    #[must_use]
    pub const fn purpose(&self) -> VerificationPurpose {
        self.purpose
    }

    #[must_use]
    pub const fn state_basis(&self) -> StateBasis {
        self.state_basis
    }

    #[must_use]
    pub const fn desired_generation(&self) -> Option<DesiredGeneration> {
        self.desired_generation
    }

    #[must_use]
    pub fn policy_revision(&self) -> &VerificationPolicyRevision {
        &self.policy_revision
    }

    #[must_use]
    pub fn rules(&self) -> &[VerificationRuleReference] {
        &self.rules
    }

    #[must_use]
    pub fn evidence(&self) -> &[VerificationEvidenceReference] {
        &self.evidence
    }

    #[must_use]
    pub fn verified_at(&self) -> &VerificationTimestamp {
        &self.verified_at
    }

    #[must_use]
    pub fn conditions(&self) -> &[VerificationConditionResult] {
        &self.conditions
    }

    #[must_use]
    pub const fn overall_result(&self) -> VerificationOverallResult {
        self.overall_result
    }

    #[must_use]
    pub fn reasons(&self) -> &[String] {
        &self.reasons
    }

    #[must_use]
    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }

    #[must_use]
    pub fn provider_id(&self) -> &VerificationProviderId {
        &self.provider_id
    }

    #[must_use]
    pub fn provider_version(&self) -> &VerificationProviderVersion {
        &self.provider_version
    }

    #[must_use]
    pub const fn result_schema_version(&self) -> SchemaVersion {
        self.result_schema_version
    }
}

/// Stable identifier for a DAIA Verification Result.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct VerificationResultId(String);

impl VerificationResultId {
    /// Creates a Verification Result identifier.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the Verification Result identifier as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for VerificationResultId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Timestamp associated with a DAIA verification request or evaluation.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct VerificationTimestamp(String);

impl VerificationTimestamp {
    /// Creates a verification timestamp from its textual representation.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the verification timestamp as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for VerificationTimestamp {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Request to evaluate declared conditions for one DAIA managed resource.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerificationRequest {
    resource_id: ResourceId,
    resource_type: ResourceType,
    resource_schema_version: SchemaVersion,
    purpose: VerificationPurpose,
    state_basis: StateBasis,
    desired_generation: Option<DesiredGeneration>,
    expected_conditions: Vec<VerificationCondition>,
    evidence_ids: Vec<EvidenceId>,
    authorized_evidence_sources: Vec<EvidenceSourceId>,
    policy_revision: VerificationPolicyRevision,
    requested_at: VerificationTimestamp,
    requesting_component: ArchitecturalComponentId,
}

impl VerificationRequest {
    /// Creates a verification request for one managed resource.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        resource_id: ResourceId,
        resource_type: ResourceType,
        resource_schema_version: SchemaVersion,
        purpose: VerificationPurpose,
        state_basis: StateBasis,
        desired_generation: Option<DesiredGeneration>,
        expected_conditions: Vec<VerificationCondition>,
        evidence_ids: Vec<EvidenceId>,
        authorized_evidence_sources: Vec<EvidenceSourceId>,
        policy_revision: VerificationPolicyRevision,
        requested_at: VerificationTimestamp,
        requesting_component: ArchitecturalComponentId,
    ) -> Self {
        Self {
            resource_id,
            resource_type,
            resource_schema_version,
            purpose,
            state_basis,
            desired_generation,
            expected_conditions,
            evidence_ids,
            authorized_evidence_sources,
            policy_revision,
            requested_at,
            requesting_component,
        }
    }

    #[must_use]
    pub fn resource_id(&self) -> &ResourceId {
        &self.resource_id
    }

    #[must_use]
    pub fn resource_type(&self) -> &ResourceType {
        &self.resource_type
    }

    #[must_use]
    pub const fn resource_schema_version(&self) -> SchemaVersion {
        self.resource_schema_version
    }

    #[must_use]
    pub const fn purpose(&self) -> VerificationPurpose {
        self.purpose
    }

    #[must_use]
    pub const fn state_basis(&self) -> StateBasis {
        self.state_basis
    }

    #[must_use]
    pub const fn desired_generation(&self) -> Option<DesiredGeneration> {
        self.desired_generation
    }

    #[must_use]
    pub fn expected_conditions(&self) -> &[VerificationCondition] {
        &self.expected_conditions
    }

    #[must_use]
    pub fn evidence_ids(&self) -> &[EvidenceId] {
        &self.evidence_ids
    }

    #[must_use]
    pub fn authorized_evidence_sources(&self) -> &[EvidenceSourceId] {
        &self.authorized_evidence_sources
    }

    #[must_use]
    pub fn policy_revision(&self) -> &VerificationPolicyRevision {
        &self.policy_revision
    }

    #[must_use]
    pub fn requested_at(&self) -> &VerificationTimestamp {
        &self.requested_at
    }

    #[must_use]
    pub fn requesting_component(&self) -> &ArchitecturalComponentId {
        &self.requesting_component
    }
}

/// Stable identifier for a DAIA verification condition.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct VerificationConditionId(String);

impl VerificationConditionId {
    /// Creates a verification condition identifier.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the verification condition identifier as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for VerificationConditionId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Expected condition declared by a DAIA Verification Request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerificationCondition {
    condition_id: VerificationConditionId,
    mandatory: bool,
}

impl VerificationCondition {
    /// Creates an expected verification condition.
    #[must_use]
    pub fn new(condition_id: VerificationConditionId, mandatory: bool) -> Self {
        Self {
            condition_id,
            mandatory,
        }
    }

    /// Returns the expected condition identifier.
    #[must_use]
    pub fn condition_id(&self) -> &VerificationConditionId {
        &self.condition_id
    }

    /// Returns whether the condition is mandatory for verification.
    #[must_use]
    pub const fn is_mandatory(&self) -> bool {
        self.mandatory
    }
}

/// Result produced for one evaluated DAIA verification condition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerificationConditionResult {
    condition_id: VerificationConditionId,
    result: ConditionResult,
}

impl VerificationConditionResult {
    /// Creates the result for one evaluated verification condition.
    #[must_use]
    pub fn new(condition_id: VerificationConditionId, result: ConditionResult) -> Self {
        Self {
            condition_id,
            result,
        }
    }

    /// Returns the evaluated condition identifier.
    #[must_use]
    pub fn condition_id(&self) -> &VerificationConditionId {
        &self.condition_id
    }

    /// Returns the condition evaluation result.
    #[must_use]
    pub const fn result(&self) -> ConditionResult {
        self.result
    }
}

/// Overall conclusion of DAIA verification for one managed resource.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum VerificationOverallResult {
    Satisfied,
    Unsatisfied,
    Unknown,
    Error,
}

/// Result of evaluating a DAIA verification condition.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ConditionResult {
    Satisfied,
    Unsatisfied,
    Unknown,
    NotApplicable,
    Error,
}

/// Purpose for which DAIA verification is requested.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum VerificationPurpose {
    CurrentStateEstablishment,
    DesiredStateSatisfaction,
    TransitionPreconditions,
    TransitionPostconditions,
    RecoveryVerification,
    DriftVerification,
    IntegrityVerification,
    HealthVerification,
}

/// Type of a DAIA managed resource.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ResourceType(String);

impl ResourceType {
    /// Creates a managed resource type.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the managed resource type as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ResourceType {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Stable identifier for a DAIA managed resource.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ResourceId(String);

impl ResourceId {
    /// Creates a managed resource identifier.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the managed resource identifier as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ResourceId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Stable identifier for a source of DAIA verification evidence.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct EvidenceSourceId(String);

impl EvidenceSourceId {
    /// Creates an evidence source identifier.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the evidence source identifier as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for EvidenceSourceId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Stable identifier for evidence submitted to DAIA verification.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct EvidenceId(String);

impl EvidenceId {
    /// Creates an evidence identifier.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the evidence identifier as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for EvidenceId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Reference to evidence used by DAIA verification and when it was collected.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerificationEvidenceReference {
    evidence_id: EvidenceId,
    collected_at: ObservationTimestamp,
}

impl VerificationEvidenceReference {
    /// Creates a reference to collected verification evidence.
    #[must_use]
    pub fn new(evidence_id: EvidenceId, collected_at: ObservationTimestamp) -> Self {
        Self {
            evidence_id,
            collected_at,
        }
    }

    /// Returns the evidence identifier.
    #[must_use]
    pub fn evidence_id(&self) -> &EvidenceId {
        &self.evidence_id
    }

    /// Returns when the evidence was collected.
    #[must_use]
    pub fn collected_at(&self) -> &ObservationTimestamp {
        &self.collected_at
    }
}

/// Evidence collected from or about a DAIA managed resource.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Observation<T> {
    resource_id: ResourceId,
    source_id: ObservationSourceId,
    collected_at: ObservationTimestamp,
    schema_version: SchemaVersion,
    observed: T,
}

impl<T> Observation<T> {
    /// Creates an observation for a managed resource.
    #[must_use]
    pub fn new(
        resource_id: ResourceId,
        source_id: ObservationSourceId,
        collected_at: ObservationTimestamp,
        schema_version: SchemaVersion,
        observed: T,
    ) -> Self {
        Self {
            resource_id,
            source_id,
            collected_at,
            schema_version,
            observed,
        }
    }

    /// Returns the observed managed resource identifier.
    #[must_use]
    pub fn resource_id(&self) -> &ResourceId {
        &self.resource_id
    }

    /// Returns the observation source identifier.
    #[must_use]
    pub fn source_id(&self) -> &ObservationSourceId {
        &self.source_id
    }

    /// Returns when the observation was collected.
    #[must_use]
    pub fn collected_at(&self) -> &ObservationTimestamp {
        &self.collected_at
    }

    /// Returns the schema version used by the collected values.
    #[must_use]
    pub const fn schema_version(&self) -> SchemaVersion {
        self.schema_version
    }

    /// Returns the collected values.
    #[must_use]
    pub fn observed(&self) -> &T {
        &self.observed
    }
}

/// Stable identifier for a DAIA observation source.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ObservationSourceId(String);

impl ObservationSourceId {
    /// Creates an observation source identifier.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the observation source identifier as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ObservationSourceId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Timestamp associated with a DAIA observation.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ObservationTimestamp(String);

impl ObservationTimestamp {
    /// Creates an observation timestamp from its textual representation.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the observation timestamp as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ObservationTimestamp {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Version of a DAIA schema.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SchemaVersion(u32);

impl SchemaVersion {
    /// Creates a schema version.
    #[must_use]
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    /// Returns the schema version number.
    #[must_use]
    pub const fn value(self) -> u32 {
        self.0
    }
}

impl fmt::Display for SchemaVersion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

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
    size_bytes: Option<u64>,
}

impl DiscoveredStorage {
    /// Creates discovered storage.
    #[must_use]
    pub fn new(id: impl Into<String>, kind: StorageKind, device_path: impl Into<PathBuf>) -> Self {
        Self {
            id: DiscoveredStorageId::new(id),
            kind,
            device_path: device_path.into(),
            size_bytes: None,
        }
    }

    /// Sets the discovered storage capacity in bytes.
    #[must_use]
    pub const fn with_size_bytes(mut self, size_bytes: u64) -> Self {
        self.size_bytes = Some(size_bytes);
        self
    }

    /// Returns the discovered storage capacity in bytes, when known.
    #[must_use]
    pub const fn size_bytes(&self) -> Option<u64> {
        self.size_bytes
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

/// Desired state declared for one DAIA managed resource.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DesiredResource<T> {
    resource_id: ResourceId,
    resource_type: ResourceType,
    schema_version: SchemaVersion,
    generation: DesiredGeneration,
    desired: T,
}

impl<T> DesiredResource<T> {
    /// Creates desired state for a managed resource.
    #[must_use]
    pub fn new(
        resource_id: ResourceId,
        resource_type: ResourceType,
        schema_version: SchemaVersion,
        generation: DesiredGeneration,
        desired: T,
    ) -> Self {
        Self {
            resource_id,
            resource_type,
            schema_version,
            generation,
            desired,
        }
    }

    /// Returns the managed resource identifier.
    #[must_use]
    pub fn resource_id(&self) -> &ResourceId {
        &self.resource_id
    }

    /// Returns the managed resource type.
    #[must_use]
    pub fn resource_type(&self) -> &ResourceType {
        &self.resource_type
    }

    /// Returns the schema version used by the desired values.
    #[must_use]
    pub const fn schema_version(&self) -> SchemaVersion {
        self.schema_version
    }

    /// Returns the Desired State generation.
    #[must_use]
    pub const fn generation(&self) -> DesiredGeneration {
        self.generation
    }

    /// Returns the desired values.
    #[must_use]
    pub fn desired(&self) -> &T {
        &self.desired
    }
}

/// Desired state for a system service.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ServiceDesiredState {
    present: bool,
    enabled: bool,
    running: bool,
}

impl ServiceDesiredState {
    /// Creates a desired service state.
    #[must_use]
    pub const fn new(present: bool, enabled: bool, running: bool) -> Self {
        Self {
            present,
            enabled,
            running,
        }
    }

    /// Returns whether the service is required to be present.
    #[must_use]
    pub const fn is_present(&self) -> bool {
        self.present
    }

    /// Returns whether the service is required to be enabled.
    #[must_use]
    pub const fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Returns whether the service is required to be running.
    #[must_use]
    pub const fn is_running(&self) -> bool {
        self.running
    }
}

/// Proposed Current State for one DAIA managed resource.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CurrentStateProposal<T> {
    resource_id: ResourceId,
    resource_type: ResourceType,
    schema_version: SchemaVersion,
    proposed: T,
}

impl<T> CurrentStateProposal<T> {
    /// Creates proposed Current State for a managed resource.
    #[must_use]
    pub fn new(
        resource_id: ResourceId,
        resource_type: ResourceType,
        schema_version: SchemaVersion,
        proposed: T,
    ) -> Self {
        Self {
            resource_id,
            resource_type,
            schema_version,
            proposed,
        }
    }

    /// Returns the managed resource identifier.
    #[must_use]
    pub fn resource_id(&self) -> &ResourceId {
        &self.resource_id
    }

    /// Returns the managed resource type.
    #[must_use]
    pub fn resource_type(&self) -> &ResourceType {
        &self.resource_type
    }

    /// Returns the resource schema version.
    #[must_use]
    pub const fn schema_version(&self) -> SchemaVersion {
        self.schema_version
    }

    /// Returns the proposed Current State payload.
    #[must_use]
    pub fn proposed(&self) -> &T {
        &self.proposed
    }

    /// Consumes the proposal and returns its proposed state.
    #[must_use]
    pub fn into_proposed(self) -> T {
        self.proposed
    }
}

/// Accepted Current State for one DAIA managed resource.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CurrentResource<T> {
    resource_id: ResourceId,
    resource_type: ResourceType,
    schema_version: SchemaVersion,
    revision: CurrentRevision,
    current: T,
}

impl<T> CurrentResource<T> {
    /// Creates accepted Current State for a managed resource.
    #[must_use]
    pub fn new(
        resource_id: ResourceId,
        resource_type: ResourceType,
        schema_version: SchemaVersion,
        revision: CurrentRevision,
        current: T,
    ) -> Self {
        Self {
            resource_id,
            resource_type,
            schema_version,
            revision,
            current,
        }
    }

    /// Returns the managed resource identifier.
    #[must_use]
    pub fn resource_id(&self) -> &ResourceId {
        &self.resource_id
    }

    /// Returns the managed resource type.
    #[must_use]
    pub fn resource_type(&self) -> &ResourceType {
        &self.resource_type
    }

    /// Returns the resource schema version.
    #[must_use]
    pub const fn schema_version(&self) -> SchemaVersion {
        self.schema_version
    }

    /// Returns the accepted Current State revision.
    #[must_use]
    pub const fn revision(&self) -> CurrentRevision {
        self.revision
    }

    /// Returns the accepted Current State payload.
    #[must_use]
    pub fn current(&self) -> &T {
        &self.current
    }
}

/// Accepted Current State for a system service.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ServiceCurrentState {
    present: bool,
    enabled: bool,
    running: bool,
}

impl ServiceCurrentState {
    /// Creates accepted Current State for a service.
    #[must_use]
    pub const fn new(present: bool, enabled: bool, running: bool) -> Self {
        Self {
            present,
            enabled,
            running,
        }
    }

    /// Returns whether the service is present.
    #[must_use]
    pub const fn is_present(&self) -> bool {
        self.present
    }

    /// Returns whether the service is enabled.
    #[must_use]
    pub const fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Returns whether the service is running.
    #[must_use]
    pub const fn is_running(&self) -> bool {
        self.running
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

/// Describes the confirmed configuration of a DAIA appliance.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApplianceConfiguration {
    profile_name: String,
    content_repository_id: ContentRepositoryId,
    content_import: ContentImportIntent,
    installation: InstallationIntent,
}

impl ApplianceConfiguration {
    /// Creates a confirmed appliance configuration.
    #[must_use]
    pub fn new(
        profile_name: impl Into<String>,
        content_repository_id: ContentRepositoryId,
        content_import: ContentImportIntent,
        installation: InstallationIntent,
    ) -> Self {
        Self {
            profile_name: profile_name.into(),
            content_repository_id,
            content_import,
            installation,
        }
    }

    /// Creates a confirmed appliance configuration from selected wizard values.
    #[must_use]
    pub fn from_selections(
        profile_name: impl Into<String>,
        content_repository_id: ContentRepositoryId,
        external_content: Vec<ExternalContentItemId>,
        storage_id: DiscoveredStorageId,
    ) -> Self {
        let profile_name = profile_name.into();

        Self::new(
            profile_name.clone(),
            content_repository_id,
            ContentImportIntent::new(external_content),
            InstallationIntent::new(profile_name, storage_id),
        )
    }

    /// Returns the selected appliance profile name.
    #[must_use]
    pub fn profile_name(&self) -> &str {
        &self.profile_name
    }

    /// Returns the selected content repository identifier.
    #[must_use]
    pub const fn content_repository_id(&self) -> &ContentRepositoryId {
        &self.content_repository_id
    }

    /// Returns the confirmed content import intent.
    #[must_use]
    pub const fn content_import(&self) -> &ContentImportIntent {
        &self.content_import
    }

    /// Returns the confirmed installation intent.
    #[must_use]
    pub const fn installation(&self) -> &InstallationIntent {
        &self.installation
    }
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
        Action, ApplianceConfiguration, ArchitecturalComponentId, AssetId, Capability,
        CapabilityId, ConditionResult, ContentImportDestination, ContentImportIntent,
        ContentRepository, ContentRepositoryId, ContentSource, ContentSourceId, CurrentResource,
        CurrentRevision, CurrentStateProposal, DesiredGeneration, DesiredResource,
        DiscoveredContent, DiscoveredStorage, DiscoveredStorageId, EvidenceId, EvidenceSourceId,
        ExternalContentItem, ExternalContentItemId, ImportedContentItem, InstallationIntent,
        Observation, ObservationSourceId, ObservationTimestamp, PackageManifest, PlanStep,
        ProviderId, ResourceId, ResourceType, SchemaVersion, ServiceCurrentState,
        ServiceDesiredState, StateBasis, StorageKind, StorageTarget, StorageTargetId,
        VerificationCondition, VerificationConditionId, VerificationConditionResult,
        VerificationEvidenceReference, VerificationOverallResult, VerificationPolicyRevision,
        VerificationProviderId, VerificationProviderVersion, VerificationPurpose,
        VerificationRequest, VerificationResult, VerificationResultId, VerificationRuleReference,
        VerificationRuleVersion, VerificationTimestamp,
    };
    use std::path::{Path, PathBuf};

    #[test]
    fn architectural_component_id_exposes_component_identity() {
        let component_id = ArchitecturalComponentId::new("reconciliation");

        assert_eq!(component_id.as_str(), "reconciliation");
        assert_eq!(component_id.to_string(), "reconciliation");
    }

    #[test]
    fn verification_condition_exposes_requirement() {
        let mandatory = VerificationCondition::new(VerificationConditionId::new("running"), true);
        let optional = VerificationCondition::new(VerificationConditionId::new("healthy"), false);

        assert_eq!(mandatory.condition_id().as_str(), "running");
        assert!(mandatory.is_mandatory());
        assert_eq!(optional.condition_id().as_str(), "healthy");
        assert!(!optional.is_mandatory());
    }

    #[test]
    fn verification_rule_reference_exposes_condition_and_version() {
        let rule = VerificationRuleReference::new(
            VerificationConditionId::new("running"),
            VerificationRuleVersion::new("service-running-v1"),
        );

        assert_eq!(rule.condition_id().as_str(), "running");
        assert_eq!(rule.version().as_str(), "service-running-v1");
    }

    #[test]
    fn verification_overall_result_exposes_supported_outcomes() {
        let outcomes = [
            VerificationOverallResult::Satisfied,
            VerificationOverallResult::Unsatisfied,
            VerificationOverallResult::Unknown,
            VerificationOverallResult::Error,
        ];

        assert_eq!(outcomes.len(), 4);
        assert_ne!(
            VerificationOverallResult::Unknown,
            VerificationOverallResult::Error
        );
    }

    #[test]
    fn verification_provider_version_exposes_provider_version() {
        let version = VerificationProviderVersion::new("system-service-verifier-v1");

        assert_eq!(version.as_str(), "system-service-verifier-v1");
        assert_eq!(version.to_string(), "system-service-verifier-v1");
    }

    #[test]
    fn verification_provider_id_exposes_provider_identity() {
        let provider_id = VerificationProviderId::new("system-service-verifier");

        assert_eq!(provider_id.as_str(), "system-service-verifier");
        assert_eq!(provider_id.to_string(), "system-service-verifier");
    }

    #[test]
    fn verification_evidence_reference_exposes_identity_and_collection_time() {
        let evidence = VerificationEvidenceReference::new(
            EvidenceId::new("observation/service/42"),
            ObservationTimestamp::new("2026-07-23T09:00:00Z"),
        );

        assert_eq!(evidence.evidence_id().as_str(), "observation/service/42");
        assert_eq!(evidence.collected_at().as_str(), "2026-07-23T09:00:00Z");
    }

    #[test]
    fn verification_condition_result_exposes_condition_and_result() {
        let condition = VerificationConditionResult::new(
            VerificationConditionId::new("running"),
            ConditionResult::Satisfied,
        );

        assert_eq!(condition.condition_id().as_str(), "running");
        assert_eq!(condition.result(), ConditionResult::Satisfied);
    }

    #[test]
    fn verification_rule_version_exposes_rule_version() {
        let version = VerificationRuleVersion::new("service-state-v1");

        assert_eq!(version.as_str(), "service-state-v1");
        assert_eq!(version.to_string(), "service-state-v1");
    }

    #[test]
    fn verification_result_exposes_result_contract() {
        let result = VerificationResult::new(
            VerificationResultId::new("verification/service/ollama/42"),
            ResourceId::new("service/ollama"),
            ResourceType::new("service"),
            VerificationPurpose::DesiredStateSatisfaction,
            StateBasis::new(DesiredGeneration::new(12), CurrentRevision::new(41)),
            Some(DesiredGeneration::new(12)),
            VerificationPolicyRevision::new("default-v1"),
            vec![VerificationRuleReference::new(
                VerificationConditionId::new("running"),
                VerificationRuleVersion::new("service-running-v1"),
            )],
            vec![VerificationEvidenceReference::new(
                EvidenceId::new("observation/service/42"),
                ObservationTimestamp::new("2026-07-23T09:00:00Z"),
            )],
            VerificationTimestamp::new("2026-07-23T09:05:00Z"),
            vec![VerificationConditionResult::new(
                VerificationConditionId::new("running"),
                ConditionResult::Satisfied,
            )],
            VerificationOverallResult::Satisfied,
            vec!["acceptable evidence proves the service is running".into()],
            vec!["evidence approaches its freshness limit".into()],
            VerificationProviderId::new("system-service-verifier"),
            VerificationProviderVersion::new("system-service-verifier-v1"),
            SchemaVersion::new(1),
        );

        assert_eq!(
            result.result_id(),
            &VerificationResultId::new("verification/service/ollama/42")
        );
        assert_eq!(result.resource_id(), &ResourceId::new("service/ollama"));
        assert_eq!(result.resource_type(), &ResourceType::new("service"));
        assert_eq!(
            result.purpose(),
            VerificationPurpose::DesiredStateSatisfaction
        );
        assert_eq!(
            result.state_basis(),
            StateBasis::new(DesiredGeneration::new(12), CurrentRevision::new(41))
        );
        assert_eq!(
            result.desired_generation(),
            Some(DesiredGeneration::new(12))
        );
        assert_eq!(
            result.policy_revision(),
            &VerificationPolicyRevision::new("default-v1")
        );

        assert_eq!(result.rules().len(), 1);
        assert_eq!(result.rules()[0].condition_id().as_str(), "running");
        assert_eq!(result.rules()[0].version().as_str(), "service-running-v1");

        assert_eq!(result.evidence().len(), 1);
        assert_eq!(
            result.evidence()[0].evidence_id().as_str(),
            "observation/service/42"
        );
        assert_eq!(
            result.evidence()[0].collected_at().as_str(),
            "2026-07-23T09:00:00Z"
        );

        assert_eq!(result.verified_at().as_str(), "2026-07-23T09:05:00Z");
        assert_eq!(result.conditions().len(), 1);
        assert_eq!(result.conditions()[0].condition_id().as_str(), "running");
        assert_eq!(result.conditions()[0].result(), ConditionResult::Satisfied);
        assert_eq!(
            result.overall_result(),
            VerificationOverallResult::Satisfied
        );
        assert_eq!(
            result.reasons(),
            &["acceptable evidence proves the service is running"]
        );
        assert_eq!(
            result.warnings(),
            &["evidence approaches its freshness limit"]
        );
        assert_eq!(
            result.provider_id(),
            &VerificationProviderId::new("system-service-verifier")
        );
        assert_eq!(
            result.provider_version(),
            &VerificationProviderVersion::new("system-service-verifier-v1")
        );
        assert_eq!(result.result_schema_version(), SchemaVersion::new(1));
    }

    #[test]
    fn verification_result_id_exposes_result_identity() {
        let result_id = VerificationResultId::new("verification/service/ollama/42");

        assert_eq!(result_id.as_str(), "verification/service/ollama/42");
        assert_eq!(result_id.to_string(), "verification/service/ollama/42");
    }

    #[test]
    fn verification_request_exposes_request_contract() {
        let request = VerificationRequest::new(
            ResourceId::new("service/ollama"),
            ResourceType::new("service"),
            SchemaVersion::new(1),
            VerificationPurpose::DesiredStateSatisfaction,
            StateBasis::new(DesiredGeneration::new(12), CurrentRevision::new(41)),
            Some(DesiredGeneration::new(12)),
            vec![
                VerificationCondition::new(VerificationConditionId::new("present"), true),
                VerificationCondition::new(VerificationConditionId::new("running"), true),
            ],
            vec![EvidenceId::new("observation/service/42")],
            vec![EvidenceSourceId::new("systemd")],
            VerificationPolicyRevision::new("default-v1"),
            VerificationTimestamp::new("2026-07-23T09:05:00Z"),
            ArchitecturalComponentId::new("reconciliation"),
        );

        assert_eq!(request.resource_id(), &ResourceId::new("service/ollama"));
        assert_eq!(request.resource_type(), &ResourceType::new("service"));
        assert_eq!(request.resource_schema_version(), SchemaVersion::new(1));
        assert_eq!(
            request.purpose(),
            VerificationPurpose::DesiredStateSatisfaction
        );
        assert_eq!(
            request.state_basis(),
            StateBasis::new(DesiredGeneration::new(12), CurrentRevision::new(41))
        );
        assert_eq!(
            request.desired_generation(),
            Some(DesiredGeneration::new(12))
        );
        assert_eq!(request.expected_conditions().len(), 2);
        assert_eq!(
            request.expected_conditions()[0].condition_id().as_str(),
            "present"
        );
        assert!(request.expected_conditions()[0].is_mandatory());
        assert_eq!(
            request.expected_conditions()[1].condition_id().as_str(),
            "running"
        );
        assert!(request.expected_conditions()[1].is_mandatory());
        assert_eq!(
            request.evidence_ids(),
            &[EvidenceId::new("observation/service/42")]
        );
        assert_eq!(
            request.authorized_evidence_sources(),
            &[EvidenceSourceId::new("systemd")]
        );
        assert_eq!(
            request.policy_revision(),
            &VerificationPolicyRevision::new("default-v1")
        );
        assert_eq!(
            request.requested_at(),
            &VerificationTimestamp::new("2026-07-23T09:05:00Z")
        );
        assert_eq!(
            request.requesting_component(),
            &ArchitecturalComponentId::new("reconciliation")
        );
    }

    #[test]
    fn verification_policy_revision_exposes_policy_identity() {
        let revision = VerificationPolicyRevision::new("default-v1");

        assert_eq!(revision.as_str(), "default-v1");
        assert_eq!(revision.to_string(), "default-v1");
    }

    #[test]
    fn verification_timestamp_exposes_its_representation() {
        let timestamp = VerificationTimestamp::new("2026-07-23T09:05:00Z");

        assert_eq!(timestamp.as_str(), "2026-07-23T09:05:00Z");
        assert_eq!(timestamp.to_string(), "2026-07-23T09:05:00Z");
    }

    #[test]
    fn evidence_source_id_exposes_source_identity() {
        let source_id = EvidenceSourceId::new("systemd");

        assert_eq!(source_id.as_str(), "systemd");
        assert_eq!(source_id.to_string(), "systemd");
    }

    #[test]
    fn evidence_id_exposes_evidence_identity() {
        let evidence_id = EvidenceId::new("observation/system-service/42");

        assert_eq!(evidence_id.as_str(), "observation/system-service/42");
        assert_eq!(evidence_id.to_string(), "observation/system-service/42");
    }

    #[test]
    fn verification_condition_id_exposes_condition_name() {
        let condition_id = VerificationConditionId::new("running");

        assert_eq!(condition_id.as_str(), "running");
        assert_eq!(condition_id.to_string(), "running");
    }

    #[test]
    fn resource_type_exposes_resource_classification() {
        let resource_type = ResourceType::new("service");

        assert_eq!(resource_type.as_str(), "service");
        assert_eq!(resource_type.to_string(), "service");
    }

    #[test]
    fn state_basis_preserves_state_coordinates() {
        let basis = StateBasis::new(DesiredGeneration::new(12), CurrentRevision::new(41));

        assert_eq!(basis.desired_generation(), DesiredGeneration::new(12));
        assert_eq!(basis.current_revision(), CurrentRevision::new(41));
    }

    #[test]
    fn current_revision_exposes_and_orders_revision() {
        let revision = CurrentRevision::new(41);
        let newer_revision = CurrentRevision::new(42);

        assert_eq!(revision.value(), 41);
        assert_eq!(revision.to_string(), "41");
        assert!(revision < newer_revision);
    }

    #[test]
    fn desired_generation_exposes_and_orders_generation() {
        let generation = DesiredGeneration::new(12);
        let newer_generation = DesiredGeneration::new(13);

        assert_eq!(generation.value(), 12);
        assert_eq!(generation.to_string(), "12");
        assert!(generation < newer_generation);
    }

    #[test]
    fn condition_result_preserves_architectural_results() {
        let results = [
            ConditionResult::Satisfied,
            ConditionResult::Unsatisfied,
            ConditionResult::Unknown,
            ConditionResult::NotApplicable,
            ConditionResult::Error,
        ];

        assert_eq!(results.len(), 5);
        assert_ne!(ConditionResult::Unknown, ConditionResult::Error);
    }

    #[test]
    fn verification_purpose_preserves_architectural_purposes() {
        let purposes = [
            VerificationPurpose::CurrentStateEstablishment,
            VerificationPurpose::DesiredStateSatisfaction,
            VerificationPurpose::TransitionPreconditions,
            VerificationPurpose::TransitionPostconditions,
            VerificationPurpose::RecoveryVerification,
            VerificationPurpose::DriftVerification,
            VerificationPurpose::IntegrityVerification,
            VerificationPurpose::HealthVerification,
        ];

        assert_eq!(purposes.len(), 8);
    }

    #[test]
    fn appliance_configuration_builds_confirmed_intents_from_selections() {
        let item_id =
            ExternalContentItemId::new("local-models-directory:/media/daia/models/model.gguf");

        let configuration = ApplianceConfiguration::from_selections(
            "desktop",
            ContentRepositoryId::new("local-models"),
            vec![item_id.clone()],
            DiscoveredStorageId::new("serial:usb-disk"),
        );

        assert_eq!(configuration.profile_name(), "desktop");
        assert_eq!(
            configuration.content_repository_id(),
            &ContentRepositoryId::new("local-models")
        );
        assert_eq!(configuration.content_import().items(), &[item_id]);
        assert_eq!(configuration.installation().profile_name(), "desktop");
        assert_eq!(
            configuration.installation().storage_id(),
            &DiscoveredStorageId::new("serial:usb-disk")
        );
    }

    #[test]
    fn appliance_configuration_exposes_confirmed_intents() {
        let content_import = ContentImportIntent::new(vec![ExternalContentItemId::new(
            "local-models-directory:/media/daia/models/model.gguf",
        )]);

        let installation =
            InstallationIntent::new("desktop", DiscoveredStorageId::new("serial:usb-disk"));

        let configuration = ApplianceConfiguration::new(
            "desktop",
            ContentRepositoryId::new("local-models"),
            content_import.clone(),
            installation.clone(),
        );

        assert_eq!(configuration.profile_name(), "desktop");
        assert_eq!(
            configuration.content_repository_id(),
            &ContentRepositoryId::new("local-models")
        );
        assert_eq!(configuration.content_import(), &content_import);
        assert_eq!(configuration.installation(), &installation);
    }
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
    fn discovered_storage_exposes_size_bytes() {
        let storage = DiscoveredStorage::new("disk-1", StorageKind::Secondary, "/dev/sdb")
            .with_size_bytes(1_000_204_886_016);

        assert_eq!(storage.size_bytes(), Some(1_000_204_886_016));
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
    fn observation_exposes_collected_resource_evidence() {
        #[derive(Debug, Eq, PartialEq)]
        struct ServiceObservation {
            present: bool,
            enabled: bool,
        }

        let observation = Observation::new(
            ResourceId::new("service/ollama"),
            ObservationSourceId::new("system-service-observer"),
            ObservationTimestamp::new("2026-07-23T09:00:00Z"),
            SchemaVersion::new(1),
            ServiceObservation {
                present: true,
                enabled: true,
            },
        );

        assert_eq!(observation.resource_id().as_str(), "service/ollama");
        assert_eq!(observation.source_id().as_str(), "system-service-observer");
        assert_eq!(observation.collected_at().as_str(), "2026-07-23T09:00:00Z");
        assert_eq!(observation.schema_version(), SchemaVersion::new(1));
        assert_eq!(
            observation.observed(),
            &ServiceObservation {
                present: true,
                enabled: true,
            }
        );
    }

    #[test]
    fn observation_timestamp_exposes_its_representation() {
        let timestamp = ObservationTimestamp::new("2026-07-23T09:00:00Z");

        assert_eq!(timestamp.as_str(), "2026-07-23T09:00:00Z");
        assert_eq!(timestamp.to_string(), "2026-07-23T09:00:00Z");
    }

    #[test]
    fn schema_version_exposes_its_value() {
        let version = SchemaVersion::new(1);

        assert_eq!(version.value(), 1);
        assert_eq!(version.to_string(), "1");
    }

    #[test]
    fn observation_source_id_exposes_its_identifier() {
        let source_id = ObservationSourceId::new("systemd");

        assert_eq!(source_id.as_str(), "systemd");
        assert_eq!(source_id.to_string(), "systemd");
    }

    #[test]
    fn resource_id_exposes_its_identifier() {
        let resource_id = ResourceId::new("service:lightdm");

        assert_eq!(resource_id.as_str(), "service:lightdm");
        assert_eq!(resource_id.to_string(), "service:lightdm");
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
    fn desired_resource_exposes_service_desired_state() {
        let desired = DesiredResource::new(
            ResourceId::new("service/ollama"),
            ResourceType::new("service"),
            SchemaVersion::new(1),
            DesiredGeneration::new(12),
            ServiceDesiredState::new(true, true, true),
        );

        assert_eq!(desired.resource_id().as_str(), "service/ollama");
        assert_eq!(desired.resource_type().as_str(), "service");
        assert_eq!(desired.schema_version(), SchemaVersion::new(1));
        assert_eq!(desired.generation(), DesiredGeneration::new(12));
        assert!(desired.desired().is_present());
        assert!(desired.desired().is_enabled());
        assert!(desired.desired().is_running());
    }

    #[test]
    fn current_state_proposal_exposes_service_current_state() {
        let proposal = CurrentStateProposal::new(
            ResourceId::new("service/ollama"),
            ResourceType::new("service"),
            SchemaVersion::new(1),
            ServiceCurrentState::new(true, true, true),
        );

        assert_eq!(proposal.resource_id().as_str(), "service/ollama");
        assert_eq!(proposal.resource_type().as_str(), "service");
        assert_eq!(proposal.schema_version(), SchemaVersion::new(1));
        assert!(proposal.proposed().is_present());
        assert!(proposal.proposed().is_enabled());
        assert!(proposal.proposed().is_running());
    }

    #[test]
    fn current_resource_exposes_service_current_state() {
        let current = CurrentResource::new(
            ResourceId::new("service/ollama"),
            ResourceType::new("service"),
            SchemaVersion::new(1),
            CurrentRevision::new(41),
            ServiceCurrentState::new(true, false, false),
        );

        assert_eq!(current.resource_id().as_str(), "service/ollama");
        assert_eq!(current.resource_type().as_str(), "service");
        assert_eq!(current.schema_version(), SchemaVersion::new(1));
        assert_eq!(current.revision(), CurrentRevision::new(41));
        assert!(current.current().is_present());
        assert!(!current.current().is_enabled());
        assert!(!current.current().is_running());
    }

    #[test]
    fn service_current_state_exposes_verified_condition() {
        let current = ServiceCurrentState::new(true, false, false);

        assert!(current.is_present());
        assert!(!current.is_enabled());
        assert!(!current.is_running());
    }

    #[test]
    fn service_desired_state_exposes_requirements() {
        let desired = ServiceDesiredState::new(true, true, true);

        assert!(desired.is_present());
        assert!(desired.is_enabled());
        assert!(desired.is_running());
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
