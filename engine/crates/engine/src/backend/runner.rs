//! Runner-backed build backend.

use executor::{ActionRunner, ExecuteError, ExecutionEnvironment, Executor};
use model::Plan;

use super::BuildBackend;

/// Adapts an execution environment into a build backend.
pub struct RunnerBackend<R> {
    executor: Executor<R>,
}

impl<R> RunnerBackend<R> {
    /// Creates a backend from an execution environment.
    #[must_use]
    pub const fn new(runner: R) -> Self {
        Self {
            executor: Executor::new(runner),
        }
    }
}

impl<R> BuildBackend for RunnerBackend<R>
where
    R: ActionRunner + ExecutionEnvironment,
{
    type Error = R::Error;

    fn build(&mut self, plan: &Plan) -> Result<(), ExecuteError<Self::Error>> {
        self.executor.execute(plan)
    }
}
