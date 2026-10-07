//! Shared localization selections for DAIA installation.

/// User-selected localization settings for the installed appliance.
///
/// Values are identifiers rather than presentation labels. Availability and
/// platform-specific validation are handled separately.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalizationConfiguration {
    language: String,
    country: String,
    locale: String,
    keyboard_layout: String,
}

impl LocalizationConfiguration {
    /// Creates localization selections without assuming a platform or region.
    #[must_use]
    pub fn new(
        language: impl Into<String>,
        country: impl Into<String>,
        locale: impl Into<String>,
        keyboard_layout: impl Into<String>,
    ) -> Self {
        Self {
            language: language.into(),
            country: country.into(),
            locale: locale.into(),
            keyboard_layout: keyboard_layout.into(),
        }
    }

    #[must_use]
    pub fn language(&self) -> &str {
        &self.language
    }

    #[must_use]
    pub fn country(&self) -> &str {
        &self.country
    }

    #[must_use]
    pub fn locale(&self) -> &str {
        &self.locale
    }

    #[must_use]
    pub fn keyboard_layout(&self) -> &str {
        &self.keyboard_layout
    }
}

#[cfg(test)]
mod tests {
    use super::LocalizationConfiguration;

    #[test]
    fn preserves_independent_localization_selections() {
        let config = LocalizationConfiguration::new("fr", "CA", "fr_CA.UTF-8", "ca");

        assert_eq!(config.language(), "fr");
        assert_eq!(config.country(), "CA");
        assert_eq!(config.locale(), "fr_CA.UTF-8");
        assert_eq!(config.keyboard_layout(), "ca");
    }

    #[test]
    fn accepts_other_language_and_keyboard_combinations() {
        let config = LocalizationConfiguration::new("ja", "JP", "ja_JP.UTF-8", "jp");

        assert_eq!(config.language(), "ja");
        assert_eq!(config.keyboard_layout(), "jp");
    }
}
