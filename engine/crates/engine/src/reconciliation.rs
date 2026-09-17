use model::{ServiceCurrentState, ServiceDesiredState};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ServiceStateDifference {
    Present,
    Enabled,
    Running,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ServiceTransition {
    Install,
    Remove,
    Enable,
    Disable,
    Start,
    Stop,
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

trait ServiceTransitionExecutor {
    type Error;

    fn execute(&mut self, service: &str, transition: ServiceTransition) -> Result<(), Self::Error>;
}

trait ServiceCommandRunner {
    fn status(&mut self, command: &mut std::process::Command) -> std::io::Result<()>;
}

struct SystemServiceTransitionExecutor<R> {
    runner: R,
}

impl<R> SystemServiceTransitionExecutor<R> {
    fn new(runner: R) -> Self {
        Self { runner }
    }
}

impl<R> ServiceTransitionExecutor for SystemServiceTransitionExecutor<R>
where
    R: ServiceCommandRunner,
{
    type Error = std::io::Error;

    fn execute(&mut self, service: &str, transition: ServiceTransition) -> Result<(), Self::Error> {
        match transition {
            ServiceTransition::Start => {
                let mut command = std::process::Command::new("systemctl");
                command.arg("start").arg(service);
                self.runner.status(&mut command)
            }
            ServiceTransition::Stop => {
                let mut command = std::process::Command::new("systemctl");
                command.arg("stop").arg(service);
                self.runner.status(&mut command)
            }
            _ => Ok(()),
        }
    }
}

fn execute_service_transitions<E>(
    service: &str,
    transitions: &[ServiceTransition],
    executor: &mut E,
) -> Result<(), E::Error>
where
    E: ServiceTransitionExecutor,
{
    for transition in transitions {
        executor.execute(service, *transition)?;
    }

    Ok(())
}

fn service_transitions(
    desired: &ServiceDesiredState,
    current: &ServiceCurrentState,
) -> Vec<ServiceTransition> {
    let mut transitions = Vec::new();

    if desired.is_running() != current.is_running() && !desired.is_running() {
        transitions.push(ServiceTransition::Stop);
    }

    if desired.is_enabled() != current.is_enabled() && !desired.is_enabled() {
        transitions.push(ServiceTransition::Disable);
    }

    if desired.is_present() != current.is_present() {
        transitions.push(if desired.is_present() {
            ServiceTransition::Install
        } else {
            ServiceTransition::Remove
        });
    }

    if desired.is_enabled() != current.is_enabled() && desired.is_enabled() {
        transitions.push(ServiceTransition::Enable);
    }

    if desired.is_running() != current.is_running() && desired.is_running() {
        transitions.push(ServiceTransition::Start);
    }

    transitions
}

pub(crate) fn service_state_differs(
    desired: &ServiceDesiredState,
    current: &ServiceCurrentState,
) -> bool {
    !service_transitions(desired, current).is_empty()
}

#[cfg(test)]
mod tests {
    use super::{
        ServiceCommandRunner, ServiceStateDifference, ServiceTransition, ServiceTransitionExecutor,
        SystemServiceTransitionExecutor, execute_service_transitions, service_state_differences,
        service_state_differs, service_transitions,
    };
    use model::{ServiceCurrentState, ServiceDesiredState};

    struct RecordingServiceTransitionExecutor {
        transitions: Vec<ServiceTransition>,
    }

    impl ServiceTransitionExecutor for RecordingServiceTransitionExecutor {
        type Error = ();

        fn execute(
            &mut self,
            service: &str,
            transition: ServiceTransition,
        ) -> Result<(), Self::Error> {
            assert_eq!(service, "ollama");
            self.transitions.push(transition);
            Ok(())
        }
    }

    struct RecordingServiceCommandRunner {
        program: Option<std::ffi::OsString>,
        args: Vec<std::ffi::OsString>,
    }

    impl ServiceCommandRunner for RecordingServiceCommandRunner {
        fn status(&mut self, command: &mut std::process::Command) -> std::io::Result<()> {
            self.program = Some(command.get_program().to_os_string());
            self.args = command.get_args().map(std::ffi::OsString::from).collect();
            Ok(())
        }
    }

    #[test]
    fn system_executor_sends_stop_service_command_to_runner() {
        let runner = RecordingServiceCommandRunner {
            program: None,
            args: Vec::new(),
        };
        let mut executor = SystemServiceTransitionExecutor::new(runner);

        executor.execute("ollama", ServiceTransition::Stop).unwrap();

        assert_eq!(
            executor.runner.program,
            Some(std::ffi::OsString::from("systemctl"))
        );
        assert_eq!(
            executor.runner.args,
            vec![
                std::ffi::OsString::from("stop"),
                std::ffi::OsString::from("ollama"),
            ]
        );
    }

    #[test]
    fn system_executor_sends_start_service_command_to_runner() {
        let runner = RecordingServiceCommandRunner {
            program: None,
            args: Vec::new(),
        };
        let mut executor = SystemServiceTransitionExecutor::new(runner);

        executor
            .execute("ollama", ServiceTransition::Start)
            .unwrap();

        assert_eq!(
            executor.runner.program,
            Some(std::ffi::OsString::from("systemctl"))
        );
        assert_eq!(
            executor.runner.args,
            vec![
                std::ffi::OsString::from("start"),
                std::ffi::OsString::from("ollama"),
            ]
        );
    }

    #[test]
    fn executes_service_transitions_in_order() {
        let transitions = vec![
            ServiceTransition::Install,
            ServiceTransition::Enable,
            ServiceTransition::Start,
        ];
        let mut executor = RecordingServiceTransitionExecutor {
            transitions: Vec::new(),
        };

        execute_service_transitions("ollama", &transitions, &mut executor).unwrap();

        assert_eq!(executor.transitions, transitions);
    }

    #[test]
    fn stops_service_transition_execution_after_failure() {
        struct FailingServiceTransitionExecutor {
            transitions: Vec<ServiceTransition>,
        }

        impl ServiceTransitionExecutor for FailingServiceTransitionExecutor {
            type Error = &'static str;

            fn execute(
                &mut self,
                service: &str,
                transition: ServiceTransition,
            ) -> Result<(), Self::Error> {
                assert_eq!(service, "ollama");
                self.transitions.push(transition);

                if transition == ServiceTransition::Enable {
                    return Err("enable failed");
                }

                Ok(())
            }
        }

        let transitions = vec![
            ServiceTransition::Install,
            ServiceTransition::Enable,
            ServiceTransition::Start,
        ];
        let mut executor = FailingServiceTransitionExecutor {
            transitions: Vec::new(),
        };

        let result = execute_service_transitions("ollama", &transitions, &mut executor);

        assert_eq!(result, Err("enable failed"));
        assert_eq!(
            executor.transitions,
            vec![ServiceTransition::Install, ServiceTransition::Enable]
        );
    }

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
    fn plans_service_transitions() {
        assert_eq!(
            service_transitions(
                &ServiceDesiredState::new(true, true, true),
                &ServiceCurrentState::new(false, false, false),
            ),
            vec![
                ServiceTransition::Install,
                ServiceTransition::Enable,
                ServiceTransition::Start,
            ]
        );

        assert_eq!(
            service_transitions(
                &ServiceDesiredState::new(false, false, false),
                &ServiceCurrentState::new(true, true, true),
            ),
            vec![
                ServiceTransition::Stop,
                ServiceTransition::Disable,
                ServiceTransition::Remove,
            ]
        );
    }

    #[test]
    fn plans_individual_service_transitions() {
        assert_eq!(
            service_transitions(
                &ServiceDesiredState::new(true, false, false),
                &ServiceCurrentState::new(false, false, false),
            ),
            vec![ServiceTransition::Install]
        );

        assert_eq!(
            service_transitions(
                &ServiceDesiredState::new(false, false, false),
                &ServiceCurrentState::new(true, false, false),
            ),
            vec![ServiceTransition::Remove]
        );

        assert_eq!(
            service_transitions(
                &ServiceDesiredState::new(true, true, false),
                &ServiceCurrentState::new(true, false, false),
            ),
            vec![ServiceTransition::Enable]
        );

        assert_eq!(
            service_transitions(
                &ServiceDesiredState::new(true, false, false),
                &ServiceCurrentState::new(true, true, false),
            ),
            vec![ServiceTransition::Disable]
        );

        assert_eq!(
            service_transitions(
                &ServiceDesiredState::new(true, false, true),
                &ServiceCurrentState::new(true, false, false),
            ),
            vec![ServiceTransition::Start]
        );

        assert_eq!(
            service_transitions(
                &ServiceDesiredState::new(true, false, false),
                &ServiceCurrentState::new(true, false, true),
            ),
            vec![ServiceTransition::Stop]
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
