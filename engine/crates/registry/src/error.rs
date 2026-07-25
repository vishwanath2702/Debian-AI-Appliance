use std::{
    error::Error,
    fmt::{self, Display, Formatter},
};

/// Error returned while loading provider registry data.
#[derive(Debug)]
pub enum RegistryError {
    /// The provider document could not be parsed as YAML.
    Parse(serde_yaml::Error),

    /// The provider document parsed successfully but contained invalid data.
    InvalidProvider(String),
}

impl Display for RegistryError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
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
            Self::Parse(error) => Some(error),
            Self::InvalidProvider(_) => None,
        }
    }
}

impl From<serde_yaml::Error> for RegistryError {
    fn from(error: serde_yaml::Error) -> Self {
        Self::Parse(error)
    }
}

#[cfg(test)]
mod tests {
    use super::RegistryError;

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
