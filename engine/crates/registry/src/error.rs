use std::{
    error::Error,
    fmt::{self, Display, Formatter},
    io,
};

use model::{CapabilityId, ProviderId};

/// Error returned while loading registry data.
#[derive(Debug)]
pub enum RegistryError {
    /// A registry YAML file or directory could not be read.
    Io(io::Error),

    /// A registry document could not be parsed as YAML.
    Parse(serde_yaml::Error),

    /// A provider document parsed successfully but contained invalid data.
    InvalidProvider(String),

    /// A package-manifest document parsed successfully but contained invalid data.
    InvalidPackageManifest(String),

    /// More than one provider used the same provider identifier.
    DuplicateProviderId(ProviderId),

    /// More than one provider supplied the same capability.
    DuplicateCapability(CapabilityId),

    /// More than one package manifest used the same name.
    DuplicatePackageManifest(String),
}

impl Display for RegistryError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => {
                write!(formatter, "failed to read registry data: {error}")
            }
            Self::Parse(error) => {
                write!(formatter, "failed to parse registry YAML: {error}")
            }
            Self::InvalidProvider(message) => {
                write!(formatter, "invalid provider: {message}")
            }
            Self::InvalidPackageManifest(message) => {
                write!(formatter, "invalid package manifest: {message}")
            }
            Self::DuplicateProviderId(provider_id) => {
                write!(formatter, "duplicate provider id: {provider_id}")
            }
            Self::DuplicateCapability(capability_id) => {
                write!(formatter, "duplicate capability: {capability_id}")
            }
            Self::DuplicatePackageManifest(name) => {
                write!(formatter, "duplicate package manifest: {name}")
            }
        }
    }
}

impl Error for RegistryError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Parse(error) => Some(error),
            Self::InvalidProvider(_)
            | Self::InvalidPackageManifest(_)
            | Self::DuplicateProviderId(_)
            | Self::DuplicateCapability(_)
            | Self::DuplicatePackageManifest(_) => None,
        }
    }
}

impl From<io::Error> for RegistryError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<serde_yaml::Error> for RegistryError {
    fn from(error: serde_yaml::Error) -> Self {
        Self::Parse(error)
    }
}

#[cfg(test)]
mod tests {
    use std::io;

    use model::{CapabilityId, ProviderId};

    use super::RegistryError;

    #[test]
    fn formats_io_error() {
        let error = RegistryError::from(io::Error::new(
            io::ErrorKind::NotFound,
            "provider file not found",
        ));

        assert_eq!(
            error.to_string(),
            concat!("failed to read registry data: ", "provider file not found")
        );
    }

    #[test]
    fn formats_parse_error() {
        let yaml_error = serde_yaml::from_str::<String>("[").expect_err("YAML should be malformed");

        let error = RegistryError::from(yaml_error);

        assert!(
            error
                .to_string()
                .starts_with("failed to parse registry YAML:")
        );
    }

    #[test]
    fn formats_invalid_provider_error() {
        let error = RegistryError::InvalidProvider("provider id must not be empty".to_owned());

        assert_eq!(
            error.to_string(),
            "invalid provider: provider id must not be empty"
        );
    }
    #[test]
    fn formats_invalid_package_manifest_error() {
        let error =
            RegistryError::InvalidPackageManifest("manifest name must not be empty".to_owned());

        assert_eq!(
            error.to_string(),
            "invalid package manifest: manifest name must not be empty"
        );
    }
    #[test]
    fn formats_duplicate_provider_id_error() {
        let error = RegistryError::DuplicateProviderId(ProviderId::new("desktop"));

        assert_eq!(error.to_string(), "duplicate provider id: desktop");
    }

    #[test]
    fn formats_duplicate_capability_error() {
        let error = RegistryError::DuplicateCapability(CapabilityId::new("desktop"));

        assert_eq!(error.to_string(), "duplicate capability: desktop");
    }
    #[test]
    fn formats_duplicate_package_manifest_error() {
        let error = RegistryError::DuplicatePackageManifest("desktop".to_owned());

        assert_eq!(error.to_string(), "duplicate package manifest: desktop");
    }
}
