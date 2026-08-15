use model::{
    DiscoveredStorage,
    DiscoveredStorageId,
    InstallationIntent,
    Plan,
};
/// A non-executed operation required to prepare an installation target.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InstallationOperation {
    /// Prepare the selected physical disk for installation.
    PrepareDisk {
        /// Stable identifier of the selected physical storage.
        storage_id: DiscoveredStorageId,
    },

    /// Create the filesystems required by the installed system.
    CreateFilesystems { filesystem: String },

    /// Mount the prepared target filesystems.
    MountFilesystems { mount_point: String },

    /// Bootstrap the base operating system.
    BootstrapSystem { root: String },

    /// Apply the appliance execution plans.
    ApplyPlans { count: usize },
}
/// Executes one planned installation operation.
pub trait InstallationOperationExecutor {
    /// Error produced while executing an operation.
    type Error;

    /// Executes one installation operation.
    ///
    /// # Errors
    ///
    /// Returns an executor-specific error if the operation fails.
    fn execute_operation(
        &mut self,
        operation: &InstallationOperation,
    ) -> Result<(), Self::Error>;
}
/// Ordered non-executed operations for installing an appliance.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstallationPlan {
    operations: Vec<InstallationOperation>,
}

impl InstallationPlan {
    /// Creates an installation plan.
    #[must_use]
    pub const fn new(operations: Vec<InstallationOperation>) -> Self {
        Self { operations }
    }

    /// Returns the ordered installation operations.
    #[must_use]
    pub fn operations(&self) -> &[InstallationOperation] {
        &self.operations
    }

    /// Executes the planned operations in order.
    ///
    /// # Errors
    ///
    /// Returns the first error produced by the operation executor.
    pub fn execute<E>(&self, executor: &mut E) -> Result<(), E::Error>
    where
        E: InstallationOperationExecutor,
    {
        for operation in &self.operations {
            executor.execute_operation(operation)?;
        }

        Ok(())
    }
}
/// A validated installation ready for later execution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedInstallation {
    intent: InstallationIntent,
    storage: DiscoveredStorage,
    plans: Vec<Plan>,
}

impl PreparedInstallation {
    /// Creates a prepared installation from validated components.
    #[must_use]
    pub const fn new(
        intent: InstallationIntent,
        storage: DiscoveredStorage,
        plans: Vec<Plan>,
    ) -> Self {
        Self {
            intent,
            storage,
            plans,
        }
    }

    /// Builds the ordered installation-operation plan.
    #[must_use]
    pub fn installation_plan(&self) -> InstallationPlan {
        InstallationPlan::new(vec![
            InstallationOperation::PrepareDisk {
                storage_id: self.intent.storage_id().clone(),
            },
            InstallationOperation::CreateFilesystems {
                filesystem: "ext4".to_owned(),
            },
            InstallationOperation::MountFilesystems {
                mount_point: "/target".to_owned(),
            },
            InstallationOperation::BootstrapSystem {
                root: "/target".to_owned(),
            },
            InstallationOperation::ApplyPlans {
                count: self.plans.len(),
            },
        ])
    }

    /// Returns the confirmed installation intent.
    #[must_use]
    pub const fn intent(&self) -> &InstallationIntent {
        &self.intent
    }

    /// Returns the validated installation storage.
    #[must_use]
    pub const fn storage(&self) -> &DiscoveredStorage {
        &self.storage
    }

    /// Returns the execution plans for the installation.
    #[must_use]
    pub fn plans(&self) -> &[Plan] {
        &self.plans
    }

    /// Returns a human-readable dry-run summary.
    #[must_use]
    pub fn summary(&self) -> String {
        format!(
            "Profile: {}\nStorage: {} ({})\nDevice: {}\nPlans: {}",
            self.intent.profile_name(),
            self.intent.storage_id(),
            self.storage.kind(),
            self.storage.device_path().display(),
            self.plans.len()
        )
    }
}
/// Executes a prepared installation.
pub trait InstallationExecutor {
    /// Error produced by the executor.
    type Error;

    /// Executes the prepared installation.
    ///
    /// # Errors
    ///
    /// Returns an executor-specific error if execution fails.
    fn execute(
        &mut self,
        installation: &PreparedInstallation,
    ) -> Result<(), Self::Error>;
}
/// Non-destructive installation executor used for validation and previews.
#[derive(Clone, Debug, Default)]
pub struct DryRunInstallationExecutor {
    summary: Option<String>,
    plan: Option<InstallationPlan>,
    executed_operations: Vec<InstallationOperation>,
}

impl DryRunInstallationExecutor {
    /// Returns the summary recorded during the most recent execution.
    #[must_use]
    pub fn summary(&self) -> Option<&str> {
        self.summary.as_deref()
    }

    /// Returns the installation plan recorded during the most recent execution.
    #[must_use]
    pub const fn plan(&self) -> Option<&InstallationPlan> {
        self.plan.as_ref()
    }

    /// Returns operations recorded through the execution pipeline.
    #[must_use]
    pub fn executed_operations(&self) -> &[InstallationOperation] {
        &self.executed_operations
    }
}

impl InstallationExecutor for DryRunInstallationExecutor {
    type Error = std::convert::Infallible;

    fn execute(
        &mut self,
        installation: &PreparedInstallation,
    ) -> Result<(), Self::Error> {
        self.summary = Some(installation.summary());

        let plan = installation.installation_plan();
        plan.execute(self)?;

        self.plan = Some(plan);

        Ok(())
    }
}

impl InstallationOperationExecutor for DryRunInstallationExecutor {
    type Error = std::convert::Infallible;

    fn execute_operation(
        &mut self,
        operation: &InstallationOperation,
    ) -> Result<(), Self::Error> {
        self.executed_operations.push(operation.clone());
        Ok(())
    }
}
