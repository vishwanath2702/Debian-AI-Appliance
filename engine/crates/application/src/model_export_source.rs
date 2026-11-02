//! Source access for model export without taking ownership of user mounts.

use std::fs;
use std::path::Path;

use inspector::ContentVolumeMounter;
use model::DiscoveredContentVolume;

/// Executes an export operation against an already-mounted source directory.
///
/// The directory remains mounted and is never unmounted by this function.
pub fn with_mounted_model_export_source<F, T>(
    directory: &Path,
    operation: F,
) -> Result<T, String>
where
    F: FnOnce(&Path) -> Result<T, String>,
{
    let metadata = fs::symlink_metadata(directory)
        .map_err(|error| format!("Cannot inspect model export source: {error}"))?;

    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err("Model export source must be a real directory".to_owned());
    }

    operation(directory)
}

/// Executes an export operation on a DAIA-owned temporary read-only mount.
///
/// The existing model-content mount guard handles cleanup after the operation.
pub fn with_unmounted_model_export_volume<M, F, T>(
    mounter: &M,
    volume: &DiscoveredContentVolume,
    operation: F,
) -> Result<T, String>
where
    M: ContentVolumeMounter,
    F: FnOnce(&Path) -> Result<T, String>,
{
    if volume.mountpoint().is_some() {
        return Err(
            "Volume is already mounted; use mounted-directory source access".to_owned()
        );
    }

    super::post_install_models::with_model_content_volume(
        mounter,
        volume,
        operation,
    )
}

/// Plans an Ollama model export from an accessible source directory.
///
/// The selected manifest is relative to the Ollama model store.
/// No source files are modified.
pub fn plan_ollama_export_from_mounted_source(
    model_store: &Path,
    relative_manifest: &Path,
) -> Result<engine::ollama_export::OllamaExportPlan, String> {
    use std::path::Component;

    if relative_manifest.components().any(|component| {
        !matches!(component, Component::Normal(_))
    }) {
        return Err("Invalid relative Ollama manifest path".to_owned());
    }

    with_mounted_model_export_source(model_store, |store| {
        let manifest = store.join("manifests").join(relative_manifest);

        engine::ollama_export::plan_ollama_export(store, &manifest)
            .map_err(|error| format!("Cannot plan Ollama export: {error}"))
    })
}

/// Plans an Ollama export from a DAIA-owned temporary read-only mount.
///
/// Planning and source validation complete before DAIA unmounts the volume.
pub fn plan_ollama_export_from_unmounted_volume<M>(
    mounter: &M,
    volume: &DiscoveredContentVolume,
    relative_store: &Path,
    relative_manifest: &Path,
) -> Result<engine::ollama_export::OllamaExportPlan, String>
where
    M: ContentVolumeMounter,
{
    use std::path::Component;

    if relative_store.components().any(|component| {
        !matches!(component, Component::Normal(_))
    }) {
        return Err("Invalid relative Ollama store path".to_owned());
    }

    with_unmounted_model_export_volume(mounter, volume, |mountpoint| {
        plan_ollama_export_from_mounted_source(
            &mountpoint.join(relative_store),
            relative_manifest,
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockMounter {
        root: std::path::PathBuf,
        mounts: std::cell::Cell<usize>,
        unmounts: std::cell::Cell<usize>,
    }

    impl ContentVolumeMounter for MockMounter {
        fn mount_read_only(
            &self,
            _: &DiscoveredContentVolume,
        ) -> Result<std::path::PathBuf, inspector::StorageInspectError> {
            self.mounts.set(self.mounts.get() + 1);
            Ok(self.root.clone())
        }

        fn unmount(
            &self,
            _: &Path,
        ) -> Result<(), inspector::StorageInspectError> {
            self.unmounts.set(self.unmounts.get() + 1);
            Ok(())
        }
    }

    #[test]
    fn temporary_mount_is_cleaned_up_after_success() {
        let directory = tempfile::tempdir().unwrap();

        let mounter = MockMounter {
            root: directory.path().to_path_buf(),
            mounts: std::cell::Cell::new(0),
            unmounts: std::cell::Cell::new(0),
        };

        let volume = DiscoveredContentVolume::new(
            "/dev/sdb1", "/dev/sdb", "ext4"
        );

        let result = with_unmounted_model_export_volume(
            &mounter,
            &volume,
            |path| {
                assert_eq!(path, directory.path());
                Ok(42)
            },
        ).unwrap();

        assert_eq!(result, 42);
        assert_eq!(mounter.mounts.get(), 1);
        assert_eq!(mounter.unmounts.get(), 1);
    }

    #[test]
    fn temporary_mount_is_cleaned_up_after_failure() {
        let directory = tempfile::tempdir().unwrap();

        let mounter = MockMounter {
            root: directory.path().to_path_buf(),
            mounts: std::cell::Cell::new(0),
            unmounts: std::cell::Cell::new(0),
        };

        let volume = DiscoveredContentVolume::new(
            "/dev/sdc1", "/dev/sdc", "ext4"
        );

        let error = with_unmounted_model_export_volume(
            &mounter,
            &volume,
            |_| -> Result<(), String> {
                Err("export failed".to_owned())
            },
        ).unwrap_err();

        assert_eq!(error, "export failed");
        assert_eq!(mounter.mounts.get(), 1);
        assert_eq!(mounter.unmounts.get(), 1);
    }

    #[test]
    fn plans_ollama_export_on_temporary_volume_and_cleans_up() {
        let directory = tempfile::tempdir().unwrap();
        let store = directory.path().join("ollama/models");
        let manifests = store.join(
            "manifests/registry.ollama.ai/library/example"
        );
        let blobs = store.join("blobs");

        fs::create_dir_all(&manifests).unwrap();
        fs::create_dir_all(&blobs).unwrap();

        let hash = "b".repeat(64);
        let digest = format!("sha256:{hash}");

        fs::write(
            blobs.join(format!("sha256-{hash}")),
            b"test",
        ).unwrap();

        let manifest_path = manifests.join("latest");

        fs::write(
            &manifest_path,
            serde_json::to_vec(&serde_json::json!({
                "schemaVersion": 2,
                "layers": [{
                    "digest": digest,
                    "size": 4
                }]
            })).unwrap(),
        ).unwrap();

        let mounter = MockMounter {
            root: directory.path().to_path_buf(),
            mounts: std::cell::Cell::new(0),
            unmounts: std::cell::Cell::new(0),
        };

        let volume = DiscoveredContentVolume::new(
            "/dev/sdb1", "/dev/sdb", "ext4"
        );

        let plan = plan_ollama_export_from_unmounted_volume(
            &mounter,
            &volume,
            Path::new("ollama/models"),
            Path::new("registry.ollama.ai/library/example/latest"),
        ).unwrap();

        assert_eq!(plan.manifest_path, manifest_path);
        assert_eq!(plan.blobs.len(), 1);
        assert_eq!(mounter.mounts.get(), 1);
        assert_eq!(mounter.unmounts.get(), 1);
    }

    #[test]
    fn plans_ollama_manifest_from_mounted_store() {
        let directory = tempfile::tempdir().unwrap();
        let store = directory.path().join("models");
        let manifests = store.join(
            "manifests/registry.ollama.ai/library/example"
        );
        let blobs = store.join("blobs");

        fs::create_dir_all(&manifests).unwrap();
        fs::create_dir_all(&blobs).unwrap();

        let hash = "a".repeat(64);
        let digest = format!("sha256:{hash}");

        fs::write(
            blobs.join(format!("sha256-{hash}")),
            b"test",
        ).unwrap();

        let manifest = serde_json::json!({
            "schemaVersion": 2,
            "config": {
                "digest": digest,
                "size": 4
            },
            "layers": []
        });

        let manifest_path = manifests.join("latest");

        fs::write(
            &manifest_path,
            serde_json::to_vec(&manifest).unwrap(),
        ).unwrap();

        let relative = Path::new(
            "registry.ollama.ai/library/example/latest"
        );

        let plan = plan_ollama_export_from_mounted_source(
            &store,
            relative,
        ).unwrap();

        assert_eq!(plan.manifest_path, manifest_path);
        assert_eq!(plan.blobs.len(), 1);
        assert_eq!(plan.blobs[0].size_bytes, 4);
        assert_eq!(
            plan.total_bytes,
            fs::metadata(&manifest_path).unwrap().len() + 4
        );

        assert!(store.is_dir());
        assert!(manifest_path.is_file());
    }

    #[test]
    fn rejects_ollama_manifest_path_traversal() {
        let directory = tempfile::tempdir().unwrap();

        let result = plan_ollama_export_from_mounted_source(
            directory.path(),
            Path::new("../outside/latest"),
        );

        assert!(result.is_err());
    }

    #[test]
    fn mounted_directory_remains_available() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("external-models");
        fs::create_dir(&source).unwrap();

        let result = with_mounted_model_export_source(&source, |path| {
            assert!(path.is_dir());
            Ok(path.to_path_buf())
        }).unwrap();

        assert_eq!(result, source);
        assert!(source.is_dir());
    }

    #[test]
    fn mounted_directory_preserves_operation_error() {
        let directory = tempfile::tempdir().unwrap();

        let error = with_mounted_model_export_source(
            directory.path(),
            |_| -> Result<(), String> {
                Err("export interrupted".to_owned())
            },
        ).unwrap_err();

        assert_eq!(error, "export interrupted");
        assert!(directory.path().is_dir());
    }

    #[cfg(unix)]
    #[test]
    fn mounted_directory_rejects_symlink() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().unwrap();
        let link = directory.path().join("linked-source");
        symlink(directory.path(), &link).unwrap();

        assert!(
            with_mounted_model_export_source(&link, |_| Ok(())).is_err()
        );
    }

    #[test]
    fn mounted_directory_rejects_regular_file() {
        let directory = tempfile::tempdir().unwrap();
        let file = directory.path().join("model.gguf");
        fs::write(&file, b"GGUF").unwrap();

        assert!(
            with_mounted_model_export_source(&file, |_| Ok(())).is_err()
        );
    }
}
