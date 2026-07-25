//! Execution of planned system actions.

use model::{Action, Plan};

/// Error returned when plan execution fails.
#[derive(Debug)]
pub struct ExecuteError<E> {
    /// Zero-based index of the failed step.
    pub step: usize,

    /// Action that failed.
    pub action: Action,

    /// Error returned by the action runner.
    pub source: E,
}

/// Executes one action from a plan.
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

#[derive(Debug, Default)]
pub struct RootfsRunner;

impl ActionRunner for RootfsRunner {
    type Error = std::convert::Infallible;

    fn run(&mut self, action: &Action) -> Result<(), Self::Error> {
        println!("executing action: {action:?}");
        Ok(())
    }
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

    /// Returns a shared reference to the underlying runner.
    #[must_use]
    pub const fn runner(&self) -> &R {
        &self.runner
    }
}

impl<R> Executor<R>
where
    R: ActionRunner,
{
    /// Executes every step in the supplied plan.
    ///
    /// Execution stops on the first error.
    ///
    /// # Errors
    ///
    /// Returns the first error produced by the runner.
    pub fn execute(&mut self, plan: &Plan) -> Result<(), ExecuteError<R::Error>> {
        for (step, plan_step) in plan.steps.iter().enumerate() {
            self.runner
                .run(&plan_step.action)
                .map_err(|source| ExecuteError {
                    step,
                    action: plan_step.action.clone(),
                    source,
                })?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use model::{Action, Capability, Plan, PlanStep, ProviderId};

    use super::{ActionRunner, Executor, RootfsRunner};
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

    struct FailingRunner;

    impl ActionRunner for FailingRunner {
        type Error = &'static str;

        fn run(&mut self, _: &Action) -> Result<(), Self::Error> {
            Err("boom")
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
            PlanStep::new(Action::EnableService("display-manager".to_owned())),
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
    fn empty_plan_succeeds() {
        let plan = plan_with_steps(Vec::new());
        let mut executor = Executor::new(RecordingRunner::default());

        executor.execute(&plan).expect("empty plan should execute");

        assert!(executor.runner().actions.is_empty());
    }

    #[test]
    fn returns_execution_context_when_runner_fails() {
        let plan = plan_with_steps(vec![PlanStep::new(Action::InstallPackageManifest(
            "desktop".to_owned(),
        ))]);

        let mut executor = Executor::new(FailingRunner);

        let error = executor.execute(&plan).expect_err("execution should fail");

        assert_eq!(error.step, 0);
        assert_eq!(
            error.action,
            Action::InstallPackageManifest("desktop".to_owned())
        );
        assert_eq!(error.source, "boom");
    }

    #[test]
    fn rootfs_runner_accepts_actions() {
        let mut runner = RootfsRunner::default();

        runner
            .run(&Action::InstallPackageManifest("desktop".to_owned()))
            .expect("rootfs runner should succeed");
    }
}
