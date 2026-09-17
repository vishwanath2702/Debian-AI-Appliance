use model::{
    ConditionResult, SchemaVersion, ServiceDesiredState, VerificationConditionId,
    VerificationConditionResult, VerificationEvidenceReference, VerificationOverallResult,
    VerificationProviderId, VerificationProviderVersion, VerificationRequest, VerificationResult,
    VerificationResultId, VerificationRuleReference, VerificationTimestamp,
};

fn service_running(active_state: &str) -> Option<bool> {
    match active_state {
        "active" | "reloading" => Some(true),
        "inactive" | "failed" => Some(false),
        "activating" | "deactivating" => None,
        _ => None,
    }
}

pub(crate) fn evaluate_service(
    desired: &ServiceDesiredState,
    observed: &facts::ServiceFacts,
) -> Vec<VerificationConditionResult> {
    let condition = |name, satisfied| {
        VerificationConditionResult::new(
            VerificationConditionId::new(name),
            if satisfied {
                ConditionResult::Satisfied
            } else {
                ConditionResult::Unsatisfied
            },
        )
    };

    let running = match service_running(observed.active_state()) {
        Some(running) => condition("running", running == desired.is_running()),
        None => VerificationConditionResult::new(
            VerificationConditionId::new("running"),
            ConditionResult::Unknown,
        ),
    };

    vec![
        condition("present", observed.is_present() == desired.is_present()),
        condition("enabled", observed.is_enabled() == desired.is_enabled()),
        running,
    ]
}

pub(crate) fn aggregate_verification_result(
    request: &VerificationRequest,
    condition_results: &[VerificationConditionResult],
) -> VerificationOverallResult {
    let mut overall = VerificationOverallResult::Satisfied;

    for expected in request
        .expected_conditions()
        .iter()
        .filter(|condition| condition.is_mandatory())
    {
        let Some(result) = condition_results
            .iter()
            .find(|result| result.condition_id() == expected.condition_id())
        else {
            overall = VerificationOverallResult::Unknown;
            continue;
        };

        match result.result() {
            ConditionResult::Error => return VerificationOverallResult::Error,
            ConditionResult::Unknown => overall = VerificationOverallResult::Unknown,
            ConditionResult::Unsatisfied if overall != VerificationOverallResult::Unknown => {
                overall = VerificationOverallResult::Unsatisfied;
            }
            ConditionResult::Satisfied | ConditionResult::NotApplicable => {}
            ConditionResult::Unsatisfied => {}
        }
    }

    overall
}

pub(crate) fn build_verification_result(
    request: &VerificationRequest,
    result_id: VerificationResultId,
    rules: Vec<VerificationRuleReference>,
    evidence: Vec<VerificationEvidenceReference>,
    verified_at: VerificationTimestamp,
    condition_results: Vec<VerificationConditionResult>,
    reasons: Vec<String>,
    warnings: Vec<String>,
    provider_id: VerificationProviderId,
    provider_version: VerificationProviderVersion,
) -> VerificationResult {
    let overall_result = aggregate_verification_result(request, &condition_results);

    VerificationResult::new(
        result_id,
        request.resource_id().clone(),
        request.resource_type().clone(),
        request.purpose(),
        request.state_basis(),
        request.desired_generation(),
        request.policy_revision().clone(),
        rules,
        evidence,
        verified_at,
        condition_results,
        overall_result,
        reasons,
        warnings,
        provider_id,
        provider_version,
        SchemaVersion::new(1),
    )
}

#[cfg(test)]
mod tests {
    use super::{
        aggregate_verification_result, build_verification_result, evaluate_service, service_running,
    };
    use model::{
        ArchitecturalComponentId, ConditionResult, CurrentRevision, DesiredGeneration, EvidenceId,
        EvidenceSourceId, ObservationTimestamp, ResourceId, ResourceType, SchemaVersion,
        ServiceDesiredState, StateBasis, VerificationCondition, VerificationConditionId,
        VerificationConditionResult, VerificationEvidenceReference, VerificationOverallResult,
        VerificationPolicyRevision, VerificationProviderId, VerificationProviderVersion,
        VerificationPurpose, VerificationRequest, VerificationResultId, VerificationRuleReference,
        VerificationRuleVersion, VerificationTimestamp,
    };

    fn expected(name: &str, mandatory: bool) -> VerificationCondition {
        VerificationCondition::new(VerificationConditionId::new(name), mandatory)
    }

    fn result(name: &str, value: ConditionResult) -> VerificationConditionResult {
        VerificationConditionResult::new(VerificationConditionId::new(name), value)
    }

    fn request(expected_conditions: Vec<VerificationCondition>) -> VerificationRequest {
        VerificationRequest::new(
            ResourceId::new("service/ollama"),
            ResourceType::new("service"),
            SchemaVersion::new(1),
            VerificationPurpose::DesiredStateSatisfaction,
            StateBasis::new(DesiredGeneration::new(1), CurrentRevision::new(1)),
            Some(DesiredGeneration::new(1)),
            expected_conditions,
            Vec::new(),
            vec![EvidenceSourceId::new("systemd")],
            VerificationPolicyRevision::new("default-v1"),
            VerificationTimestamp::new("2026-09-17T00:00:00Z"),
            ArchitecturalComponentId::new("test"),
        )
    }

    #[test]
    fn builds_verification_result_from_request_and_evaluation() {
        let request = request(vec![
            expected("present", true),
            expected("enabled", true),
            expected("running", true),
        ]);
        let conditions = vec![
            result("present", ConditionResult::Satisfied),
            result("enabled", ConditionResult::Unsatisfied),
            result("running", ConditionResult::Satisfied),
        ];

        let verification = build_verification_result(
            &request,
            VerificationResultId::new("verification/service/ollama/1"),
            vec![VerificationRuleReference::new(
                VerificationConditionId::new("running"),
                VerificationRuleVersion::new("service-running-v1"),
            )],
            vec![VerificationEvidenceReference::new(
                EvidenceId::new("observation/service/1"),
                ObservationTimestamp::new("2026-09-17T00:00:00Z"),
            )],
            VerificationTimestamp::new("2026-09-17T00:01:00Z"),
            conditions.clone(),
            vec!["service state evaluated".into()],
            Vec::new(),
            VerificationProviderId::new("system-service-verifier"),
            VerificationProviderVersion::new("system-service-verifier-v1"),
        );

        assert_eq!(verification.resource_id(), request.resource_id());
        assert_eq!(verification.resource_type(), request.resource_type());
        assert_eq!(verification.purpose(), request.purpose());
        assert_eq!(verification.state_basis(), request.state_basis());
        assert_eq!(
            verification.desired_generation(),
            request.desired_generation()
        );
        assert_eq!(verification.policy_revision(), request.policy_revision());
        assert_eq!(verification.conditions(), conditions);
        assert_eq!(
            verification.overall_result(),
            VerificationOverallResult::Unsatisfied
        );
        assert_eq!(verification.result_schema_version(), SchemaVersion::new(1));
    }

    #[test]
    fn evaluates_service_desired_state_against_observed_facts() {
        let desired = ServiceDesiredState::new(true, true, true);
        let observed = facts::ServiceFacts::new(true, true, "active");

        assert_eq!(
            evaluate_service(&desired, &observed),
            vec![
                result("present", ConditionResult::Satisfied),
                result("enabled", ConditionResult::Satisfied),
                result("running", ConditionResult::Satisfied),
            ]
        );

        let desired = ServiceDesiredState::new(true, false, false);
        let observed = facts::ServiceFacts::new(true, true, "inactive");

        assert_eq!(
            evaluate_service(&desired, &observed),
            vec![
                result("present", ConditionResult::Satisfied),
                result("enabled", ConditionResult::Unsatisfied),
                result("running", ConditionResult::Satisfied),
            ]
        );

        let observed = facts::ServiceFacts::new(true, false, "activating");

        assert_eq!(
            evaluate_service(&desired, &observed),
            vec![
                result("present", ConditionResult::Satisfied),
                result("enabled", ConditionResult::Satisfied),
                result("running", ConditionResult::Unknown),
            ]
        );
    }

    #[test]
    fn interprets_systemd_active_state_as_service_running_state() {
        assert_eq!(service_running("active"), Some(true));
        assert_eq!(service_running("reloading"), Some(true));

        assert_eq!(service_running("inactive"), Some(false));
        assert_eq!(service_running("failed"), Some(false));

        assert_eq!(service_running("activating"), None);
        assert_eq!(service_running("deactivating"), None);
        assert_eq!(service_running("maintenance"), None);
    }

    #[test]
    fn aggregates_mandatory_condition_results() {
        let request = request(vec![
            expected("present", true),
            expected("enabled", true),
            expected("running", true),
        ]);

        assert_eq!(
            aggregate_verification_result(
                &request,
                &[
                    result("present", ConditionResult::Satisfied),
                    result("enabled", ConditionResult::Satisfied),
                    result("running", ConditionResult::Satisfied),
                ],
            ),
            VerificationOverallResult::Satisfied
        );

        assert_eq!(
            aggregate_verification_result(
                &request,
                &[
                    result("present", ConditionResult::Satisfied),
                    result("enabled", ConditionResult::Unsatisfied),
                    result("running", ConditionResult::Satisfied),
                ],
            ),
            VerificationOverallResult::Unsatisfied
        );

        assert_eq!(
            aggregate_verification_result(
                &request,
                &[
                    result("present", ConditionResult::Satisfied),
                    result("enabled", ConditionResult::Unsatisfied),
                    result("running", ConditionResult::Unknown),
                ],
            ),
            VerificationOverallResult::Unknown
        );

        assert_eq!(
            aggregate_verification_result(
                &request,
                &[
                    result("present", ConditionResult::Unknown),
                    result("enabled", ConditionResult::Error),
                    result("running", ConditionResult::Unsatisfied),
                ],
            ),
            VerificationOverallResult::Error
        );
    }

    #[test]
    fn missing_mandatory_condition_is_unknown_unless_another_condition_errors() {
        let request = request(vec![expected("present", true), expected("running", true)]);

        assert_eq!(
            aggregate_verification_result(
                &request,
                &[result("present", ConditionResult::Satisfied)],
            ),
            VerificationOverallResult::Unknown
        );

        assert_eq!(
            aggregate_verification_result(&request, &[result("running", ConditionResult::Error)],),
            VerificationOverallResult::Error
        );
    }

    #[test]
    fn optional_and_not_applicable_conditions_do_not_prevent_satisfaction() {
        let request = request(vec![
            expected("present", true),
            expected("health", false),
            expected("legacy", true),
        ]);

        assert_eq!(
            aggregate_verification_result(
                &request,
                &[
                    result("present", ConditionResult::Satisfied),
                    result("health", ConditionResult::Error),
                    result("legacy", ConditionResult::NotApplicable),
                ],
            ),
            VerificationOverallResult::Satisfied
        );
    }
}
