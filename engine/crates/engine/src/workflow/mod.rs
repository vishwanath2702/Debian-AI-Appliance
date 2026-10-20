//! Appliance build workflows.

mod iso;
mod rootfs_verification;

pub use iso::IsoWorkflow;

/// Verifies a desktop rootfs without bootstrapping or ISO packaging.
///
/// # Errors
///
/// Returns an I/O error when required packages or firmware are missing.
pub fn verify_desktop_rootfs(
    rootfs: &std::path::Path,
    package_repository: &registry::PackageRepository,
) -> std::io::Result<()> {
    let manifest = package_repository.manifest("desktop").ok_or_else(|| {
        std::io::Error::other("rootfs verification failed: desktop package manifest missing")
    })?;

    let required: Vec<&str> = manifest.packages().iter().map(String::as_str).collect();

    rootfs_verification::verify_installed_packages(rootfs, &required)?;
    rootfs_verification::verify_intel_8265_firmware(rootfs)
}
