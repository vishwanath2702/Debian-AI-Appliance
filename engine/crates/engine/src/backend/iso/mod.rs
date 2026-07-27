//! ISO build backend.

mod context;
mod layout;
mod pipeline;
mod stage;

pub use context::{IsoConfig, IsoContext};
pub use layout::Layout;
pub use pipeline::IsoPipeline;
pub use stage::{InitramfsStage, KernelStage, WorkspaceStage};

use executor::ExecuteError;
use model::Plan;
use std::path::{Path, PathBuf};

use super::BuildBackend;
/// Backend responsible for producing a bootable ISO image.
pub struct IsoBackend {
    rootfs: PathBuf,
    work_directory: PathBuf,
    output_path: PathBuf,
}

impl IsoBackend {
    /// Creates an ISO backend.
    #[must_use]
    pub fn new(
        rootfs: impl Into<PathBuf>,
        work_directory: impl Into<PathBuf>,
        output_path: impl Into<PathBuf>,
    ) -> Self {
        Self {
            rootfs: rootfs.into(),
            work_directory: work_directory.into(),
            output_path: output_path.into(),
        }
    }

    /// Returns the prepared root filesystem path.
    #[must_use]
    pub fn rootfs(&self) -> &Path {
        &self.rootfs
    }

    /// Returns the ISO working-directory path.
    #[must_use]
    pub fn work_directory(&self) -> &Path {
        &self.work_directory
    }

    /// Returns the final ISO output path.
    #[must_use]
    pub fn output_path(&self) -> &Path {
        &self.output_path
    }

    #[must_use]
    pub fn layout(&self) -> Layout {
        Layout::new(self.work_directory.join("iso"))
    }
}

impl BuildBackend for IsoBackend {
    type Error = std::io::Error;

    fn build(&mut self, _plan: &Plan) -> Result<(), ExecuteError<Self::Error>> {
        let context = IsoContext {
            config: IsoConfig {
                rootfs: self.rootfs.clone(),
                layout: self.layout(),
            },
        };
        IsoPipeline::run(&context).map_err(ExecuteError::Environment)?;

        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use model::{Capability, Plan, ProviderId};
    use tempfile::tempdir;

    #[test]
    fn build_creates_workspace_layout() {
        let temp = tempdir().unwrap();

        let mut backend = IsoBackend::new(
            temp.path().join("rootfs"),
            temp.path().join("work"),
            temp.path().join("image.iso"),
        );

        let plan = Plan {
            capability: Capability::new("test"),
            provider: ProviderId::new("test"),
            steps: Vec::new(),
        };

        let boot_directory = backend.rootfs().join("boot");

        std::fs::create_dir_all(&boot_directory).unwrap();
        std::fs::File::create(boot_directory.join("vmlinuz-test")).unwrap();
        std::fs::File::create(boot_directory.join("initrd.img-test")).unwrap();
        backend.build(&plan).unwrap();

        let layout = backend.layout();

        assert!(layout.boot_grub().is_dir());
        assert!(layout.efi_boot().is_dir());
        assert!(layout.live().is_dir());
        assert!(layout.staging().is_dir());
    }
}
