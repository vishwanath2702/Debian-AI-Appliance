//! Execution of planned system actions.

use model::{Action, Plan};

/// Executes one action from a plan.
///
/// Implementations decide how actions are applied. Production implementations
/// may operate on a root filesystem, while tests can use an in-memory runner.
pub trait ActionRunner {
    /// Error returned when an action cannot be executed.
    type Error;

    /// Executes one action.
    ///
    /// # Errors
    ///
    /// Returns an error when the action cannot be executed.
    fn run(&mut self, action: &Action) -> Result<(), Self::Error>;
}

/// Executes every action in a plan in its declared order.
pub struct Executor<R> {
    runner: R,
}

impl<R> Executor<R> {
    /// Creates an executor using the supplied action runner.
    #[must_use]
    pub const fn new(runner: R) -> Self {
        Self { runner }
    }

    /// Returns a shared reference to the underlying action runner.
    #[must_use]
    pub const fn runner(&self) -> &R {
        &self.runner
    }

    /// Consumes the executor and returns its action runner.
    #[must_use]
    pub fn into_runner(self) -> R {
        self.runner
    }
}

impl<R> Executor<R>
where
    R: ActionRunner,
{
    /// Executes every step in the supplied plan.
    ///
    /// Execution stops immediately when an action runner returns an error.
    ///
    /// # Errors
    ///
    /// Returns the first error produced by the action runner.
    pub fn execute(&mut self, plan: &Plan) -> Result<(), R::Error> {
        for step in &plan.steps {
            self.runner.run(&step.action)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use model::{Action, Capability, Plan, PlanStep, ProviderId};

    use super::{ActionRunner, Executor};

    #[derive(Default)]
    struct RecordingRunner {
        actions: Vec<Action>,
    }

    impl ActionRunner for RecordingRunner {
        type Error = ();

        fn run(&mut self, action: &Action) -> Result<(), Self::Error> {
            self.actions.push(action.clone());

            Ok(())
        }
    }

    struct FailingRunner {
        actions: Vec<Action>,
        fail_on_call: usize,
    }

    impl ActionRunner for FailingRunner {
        type Error = &'static str;

        fn run(&mut self, action: &Action) -> Result<(), Self::Error> {
            if self.actions.len() == self.fail_on_call {
                return Err("action failed");
            }

            self.actions.push(action.clone());

            Ok(())
        }
    }

    fn plan_with_steps(steps: Vec<PlanStep>) -> Plan {
        Plan {
            capability: Capability::new("desktop"),
            provider: ProviderId::new("desktop"),
            steps,
        }
    }

    #[test]
    fn executes_plan_actions_in_order() {
        let plan = plan_with_steps(vec![
            PlanStep::new(Action::InstallPackageManifest("desktop".to_owned())),
            PlanStep::new(Action::EnableService(
                "display-manager".to_owned(),
            )),
        ]);

        let mut executor = Executor::new(RecordingRunner::default());

        executor.execute(&plan).expect("plan should execute");

        assert_eq!(
            executor.runner().actions,
            vec![
                Action::InstallPackageManifest("desktop".to_owned()),
                Action::EnableService("display-manager".to_owned()),
            ]
        );
    }

    #[test]
    fn empty_plan_succeeds_without_executing_actions() {
        let plan = plan_with_steps(Vec::new());
        let mut executor = Executor::new(RecordingRunner::default());

        executor.execute(&plan).expect("empty plan should execute");

        assert!(executor.runner().actions.is_empty());
    }

    #[test]
    fn stops_execution_after_first_error() {
        let plan = plan_with_steps(vec![
            PlanStep::new(Action::InstallPackageManifest("desktop".to_owned())),
            PlanStep::new(Action::EnableService(
                "display-manager".to_owned(),
            )),
            PlanStep::new(Action::EnableService("sshd".to_owned())),
        ]);

        let runner = FailingRunner {
            actions: Vec::new(),
            fail_on_call: 1,
        };
        let mut executor = Executor::new(runner);

        let error = executor
            .execute(&plan)
            .expect_err("second action should fail");

        assert_eq!(error, "action failed");
        assert_eq!(
            executor.runner().actions,
            vec![Action::InstallPackageManifest("desktop".to_owned())]
        );
    }
}
