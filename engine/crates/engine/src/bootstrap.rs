//! Debian root filesystem bootstrap configuration.
use inspector::IsoMetadata;
/// Configuration used to bootstrap a Debian root filesystem.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BootstrapConfig {
    release: String,
    architecture: String,
    mirror: String,
    components: Vec<String>,
    variant: String,
}

impl BootstrapConfig {
    /// Creates bootstrap configuration from inspected ISO metadata.
    #[must_use]
    pub fn from_iso_metadata(metadata: &IsoMetadata) -> Self {
        let components = metadata
            .repositories()
            .first()
            .map(|repository| repository.components().to_vec())
            .unwrap_or_else(|| vec!["main".to_owned()]);

        Self::new(
            metadata.codename().to_ascii_lowercase(),
            metadata.architecture().to_owned(),
            "https://deb.debian.org/debian",
            components,
            "minbase",
        )
    }
    /// Creates a bootstrap configuration.
    #[must_use]
    pub fn new(
        release: impl Into<String>,
        architecture: impl Into<String>,
        mirror: impl Into<String>,
        components: Vec<String>,
        variant: impl Into<String>,
    ) -> Self {
        Self {
            release: release.into(),
            architecture: architecture.into(),
            mirror: mirror.into(),
            components,
            variant: variant.into(),
        }
    }

    /// Returns the Debian release or suite.
    #[must_use]
    pub fn release(&self) -> &str {
        &self.release
    }

    /// Returns the target Debian architecture.
    #[must_use]
    pub fn architecture(&self) -> &str {
        &self.architecture
    }

    /// Returns the Debian archive mirror.
    #[must_use]
    pub fn mirror(&self) -> &str {
        &self.mirror
    }

    /// Returns the enabled Debian archive components.
    #[must_use]
    pub fn components(&self) -> &[String] {
        &self.components
    }

    /// Returns the bootstrap variant.
    #[must_use]
    pub fn variant(&self) -> &str {
        &self.variant
    }
}
impl Default for BootstrapConfig {
    fn default() -> Self {
        Self::new(
            "bookworm",
            "amd64",
            "https://deb.debian.org/debian",
            vec!["main".to_owned()],
            "minbase",
        )
    }
}

#[cfg(test)]
mod tests {
    use super::BootstrapConfig;

    #[test]
    fn exposes_bootstrap_configuration() {
        let config = BootstrapConfig::new(
            "bookworm",
            "amd64",
            "https://deb.debian.org/debian",
            vec!["main".to_owned(), "non-free-firmware".to_owned()],
            "minbase",
        );

        assert_eq!(config.release(), "bookworm");
        assert_eq!(config.architecture(), "amd64");
        assert_eq!(config.mirror(), "https://deb.debian.org/debian");
        assert_eq!(
            config.components(),
            &["main".to_owned(), "non-free-firmware".to_owned()]
        );
        assert_eq!(config.variant(), "minbase");
    }
    #[test]
    fn default_bootstrap_configuration_is_debian_bookworm() {
        let config = BootstrapConfig::default();

        assert_eq!(config.release(), "bookworm");
        assert_eq!(config.architecture(), "amd64");
        assert_eq!(config.mirror(), "https://deb.debian.org/debian");
        assert_eq!(config.components(), &["main".to_owned()]);
        assert_eq!(config.variant(), "minbase");
    }
}
