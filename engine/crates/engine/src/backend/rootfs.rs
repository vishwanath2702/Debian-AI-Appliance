//! Root filesystem build backend.

use std::path::PathBuf;

use crate::BuildBackend;
use assets::FilesystemAssetStore;
use executor::{AptInstaller, ExecuteError, RootfsRunError, RootfsRunner};
use model::Plan;
use registry::PackageRepository;

/// Backend that builds an appliance inside a root filesystem.
pub struct RootfsBackend {
    rootfs: PathBuf,
    asset_directory: PathBuf,
    package_repository: PackageRepository,
}

impl RootfsBackend {
    /// Creates a root filesystem backend.
    #[must_use]
    pub const fn new(
        rootfs: PathBuf,
        asset_directory: PathBuf,
        package_repository: PackageRepository,
    ) -> Self {
        Self {
            rootfs,
            asset_directory,
            package_repository,
        }
    }
}

impl BuildBackend for RootfsBackend {
    type Error = RootfsRunError;

    fn build(&mut self, plan: &Plan) -> Result<(), ExecuteError<Self::Error>> {
        let runner = RootfsRunner::with_dependencies(
            self.rootfs.clone(),
            self.package_repository.clone(),
            Box::new(AptInstaller::new()),
            Box::new(FilesystemAssetStore::new(self.asset_directory.clone())),
        );

        let mut backend = crate::RunnerBackend::new(runner);

        backend.build(plan)
    }
}
