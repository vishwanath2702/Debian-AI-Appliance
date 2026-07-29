//! Shared workflow execution context.

use model::Capability;
use registry::PackageRepository;

use crate::{BuildContext, Engine};

/// Borrowed inputs shared by appliance build workflows.
pub struct WorkflowContext<'a> {
    engine: &'a Engine,
    capability: &'a Capability,
    build_context: &'a BuildContext,
    package_repository: &'a PackageRepository,
}

impl<'a> WorkflowContext<'a> {
    /// Creates a workflow context.
    #[must_use]
    pub const fn new(
        engine: &'a Engine,
        capability: &'a Capability,
        build_context: &'a BuildContext,
        package_repository: &'a PackageRepository,
    ) -> Self {
        Self {
            engine,
            capability,
            build_context,
            package_repository,
        }
    }

    /// Returns the orchestration engine.
    #[must_use]
    pub const fn engine(&self) -> &Engine {
        self.engine
    }

    /// Returns the requested capability.
    #[must_use]
    pub const fn capability(&self) -> &Capability {
        self.capability
    }

    /// Returns the shared build configuration.
    #[must_use]
    pub const fn build_context(&self) -> &BuildContext {
        self.build_context
    }

    /// Returns the package repository.
    #[must_use]
    pub const fn package_repository(&self) -> &PackageRepository {
        self.package_repository
    }
}
