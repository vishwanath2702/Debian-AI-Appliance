use model::{ServiceCurrentState, ServiceDesiredState};

pub(crate) fn service_state_differs(
    desired: &ServiceDesiredState,
    current: &ServiceCurrentState,
) -> bool {
    desired.is_present() != current.is_present()
        || desired.is_enabled() != current.is_enabled()
        || desired.is_running() != current.is_running()
}

#[cfg(test)]
mod tests {
    use super::service_state_differs;
    use model::{ServiceCurrentState, ServiceDesiredState};

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
