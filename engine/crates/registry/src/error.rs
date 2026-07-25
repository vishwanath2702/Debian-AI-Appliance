use std::{
    error::Error,
    fmt::{self, Display, Formatter},
    io,
};

/// Error returned while loading provider registry data.
#[derive(Debug)]
pub enum RegistryError {
    /// A provider YAML file could not be read.
    Io(io::Error),

    /// The provider document could not be parsed as YAML.
    Parse(serde_yaml::Error),

    /// The provider document parsed successfully but contained invalid data.
    InvalidProvider(String),
}

impl Display for RegistryError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => {
                write!(formatter, "failed to read provider YAML: {error}")
            }
            Self::Parse(error) => {
                write!(formatter, "failed to parse provider YAML: {error}")
            }
            Self::InvalidProvider(message) => {
                write!(formatter, "invalid provider: {message}")
            }
        }
    }
}

impl Error for RegistryError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Parse(error) => Some(error),
            Self::InvalidProvider(_) => None,
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

    use super::RegistryError;

    #[test]
    fn formats_io_error() {
        let error = RegistryError::from(io::Error::new(
            io::ErrorKind::NotFound,
            "provider file not found",
        ));

        assert_eq!(
            error.to_string(),
            concat!("failed to read provider YAML: ", "provider file not found")
        );
    }

    #[test]
    fn formats_parse_error() {
        let yaml_error = serde_yaml::from_str::<String>("[").expect_err("YAML should be malformed");

        let error = RegistryError::from(yaml_error);

        assert!(
            error
                .to_string()
                .starts_with("failed to parse provider YAML:")
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
}
