use model::{PackageManifest, PackageManifestRealization};

use crate::PackageRepository;

pub(crate) fn resolve_package_manifest_realization<'a>(
    realization: &PackageManifestRealization,
    repository: &'a PackageRepository,
) -> Option<&'a PackageManifest> {
    repository.manifest(realization.package_manifest())
}

#[cfg(test)]
mod tests {
    use model::{PackageManifest, PackageManifestRealization};

    use crate::PackageRepository;

    use super::resolve_package_manifest_realization;

    #[test]
    fn resolves_package_manifest_realization() {
        let repository = PackageRepository::from_manifests(vec![PackageManifest::new(
            "desktop",
            vec!["gdm3".to_owned()],
        )])
        .expect("valid package repository");
        let realization = PackageManifestRealization::new("desktop");

        let manifest = resolve_package_manifest_realization(&realization, &repository)
            .expect("package manifest realization should resolve");

        assert_eq!(manifest.name(), "desktop");
        assert_eq!(manifest.packages(), &["gdm3".to_owned()]);
    }

    #[test]
    fn missing_package_manifest_realization_does_not_resolve() {
        let repository = PackageRepository::new();
        let realization = PackageManifestRealization::new("desktop");

        assert!(resolve_package_manifest_realization(&realization, &repository).is_none());
    }
}
