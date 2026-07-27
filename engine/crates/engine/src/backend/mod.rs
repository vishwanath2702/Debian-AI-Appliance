//! Build backends.
mod iso;
mod rootfs;
mod runner;

use executor::ExecuteError;
use model::Plan;

pub use rootfs::RootfsBackend;
pub use runner::RunnerBackend;

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
