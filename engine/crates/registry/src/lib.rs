//! Provider and package-manifest registries for the DAIA engine.

mod dto;
mod error;
mod package_manifest;
mod package_repository;
mod registry;

pub use error::RegistryError;
pub use package_repository::PackageRepository;
pub use registry::Registry;
