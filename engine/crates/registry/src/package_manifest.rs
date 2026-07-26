use model::PackageManifest;
use serde::Deserialize;

use crate::RegistryError;

/// Deserializes and validates one package-manifest YAML document.
#[allow(clippy::redundant_pub_crate)]
pub(crate) fn package_manifest_from_yaml(yaml: &str) -> Result<PackageManifest, RegistryError> {
    let manifest_dto = serde_yaml::from_str::<PackageManifestDto>(yaml)?;

    manifest_dto.try_into_package_manifest()
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PackageManifestDto {
    name: String,
    packages: Vec<String>,
}

impl PackageManifestDto {
    fn try_into_package_manifest(self) -> Result<PackageManifest, RegistryError> {
        let name = validated_value(&self.name, "manifest name")?;

        if self.packages.is_empty() {
            return Err(RegistryError::InvalidPackageManifest(
                "manifest must contain at least one package".to_owned(),
            ));
        }

        let packages = self
            .packages
            .into_iter()
            .map(|package| validated_value(&package, "package name"))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(PackageManifest::new(name, packages))
    }
}

fn validated_value(value: &str, field_name: &str) -> Result<String, RegistryError> {
    let value = value.trim();

    if value.is_empty() {
        return Err(RegistryError::InvalidPackageManifest(format!(
            "{field_name} must not be empty"
        )));
    }

    Ok(value.to_owned())
}

#[cfg(test)]
mod tests {
    use crate::RegistryError;

    use super::package_manifest_from_yaml;

    #[test]
    fn parses_package_manifest_yaml() {
        let manifest = package_manifest_from_yaml(
            r"
name: desktop
packages:
  - gnome-shell
  - gdm3
",
        )
        .expect("package manifest should parse");

        assert_eq!(manifest.name(), "desktop");
        assert_eq!(
            manifest.packages(),
            &["gnome-shell".to_owned(), "gdm3".to_owned()]
        );
    }

    #[test]
    fn trims_manifest_and_package_names() {
        let manifest = package_manifest_from_yaml(
            r#"
name: " desktop "
packages:
  - " gnome-shell "
"#,
        )
        .expect("package manifest should parse");

        assert_eq!(manifest.name(), "desktop");
        assert_eq!(manifest.packages(), &["gnome-shell".to_owned()]);
    }

    #[test]
    fn rejects_empty_manifest_name() {
        let error = package_manifest_from_yaml(
            r#"
name: " "
packages:
  - gnome-shell
"#,
        )
        .expect_err("empty manifest name should fail");

        assert!(matches!(
            error,
            RegistryError::InvalidPackageManifest(message)
                if message == "manifest name must not be empty"
        ));
    }

    #[test]
    fn rejects_empty_package_collection() {
        let error = package_manifest_from_yaml(
            r"
name: desktop
packages: []
",
        )
        .expect_err("empty package collection should fail");

        assert!(matches!(
            error,
            RegistryError::InvalidPackageManifest(message)
                if message == "manifest must contain at least one package"
        ));
    }

    #[test]
    fn rejects_empty_package_name() {
        let error = package_manifest_from_yaml(
            r#"
name: desktop
packages:
  - " "
"#,
        )
        .expect_err("empty package name should fail");

        assert!(matches!(
            error,
            RegistryError::InvalidPackageManifest(message)
                if message == "package name must not be empty"
        ));
    }

    #[test]
    fn rejects_unknown_fields() {
        let error = package_manifest_from_yaml(
            r"
name: desktop
packages:
  - gnome-shell
description: desktop packages
",
        )
        .expect_err("unknown field should fail");

        assert!(matches!(error, RegistryError::Parse(_)));
    }
}
