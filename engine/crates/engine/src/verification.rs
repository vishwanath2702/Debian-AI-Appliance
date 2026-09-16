use model::{
    ConditionResult, VerificationCondition, VerificationConditionResult, VerificationOverallResult,
};

pub(crate) fn aggregate_verification_result(
    expected_conditions: &[VerificationCondition],
    condition_results: &[VerificationConditionResult],
) -> VerificationOverallResult {
    let mut overall = VerificationOverallResult::Satisfied;

    for expected in expected_conditions
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
        ConditionResult, VerificationCondition, VerificationConditionId,
        VerificationConditionResult, VerificationOverallResult,
    };

    fn expected(name: &str, mandatory: bool) -> VerificationCondition {
        VerificationCondition::new(VerificationConditionId::new(name), mandatory)
    }

    fn result(name: &str, value: ConditionResult) -> VerificationConditionResult {
        VerificationConditionResult::new(VerificationConditionId::new(name), value)
    }

    #[test]
    fn aggregates_mandatory_condition_results() {
        let expected_conditions = vec![
            expected("present", true),
            expected("enabled", true),
            expected("running", true),
        ];

        assert_eq!(
            aggregate_verification_result(
                &expected_conditions,
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
                &expected_conditions,
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
                &expected_conditions,
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
                &expected_conditions,
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
        let expected_conditions = [expected("present", true), expected("running", true)];

        assert_eq!(
            aggregate_verification_result(
                &expected_conditions,
                &[result("present", ConditionResult::Satisfied)],
            ),
            VerificationOverallResult::Unknown
        );

        assert_eq!(
            aggregate_verification_result(
                &expected_conditions,
                &[result("running", ConditionResult::Error)],
            ),
            VerificationOverallResult::Error
        );
    }

    #[test]
    fn optional_and_not_applicable_conditions_do_not_prevent_satisfaction() {
        assert_eq!(
            aggregate_verification_result(
                &[
                    expected("present", true),
                    expected("health", false),
                    expected("legacy", true),
                ],
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
