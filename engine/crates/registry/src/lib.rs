//! Provider, package-manifest, and appliance-profile registries for the DAIA engine.

mod appliance_profile;
mod appliance_profile_repository;
mod dto;
mod error;
mod package_manifest;
mod package_repository;
mod registry;

pub use appliance_profile_repository::ApplianceProfileRepository;
pub use error::RegistryError;
pub use package_repository::PackageRepository;
pub use registry::Registry;
