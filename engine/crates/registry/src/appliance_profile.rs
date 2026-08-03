use model::{ApplianceProfile, Capability};
use serde::Deserialize;

use crate::RegistryError;

/// Deserializes and validates one appliance profile YAML document.
#[allow(clippy::redundant_pub_crate)]
pub(crate) fn appliance_profile_from_yaml(yaml: &str) -> Result<ApplianceProfile, RegistryError> {
    let profile_dto = serde_yaml::from_str::<ApplianceProfileDto>(yaml)?;

    profile_dto.try_into_appliance_profile()
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ApplianceProfileDto {
    name: String,
    description: String,
    capabilities: Vec<String>,
}

impl ApplianceProfileDto {
    fn try_into_appliance_profile(self) -> Result<ApplianceProfile, RegistryError> {
        let name = validated_value(&self.name, "profile name")?;
        let description = validated_value(&self.description, "profile description")?;

        if self.capabilities.is_empty() {
            return Err(RegistryError::InvalidApplianceProfile(
                "profile must contain at least one capability".to_owned(),
            ));
        }

        let capabilities = self
            .capabilities
            .into_iter()
            .map(|capability| validated_value(&capability, "capability name").map(Capability::new))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(ApplianceProfile::new(name, description, capabilities))
    }
}

fn validated_value(value: &str, field_name: &str) -> Result<String, RegistryError> {
    let value = value.trim();

    if value.is_empty() {
        return Err(RegistryError::InvalidApplianceProfile(format!(
            "{field_name} must not be empty"
        )));
    }

    Ok(value.to_owned())
}
#[cfg(test)]
mod tests {
    use crate::RegistryError;

    use super::appliance_profile_from_yaml;

    #[test]
    fn parses_appliance_profile_yaml() {
        let profile = appliance_profile_from_yaml(
            r"
name: desktop
description: Desktop appliance
capabilities:
  - desktop
  - remote-access
",
        )
        .expect("appliance profile should parse");

        assert_eq!(profile.name(), "desktop");
        assert_eq!(profile.description(), "Desktop appliance");
        assert_eq!(
            profile.capabilities(),
            &[
                model::Capability::new("desktop"),
                model::Capability::new("remote-access"),
            ]
        );
    }

    #[test]
    fn trims_profile_fields() {
        let profile = appliance_profile_from_yaml(
            r#"
name: " desktop "
description: " Desktop appliance "
capabilities:
  - " desktop "
"#,
        )
        .expect("profile should parse");

        assert_eq!(profile.name(), "desktop");
        assert_eq!(profile.description(), "Desktop appliance");
        assert_eq!(profile.capabilities(), &[model::Capability::new("desktop")]);
    }

    #[test]
    fn rejects_empty_profile_name() {
        let error = appliance_profile_from_yaml(
            r#"
name: " "
description: Desktop appliance
capabilities:
  - desktop
"#,
        )
        .expect_err("empty profile name should fail");

        assert!(matches!(
            error,
            RegistryError::InvalidApplianceProfile(message)
                if message == "profile name must not be empty"
        ));
    }

    #[test]
    fn rejects_empty_profile_description() {
        let error = appliance_profile_from_yaml(
            r#"
name: desktop
description: " "
capabilities:
  - desktop
"#,
        )
        .expect_err("empty profile description should fail");

        assert!(matches!(
            error,
            RegistryError::InvalidApplianceProfile(message)
                if message == "profile description must not be empty"
        ));
    }

    #[test]
    fn rejects_empty_capability_collection() {
        let error = appliance_profile_from_yaml(
            r"
name: desktop
description: Desktop appliance
capabilities: []
",
        )
        .expect_err("empty capability collection should fail");

        assert!(matches!(
            error,
            RegistryError::InvalidApplianceProfile(message)
                if message == "profile must contain at least one capability"
        ));
    }

    #[test]
    fn rejects_empty_capability_name() {
        let error = appliance_profile_from_yaml(
            r#"
name: desktop
description: Desktop appliance
capabilities:
  - " "
"#,
        )
        .expect_err("empty capability name should fail");

        assert!(matches!(
            error,
            RegistryError::InvalidApplianceProfile(message)
                if message == "capability name must not be empty"
        ));
    }

    #[test]
    fn rejects_unknown_fields() {
        let error = appliance_profile_from_yaml(
            r"
name: desktop
description: Desktop appliance
capabilities:
  - desktop
unknown: value
",
        )
        .expect_err("unknown field should fail");

        assert!(matches!(error, RegistryError::Parse(_)));
    }
}
