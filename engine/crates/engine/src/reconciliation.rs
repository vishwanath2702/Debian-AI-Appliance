use model::{ServiceCurrentState, ServiceDesiredState};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ServiceStateDifference {
    Present,
    Enabled,
    Running,
}

fn service_state_differences(
    desired: &ServiceDesiredState,
    current: &ServiceCurrentState,
) -> Vec<ServiceStateDifference> {
    let mut differences = Vec::new();

    if desired.is_present() != current.is_present() {
        differences.push(ServiceStateDifference::Present);
    }

    if desired.is_enabled() != current.is_enabled() {
        differences.push(ServiceStateDifference::Enabled);
    }

    if desired.is_running() != current.is_running() {
        differences.push(ServiceStateDifference::Running);
    }

    differences
}

pub(crate) fn service_state_differs(
    desired: &ServiceDesiredState,
    current: &ServiceCurrentState,
) -> bool {
    !service_state_differences(desired, current).is_empty()
}

#[cfg(test)]
mod tests {
    use super::{ServiceStateDifference, service_state_differences, service_state_differs};
    use model::{ServiceCurrentState, ServiceDesiredState};

    #[test]
    fn identifies_service_state_differences() {
        let desired = ServiceDesiredState::new(true, false, true);
        let current = ServiceCurrentState::new(false, true, true);

        assert_eq!(
            service_state_differences(&desired, &current),
            vec![
                ServiceStateDifference::Present,
                ServiceStateDifference::Enabled,
            ]
        );
    }

    #[test]
    fn identifies_no_and_all_service_state_differences() {
        assert_eq!(
            service_state_differences(
                &ServiceDesiredState::new(true, true, true),
                &ServiceCurrentState::new(true, true, true),
            ),
            vec![]
        );

        assert_eq!(
            service_state_differences(
                &ServiceDesiredState::new(true, true, true),
                &ServiceCurrentState::new(false, false, false),
            ),
            vec![
                ServiceStateDifference::Present,
                ServiceStateDifference::Enabled,
                ServiceStateDifference::Running,
            ]
        );
    }

    #[test]
    fn detects_service_state_difference() {
        let desired = ServiceDesiredState::new(true, true, true);

        assert!(!service_state_differs(
            &desired,
            &ServiceCurrentState::new(true, true, true),
        ));

        assert!(service_state_differs(
            &desired,
            &ServiceCurrentState::new(false, true, true),
        ));

        assert!(service_state_differs(
            &desired,
            &ServiceCurrentState::new(true, false, true),
        ));

        assert!(service_state_differs(
            &desired,
            &ServiceCurrentState::new(true, true, false),
        ));
    }
}
