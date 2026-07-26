//! Build backends.

use executor::{ActionRunner, ExecuteError, ExecutionEnvironment, Executor};
use model::Plan;

/// Backend responsible for producing a build artifact.
pub trait BuildBackend {
    /// Backend-specific error.
    type Error;

    /// Builds the requested artifact from a plan.
    ///
    /// # Errors
    ///
    /// Returns an error if the backend cannot complete the build.
    fn build(&mut self, plan: &Plan) -> Result<(), ExecuteError<Self::Error>>;
}

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
