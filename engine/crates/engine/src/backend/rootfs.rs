//! Root filesystem build backend.

use std::path::PathBuf;

use crate::BuildBackend;
use executor::{AptInstaller, ExecuteError, RootfsRunError, RootfsRunner};
use model::Plan;
use registry::PackageRepository;

/// Backend that builds an appliance inside a root filesystem.
pub struct RootfsBackend {
    rootfs: PathBuf,
    package_repository: PackageRepository,
}

impl RootfsBackend {
    /// Creates a root filesystem backend.
    #[must_use]
    pub const fn new(rootfs: PathBuf, package_repository: PackageRepository) -> Self {
        Self {
            rootfs,
            package_repository,
        }
    }
}

impl BuildBackend for RootfsBackend {
    type Error = RootfsRunError;

    fn build(&mut self, plan: &Plan) -> Result<(), ExecuteError<Self::Error>> {
        let runner = RootfsRunner::with_installer(
            self.rootfs.clone(),
            self.package_repository.clone(),
            Box::new(AptInstaller::new()),
        );

        let mut backend = crate::RunnerBackend::new(runner);

        backend.build(plan)
    }
}
