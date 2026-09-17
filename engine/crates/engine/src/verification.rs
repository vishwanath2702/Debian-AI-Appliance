use model::{
    ConditionResult, VerificationConditionResult, VerificationOverallResult, VerificationRequest,
};

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

#[cfg(test)]
mod tests {
    use super::aggregate_verification_result;
    use model::{
        ArchitecturalComponentId, ConditionResult, CurrentRevision, DesiredGeneration,
        EvidenceSourceId, ResourceId, ResourceType, SchemaVersion, StateBasis,
        VerificationCondition, VerificationConditionId, VerificationConditionResult,
        VerificationOverallResult, VerificationPolicyRevision, VerificationPurpose,
        VerificationRequest, VerificationTimestamp,
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
