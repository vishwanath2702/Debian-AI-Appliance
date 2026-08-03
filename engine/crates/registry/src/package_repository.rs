use std::{collections::HashSet, fs, path::Path};

use model::PackageManifest;

use crate::{RegistryError, package_manifest::package_manifest_from_yaml};

/// Collection of package manifests known to the DAIA engine.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PackageRepository {
    manifests: Vec<PackageManifest>,
}

impl PackageRepository {
    /// Creates an empty package repository.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            manifests: Vec::new(),
        }
    }

    /// Creates a repository from an existing collection of package manifests.
    ///
    /// # Errors
    ///
    /// Returns [`RegistryError::DuplicatePackageManifest`] if two manifests use
    /// the same name.
    pub fn from_manifests(manifests: Vec<PackageManifest>) -> Result<Self, RegistryError> {
        let mut names = HashSet::new();

        for manifest in &manifests {
            if !names.insert(manifest.name().to_owned()) {
                return Err(RegistryError::DuplicatePackageManifest(
                    manifest.name().to_owned(),
                ));
            }
        }

        Ok(Self { manifests })
    }

    /// Creates a repository from one package-manifest YAML document.
    ///
    /// # Errors
    ///
    /// Returns a [`RegistryError`] if the YAML cannot be parsed or the package
    /// manifest contains invalid data.
    pub fn from_yaml_str(yaml: &str) -> Result<Self, RegistryError> {
        let manifest = package_manifest_from_yaml(yaml)?;

        Self::from_manifests(vec![manifest])
    }

    /// Creates a repository from one package-manifest YAML file.
    ///
    /// # Errors
    ///
    /// Returns a [`RegistryError`] if the file cannot be read, the YAML cannot
    /// be parsed, or the package manifest contains invalid data.
    pub fn from_yaml_file(path: impl AsRef<Path>) -> Result<Self, RegistryError> {
        let yaml = fs::read_to_string(path)?;

        Self::from_yaml_str(&yaml)
    }

    /// Creates a repository from all package-manifest YAML files in a directory.
    ///
    /// Files ending in `.yaml` or `.yml` are loaded. All other files are
    /// ignored.
    ///
    /// # Errors
    ///
    /// Returns a [`RegistryError`] if the directory cannot be read, a manifest
    /// file cannot be read, the YAML cannot be parsed, a manifest is invalid, or
    /// two manifests use the same name.
    pub fn from_directory(path: impl AsRef<Path>) -> Result<Self, RegistryError> {
        let mut manifests = Vec::new();

        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let path = entry.path();

            if !path.is_file() {
                continue;
            }

            let Some(extension) = path.extension() else {
                continue;
            };

            if extension != "yaml" && extension != "yml" {
                continue;
            }

            let repository = Self::from_yaml_file(&path)?;

            manifests.extend(repository.manifests);
        }

        Self::from_manifests(manifests)
    }

    /// Returns every package manifest in the repository.
    #[must_use]
    pub fn manifests(&self) -> &[PackageManifest] {
        &self.manifests
    }

    /// Finds a package manifest by name.
    #[must_use]
    pub fn manifest(&self, name: &str) -> Option<&PackageManifest> {
        self.manifests
            .iter()
            .find(|manifest| manifest.name() == name)
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, path::Path};

    use model::PackageManifest;

    use crate::RegistryError;

    use super::PackageRepository;

    const DESKTOP_MANIFEST_YAML: &str = r"
name: desktop
packages:
  - gnome-shell
  - gdm3
";

    const SSH_MANIFEST_YAML: &str = r"
name: ssh
packages:
  - openssh-server
";
    #[test]
    fn repository_package_manifest_directory_contains_desktop_manifest() {
        let manifest_directory =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../registry/package-manifests");

        let repository = PackageRepository::from_directory(manifest_directory)
            .expect("repository package-manifest directory should load");

        let manifest = repository
            .manifest("desktop")
            .expect("desktop package manifest should exist");

        assert_eq!(manifest.name(), "desktop");
assert_eq!(
    manifest.packages(),
    &[
        "live-boot".to_owned(),
        "live-config".to_owned(),
        "systemd-sysv".to_owned(),
        "linux-image-amd64".to_owned(),
        "initramfs-tools".to_owned(),
        "task-gnome-desktop".to_owned(),
        "gdm3".to_owned(),
    ]
);
    }
    #[test]
    fn new_repository_is_empty() {
        let repository = PackageRepository::new();

        assert!(repository.manifests().is_empty());
    }

    #[test]
    fn repository_can_be_created_from_manifests() {
        let manifest = PackageManifest::new("desktop", vec!["gnome-shell".to_owned()]);

        let repository = PackageRepository::from_manifests(vec![manifest.clone()])
            .expect("valid package repository");

        assert_eq!(repository.manifests(), &[manifest]);
    }

    #[test]
    fn duplicate_manifest_names_are_rejected() {
        let manifests = vec![
            PackageManifest::new("desktop", vec!["gnome-shell".to_owned()]),
            PackageManifest::new("desktop", vec!["gdm3".to_owned()]),
        ];

        let error = PackageRepository::from_manifests(manifests)
            .expect_err("duplicate manifest names should fail");

        assert!(matches!(
            error,
            RegistryError::DuplicatePackageManifest(name)
                if name == "desktop"
        ));
    }

    #[test]
    fn repository_can_be_created_from_yaml() {
        let repository = PackageRepository::from_yaml_str(DESKTOP_MANIFEST_YAML)
            .expect("package manifest YAML should load");

        let manifest = repository
            .manifest("desktop")
            .expect("desktop manifest should exist");

        assert_eq!(
            manifest.packages(),
            &["gnome-shell".to_owned(), "gdm3".to_owned()]
        );
    }

    #[test]
    fn directory_loads_multiple_package_manifests() {
        let directory = tempfile::tempdir().expect("temporary directory should exist");

        fs::write(directory.path().join("desktop.yaml"), DESKTOP_MANIFEST_YAML)
            .expect("desktop manifest YAML should be written");

        fs::write(directory.path().join("ssh.yml"), SSH_MANIFEST_YAML)
            .expect("SSH manifest YAML should be written");

        let repository = PackageRepository::from_directory(directory.path())
            .expect("package-manifest directory should load");

        assert_eq!(repository.manifests().len(), 2);
        assert!(repository.manifest("desktop").is_some());
        assert!(repository.manifest("ssh").is_some());
    }

    #[test]
    fn manifest_returns_none_for_unknown_name() {
        let repository = PackageRepository::from_yaml_str(DESKTOP_MANIFEST_YAML)
            .expect("package manifest YAML should load");

        assert!(repository.manifest("does-not-exist").is_none());
    }

    #[test]
    fn malformed_manifest_in_directory_returns_parse_error() {
        let directory = tempfile::tempdir().expect("temporary directory should exist");

        fs::write(
            directory.path().join("desktop.yaml"),
            "name: desktop\npackages: [",
        )
        .expect("malformed YAML should be written");

        let error = PackageRepository::from_directory(directory.path())
            .expect_err("invalid YAML should fail");

        assert!(matches!(error, RegistryError::Parse(_)));
    }

    #[test]
    fn missing_directory_returns_io_error() {
        let directory = tempfile::tempdir().expect("temporary directory should exist");

        let path = directory.path().join("does-not-exist");

        let error =
            PackageRepository::from_directory(path).expect_err("missing directory should fail");

        assert!(matches!(error, RegistryError::Io(_)));
    }

    #[test]
    fn non_yaml_files_are_ignored() {
        let directory = tempfile::tempdir().expect("temporary directory should exist");

        fs::write(directory.path().join("README.md"), "# Example")
            .expect("README should be written");

        fs::write(directory.path().join("desktop.yaml"), DESKTOP_MANIFEST_YAML)
            .expect("desktop manifest YAML should be written");

        let repository =
            PackageRepository::from_directory(directory.path()).expect("directory should load");

        assert_eq!(repository.manifests().len(), 1);
        assert!(repository.manifest("desktop").is_some());
    }

    #[test]
    fn empty_directory_creates_empty_repository() {
        let directory = tempfile::tempdir().expect("temporary directory should exist");

        let repository = PackageRepository::from_directory(directory.path())
            .expect("empty directory should load");

        assert!(repository.manifests().is_empty());
    }
}
