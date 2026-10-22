/// A model artifact successfully discovered in a selected directory.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiscoveredModel {
    pub item: model::ExternalContentItem,
    pub metadata: inspector::GgufMetadata,
}

/// A model artifact that could not be inspected.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelDiscoveryIssue {
    pub path: std::path::PathBuf,
    pub message: String,
}

/// Results of scanning a model directory.
///
/// Valid models and per-file inspection issues are reported separately.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ModelDiscoveryReport {
    pub models: Vec<DiscoveredModel>,
    pub issues: Vec<ModelDiscoveryIssue>,
}

/// Ensures a DAIA-owned content mount is cleaned up on scope exit.
struct ModelContentMountGuard<'a, M: inspector::ContentVolumeMounter> {
    mounter: &'a M,
    mountpoint: std::path::PathBuf,
    cleanup_attempted: bool,
}

impl<M: inspector::ContentVolumeMounter> ModelContentMountGuard<'_, M> {
    fn cleanup(&mut self) -> Result<(), inspector::StorageInspectError> {
        self.cleanup_attempted = true;
        self.mounter.unmount(&self.mountpoint)
    }
}

impl<M: inspector::ContentVolumeMounter> Drop for ModelContentMountGuard<'_, M> {
    fn drop(&mut self) {
        if !self.cleanup_attempted {
            if let Err(error) = self.cleanup() {
                eprintln!("Error unmounting model content volume during cleanup: {error}");
            }
        }
    }
}

/// Runs an operation with a DAIA-owned content volume mounted read-only.
///
/// The mount is cleaned up after the operation, including during panic unwinding.
pub(super) fn with_model_content_volume<M, F, T>(
    mounter: &M,
    volume: &model::DiscoveredContentVolume,
    operation: F,
) -> Result<T, String>
where
    M: inspector::ContentVolumeMounter,
    F: FnOnce(&std::path::Path) -> Result<T, String>,
{
    let mountpoint = mounter
        .mount_read_only(volume)
        .map_err(|error| format!("Error mounting model content volume: {error}"))?;

    let mut guard = ModelContentMountGuard {
        mounter,
        mountpoint,
        cleanup_attempted: false,
    };

    let operation_result = operation(&guard.mountpoint);
    let cleanup_result = guard.cleanup();

    match (operation_result, cleanup_result) {
        (Ok(value), Ok(())) => Ok(value),
        (Err(error), Ok(())) => Err(error),
        (Ok(_), Err(error)) => Err(format!("Error unmounting model content volume: {error}")),
        (Err(operation_error), Err(cleanup_error)) => Err(format!(
            "{operation_error}; also failed to unmount model content volume: {cleanup_error}"
        )),
    }
}

/// Validates a model filename selected from removable storage.
///
/// The filename is resolved against a fresh mountpoint during import.
///
/// # Errors
///
/// Rejects empty names, directory components, and non-GGUF extensions.
pub fn validate_selected_model_filename(filename: &std::path::Path) -> Result<(), String> {
    use std::path::Component;

    let mut components = filename.components();

    if !matches!(components.next(), Some(Component::Normal(_))) || components.next().is_some() {
        return Err("Selected model must be a single filename".to_owned());
    }

    let is_gguf = filename.extension().is_some_and(|extension| {
        extension
            .to_str()
            .is_some_and(|value| value.eq_ignore_ascii_case("gguf"))
    });

    if !is_gguf {
        return Err("Selected model must have a GGUF extension".to_owned());
    }

    Ok(())
}

/// Resolves a selected model within its mounted source directory.
///
/// Rejects symbolic links and files that are not regular GGUF files.
///
/// # Errors
///
/// Returns an error if the filename or source file is invalid.
pub fn resolve_selected_model_path(
    mountpoint: &std::path::Path,
    filename: &std::path::Path,
) -> Result<std::path::PathBuf, String> {
    validate_selected_model_filename(filename)?;

    let path = mountpoint.join(filename);
    let metadata = std::fs::symlink_metadata(&path)
        .map_err(|error| format!("Error inspecting selected model: {error}"))?;

    if !metadata.file_type().is_file() {
        return Err("Selected model must be a regular file, not a symbolic link".to_owned());
    }

    let canonical_mountpoint = mountpoint
        .canonicalize()
        .map_err(|error| format!("Error resolving model mountpoint: {error}"))?;

    let canonical_path = path
        .canonicalize()
        .map_err(|error| format!("Error resolving selected model: {error}"))?;

    if !canonical_path.starts_with(&canonical_mountpoint) {
        return Err("Selected model is outside the mounted source".to_owned());
    }

    Ok(canonical_path)
}

/// Selects exactly one volume from a supplied discovery result.
///
/// The caller should obtain the volume list from fresh storage discovery.
///
/// # Errors
///
/// Returns an error if the requested device is absent or ambiguous.
pub fn select_post_install_model_volume(
    volumes: &[model::DiscoveredContentVolume],
    requested_device: &std::path::Path,
) -> Result<model::DiscoveredContentVolume, String> {
    let mut matches = volumes
        .iter()
        .filter(|volume| volume.device_path() == requested_device);

    let selected = matches.next().ok_or_else(|| {
        format!(
            "Model content volume was not discovered: {}",
            requested_device.display()
        )
    })?;

    if matches.next().is_some() {
        return Err(format!(
            "Ambiguous model content volume device: {}",
            requested_device.display()
        ));
    }

    Ok(selected.clone())
}

/// Discovers GGUF models on a selected content volume.
///
/// Mounts the volume read-only, scans it, and cleans up the mount.
///
/// # Errors
///
/// Returns an error if mounting, scanning, or cleanup fails.
pub fn discover_models_on_volume_with<M>(
    mounter: &M,
    volume: &model::DiscoveredContentVolume,
) -> Result<ModelDiscoveryReport, String>
where
    M: inspector::ContentVolumeMounter,
{
    with_model_content_volume(mounter, volume, |mountpoint| {
        discover_models_with_report(mountpoint)
            .map_err(|error| format!("Error scanning model content volume: {error}"))
    })
}

/// Discovers valid models and reports individual inspection failures.
///
/// Scans files directly inside the selected directory without importing them.
///
/// # Errors
///
/// Returns an error if the directory is invalid or cannot be enumerated.
pub fn discover_models_with_report(
    directory: &std::path::Path,
) -> Result<ModelDiscoveryReport, Box<dyn std::error::Error>> {
    if !directory.is_dir() {
        return Err(format!("Model source is not a directory: {}", directory.display()).into());
    }

    let locator = directory
        .to_str()
        .ok_or("Model source path is not valid UTF-8")?;

    let repository = model::ContentRepository::with_sources(
        "selected-model-folder",
        "User-selected model directory",
        vec![model::ContentSource::new(
            "selected-model-folder-source",
            model::ContentRepositoryId::new("selected-model-folder"),
            locator,
        )],
    );

    let engine = engine::Engine::from_registry(registry::Registry::default());
    let inspector = inspector::LocalFilesystemContentInspector::new();
    let mut report = ModelDiscoveryReport::default();

    for item in engine.repository_content_items(&repository, &inspector)? {
        let is_gguf = item.path().extension().is_some_and(|extension| {
            extension
                .to_str()
                .is_some_and(|value| value.eq_ignore_ascii_case("gguf"))
        });

        let inspection = if is_gguf {
            inspector::inspect_gguf(item.path())
                .map(Some)
                .map_err(|error| error.to_string())
        } else {
            engine
                .inspect_external_model(&item)
                .map_err(|error| error.to_string())
        };

        match inspection {
            Ok(Some(metadata)) => report.models.push(DiscoveredModel { item, metadata }),
            Ok(None) => {}
            Err(message) => report.issues.push(ModelDiscoveryIssue {
                path: item.path().to_path_buf(),
                message,
            }),
        }
    }

    report
        .models
        .sort_by(|left, right| left.item.path().cmp(right.item.path()));
    report
        .issues
        .sort_by(|left, right| left.path.cmp(&right.path));
    Ok(report)
}

/// Discovers GGUF models directly inside a selected directory.
///
/// This operation does not import files or modify appliance state.
///
/// # Errors
///
/// Returns an error if content enumeration or GGUF inspection fails.
pub fn discover_models_in_directory(
    directory: &std::path::Path,
) -> Result<Vec<(model::ExternalContentItem, inspector::GgufMetadata)>, Box<dyn std::error::Error>>
{
    if !directory.is_dir() {
        return Err(format!("Model source is not a directory: {}", directory.display()).into());
    }

    let locator = directory
        .to_str()
        .ok_or("Model source path is not valid UTF-8")?;

    let repository_id = model::ContentRepositoryId::new("selected-model-folder");
    let repository = model::ContentRepository::with_sources(
        "selected-model-folder",
        "User-selected model directory",
        vec![model::ContentSource::new(
            "selected-model-folder-source",
            repository_id,
            locator,
        )],
    );

    let engine = engine::Engine::from_registry(registry::Registry::default());
    let inspector = inspector::LocalFilesystemContentInspector::new();
    let mut models = Vec::new();

    for item in engine.repository_content_items(&repository, &inspector)? {
        let metadata = if item.path().extension().is_some_and(|extension| {
            extension
                .to_str()
                .is_some_and(|value| value.eq_ignore_ascii_case("gguf"))
        }) {
            Some(inspector::inspect_gguf(item.path())?)
        } else {
            engine.inspect_external_model(&item)?
        };

        if let Some(metadata) = metadata {
            models.push((item, metadata));
        }
    }

    models.sort_by(|left, right| left.0.path().cmp(right.0.path()));
    Ok(models)
}

/// Discovers storage volumes available for post-install content browsing.
///
/// This operation only inspects storage; it does not mount or modify volumes.
///
/// # Errors
///
/// Returns an error if system storage inspection fails.
pub fn discover_post_install_model_volumes()
-> Result<Vec<model::DiscoveredContentVolume>, inspector::StorageInspectError> {
    use inspector::ContentVolumeInspector;

    inspector::LinuxContentVolumeInspector::new()
        .with_post_install_discovery()
        .inspect()
}

/// Copies a selected GGUF from a mounted volume into appliance storage.
///
/// Validates the source, copies it, and persists the desired model state
/// before releasing the read-only mount.
///
/// # Errors
///
/// Returns an error if validation, import, persistence, or cleanup fails.
pub fn import_selected_model_on_volume_with<M>(
    mounter: &M,
    volume: &model::DiscoveredContentVolume,
    filename: &std::path::Path,
    model_name: &str,
    root: &std::path::Path,
) -> Result<(), String>
where
    M: inspector::ContentVolumeMounter,
{
    if model_name.trim().is_empty() {
        return Err("DAIA model name cannot be empty".to_owned());
    }

    validate_selected_model_filename(filename)?;

    let appliance_state = state::ApplianceState::read_from_root(root)
        .map_err(|error| format!("Error loading appliance state: {error}"))?;

    if appliance_state
        .model_realizations()
        .iter()
        .any(|realization| realization.id().as_str() == model_name)
    {
        return Err(format!(
            "Model name {model_name:?} already exists; explicit replacement is required"
        ));
    }

    with_model_content_volume(mounter, volume, |mountpoint| {
        let path = resolve_selected_model_path(mountpoint, filename)?;

        inspector::inspect_gguf(&path)
            .map_err(|error| format!("Error inspecting selected GGUF: {error}"))?;

        let item = model::ExternalContentItem::new(
            model::ContentSourceId::new("post-install-model-volume"),
            path,
        );

        let engine = engine::Engine::from_registry(registry::Registry::default());

        let prepared = engine
            .prepare_content_import(
                model::ContentImportIntent::new(vec![item.id().clone()]),
                vec![item.clone()],
                model::ContentImportDestination::new("/var/lib/daia/content"),
            )
            .map_err(|error| format!("Error preparing model import: {error}"))?;

        let realization_intent = model::ModelRealizationIntent::new(
            model::ModelRealizationId::new(model_name),
            model::InferenceEngineId::ollama(),
            item.id().clone(),
        );

        apply_post_install_model_content_import(&engine, &prepared, &[realization_intent], root)
    })
}

/// Imports a selected GGUF, unmounts its source, then realizes the model.
///
/// Ollama realization is attempted only after successful import and cleanup.
///
/// # Errors
///
/// Returns an error if import, unmounting, state loading, or realization fails.
pub fn import_and_realize_selected_model_on_volume_with<M, E>(
    mounter: &M,
    volume: &model::DiscoveredContentVolume,
    filename: &std::path::Path,
    model_name: &str,
    root: &std::path::Path,
    executor: &mut E,
) -> Result<(), String>
where
    M: inspector::ContentVolumeMounter,
    E: engine::ModelRealizationExecutor,
    E::Error: std::fmt::Display,
{
    import_selected_model_on_volume_with(mounter, volume, filename, model_name, root)?;

    let appliance_state = state::ApplianceState::read_from_root(root)
        .map_err(|error| format!("Error loading appliance state: {error}"))?;

    let selected: Vec<_> = appliance_state
        .model_realizations()
        .iter()
        .filter(|realization| realization.id().as_str() == model_name)
        .cloned()
        .collect();

    if selected.len() != 1 {
        return Err(format!(
            "Expected exactly one imported model named {model_name:?}, found {}",
            selected.len()
        ));
    }

    realize_models_with(&selected, executor)
        .map_err(|error| format!("Error realizing selected model: {error}"))
}

/// Inspects a selected GGUF file on a read-only mounted volume.
///
/// The volume is unmounted after inspection, including when inspection fails.
///
/// # Errors
///
/// Returns an error if mounting, file validation, inspection, or cleanup fails.
pub fn inspect_selected_model_on_volume_with<M>(
    mounter: &M,
    volume: &model::DiscoveredContentVolume,
    filename: &std::path::Path,
) -> Result<inspector::GgufMetadata, String>
where
    M: inspector::ContentVolumeMounter,
{
    with_model_content_volume(mounter, volume, |mountpoint| {
        let path = resolve_selected_model_path(mountpoint, filename)?;

        inspector::inspect_gguf(&path)
            .map_err(|error| format!("Error inspecting selected GGUF: {error}"))
    })
}

/// Discovers models on a volume selected from a supplied volume list.
///
/// Selects the requested device, mounts it read-only, scans for GGUF models,
/// and releases the mount.
///
/// # Errors
///
/// Returns an error if selection, mounting, scanning, or cleanup fails.
pub fn discover_models_on_selected_volume_with<M>(
    mounter: &M,
    volumes: &[model::DiscoveredContentVolume],
    requested_device: &std::path::Path,
) -> Result<ModelDiscoveryReport, String>
where
    M: inspector::ContentVolumeMounter,
{
    let selected = select_post_install_model_volume(volumes, requested_device)?;
    discover_models_on_volume_with(mounter, &selected)
}

/// Discovers models after refreshing the available content volumes.
///
/// Performs discovery before selecting a device, then mounts it read-only,
/// scans its GGUF files, and unmounts it.
///
/// # Errors
///
/// Returns an error if discovery, selection, mounting, scanning, or cleanup fails.
pub fn discover_models_on_fresh_volume_with<I, M>(
    volume_inspector: &I,
    mounter: &M,
    requested_device: &std::path::Path,
) -> Result<ModelDiscoveryReport, String>
where
    I: inspector::ContentVolumeInspector,
    M: inspector::ContentVolumeMounter,
{
    let volumes = volume_inspector
        .inspect()
        .map_err(|error| format!("Error discovering model content volumes: {error}"))?;

    discover_models_on_selected_volume_with(mounter, &volumes, requested_device)
}

/// Discovers GGUF models on a selected post-install storage volume.
///
/// Refreshes volume discovery, validates the requested device, mounts it
/// read-only, scans its models, and unmounts it.
///
/// # Errors
///
/// Returns an error if discovery, selection, mounting, scanning, or cleanup fails.
pub fn discover_models_on_post_install_volume(
    requested_device: &std::path::Path,
) -> Result<ModelDiscoveryReport, String> {
    let volume_inspector =
        inspector::LinuxContentVolumeInspector::new().with_post_install_discovery();
    let mounter = inspector::LinuxContentVolumeMounter::new();

    discover_models_on_fresh_volume_with(&volume_inspector, &mounter, requested_device)
}

/// Copies imported model content and persists its desired state.
///
/// The operation does not realize models. The source files must remain
/// accessible until this function returns.
///
/// # Errors
///
/// Returns an error if copying or appliance-state persistence fails.
pub fn apply_post_install_model_content_import(
    engine: &engine::Engine,
    prepared: &engine::PreparedContentImport,
    model_realization_intents: &[model::ModelRealizationIntent],
    root: impl AsRef<std::path::Path>,
) -> Result<(), String> {
    engine
        .apply_post_install_content_import(prepared, model_realization_intents, root)
        .map_err(|error| format!("Error importing model content: {error}"))
}

/// Imports model content, persists desired state, and realizes models.
///
/// # Errors
///
/// Returns an error from content import, state loading, or model realization.
pub fn apply_post_install_model_import_with<E>(
    engine: &engine::Engine,
    prepared: &engine::PreparedContentImport,
    model_realization_intents: &[model::ModelRealizationIntent],
    root: impl AsRef<std::path::Path>,
    executor: &mut E,
) -> Result<(), String>
where
    E: engine::ModelRealizationExecutor,
    E::Error: std::fmt::Display,
{
    let root = root.as_ref();

    apply_post_install_model_content_import(engine, prepared, model_realization_intents, root)?;

    let appliance_state = state::ApplianceState::read_from_root(root)
        .map_err(|error| format!("Error loading appliance state: {error}"))?;

    realize_models_with(appliance_state.model_realizations(), executor)
        .map_err(|error| format!("Error realizing models: {error}"))
}

/// Retries realization of an existing imported model without importing it again.
///
/// The persisted model and its content are left unchanged.
///
/// # Errors
///
/// Returns an error if the model is missing, ambiguous, its stored content is
/// unavailable, or realization fails.
pub fn resume_model_realization_with<E>(
    model_name: &str,
    root: &std::path::Path,
    executor: &mut E,
) -> Result<(), String>
where
    E: engine::ModelRealizationExecutor,
    E::Error: std::fmt::Display,
{
    if model_name.trim().is_empty() {
        return Err("DAIA model name cannot be empty".to_owned());
    }

    let appliance_state = state::ApplianceState::read_from_root(root)
        .map_err(|error| format!("Error loading appliance state: {error}"))?;

    let selected: Vec<_> = appliance_state
        .model_realizations()
        .iter()
        .filter(|realization| realization.id().as_str() == model_name)
        .cloned()
        .collect();

    if selected.len() != 1 {
        return Err(format!(
            "Expected exactly one existing model named {model_name:?}, found {}",
            selected.len()
        ));
    }

    let stored_path = selected[0].content().path();

    let content_directory = std::path::Path::new("/var/lib/daia/content");

    let filename = stored_path
        .strip_prefix(content_directory)
        .map_err(|_| "Imported model path is outside DAIA content storage".to_owned())?;

    if filename.components().count() != 1
        || !matches!(
            filename.components().next(),
            Some(std::path::Component::Normal(_))
        )
    {
        return Err("Imported model path contains unsafe components".to_owned());
    }

    let content_directory_path = root.join("var/lib/daia/content");

    for component in ["var", "var/lib", "var/lib/daia", "var/lib/daia/content"] {
        let directory_path = root.join(component);
        let metadata = std::fs::symlink_metadata(&directory_path).map_err(|error| {
            format!(
                "Cannot inspect DAIA directory {}: {error}",
                directory_path.display()
            )
        })?;

        if !metadata.file_type().is_dir() {
            return Err(format!(
                "DAIA directory must be a real directory, not a symlink: {}",
                directory_path.display()
            ));
        }
    }

    let content_path = content_directory_path.join(filename);

    let metadata = std::fs::symlink_metadata(&content_path).map_err(|error| {
        format!(
            "Cannot inspect imported model content {}: {error}",
            stored_path.display()
        )
    })?;

    if metadata.file_type().is_symlink() {
        return Err(format!(
            "Imported model content must not be a symlink: {}",
            stored_path.display()
        ));
    }

    if !content_path.is_file() {
        return Err(format!(
            "Imported model content is missing or not a regular file: {}",
            stored_path.display()
        ));
    }

    inspector::inspect_gguf(&content_path)
        .map_err(|error| format!("Error validating imported GGUF: {error}"))?;

    realize_models_with(&selected, executor)
        .map_err(|error| format!("Error resuming model realization: {error}"))
}

/// Realizes models using the caller-provided executor.
///
/// Stops at the first realization failure.
///
/// # Errors
///
/// Returns the executor's error when a realization fails.
pub fn realize_models_with<E>(
    realizations: &[model::ModelRealization],
    executor: &mut E,
) -> Result<(), E::Error>
where
    E: engine::ModelRealizationExecutor,
{
    for realization in realizations {
        executor.execute(realization)?;
    }

    Ok(())
}

#[cfg(test)]
mod model_content_mount_tests {
    use std::{
        cell::RefCell,
        path::{Path, PathBuf},
    };

    use inspector::{ContentVolumeMounter, StorageInspectError};

    #[derive(Default)]
    struct MockMounter {
        events: RefCell<Vec<&'static str>>,
        fail_mount: bool,
        fail_unmount: bool,
    }

    impl ContentVolumeMounter for MockMounter {
        fn mount_read_only(
            &self,
            _volume: &model::DiscoveredContentVolume,
        ) -> Result<PathBuf, StorageInspectError> {
            self.events.borrow_mut().push("mount");
            if self.fail_mount {
                return Err(StorageInspectError::InvalidOutput(
                    "mock mount failure".to_owned(),
                ));
            }
            Ok(PathBuf::from("/run/daia/content/mock"))
        }

        fn unmount(&self, _mountpoint: &Path) -> Result<(), StorageInspectError> {
            self.events.borrow_mut().push("unmount");
            if self.fail_unmount {
                return Err(StorageInspectError::InvalidOutput(
                    "mock unmount failure".to_owned(),
                ));
            }
            Ok(())
        }
    }

    #[test]
    fn fresh_model_volume_success_scans_and_unmounts() {
        use inspector::ContentVolumeInspector;

        struct MockInspector {
            events: std::rc::Rc<RefCell<Vec<&'static str>>>,
        }

        impl ContentVolumeInspector for MockInspector {
            fn inspect(&self) -> Result<Vec<model::DiscoveredContentVolume>, StorageInspectError> {
                self.events.borrow_mut().push("discover");
                Ok(vec![model::DiscoveredContentVolume::new(
                    "/dev/mock1",
                    "/dev/mock",
                    "ext4",
                )])
            }
        }

        struct MockMounter<'a> {
            directory: &'a Path,
            events: std::rc::Rc<RefCell<Vec<&'static str>>>,
        }

        impl ContentVolumeMounter for MockMounter<'_> {
            fn mount_read_only(
                &self,
                volume: &model::DiscoveredContentVolume,
            ) -> Result<PathBuf, StorageInspectError> {
                assert_eq!(volume.device_path(), Path::new("/dev/mock1"));
                self.events.borrow_mut().push("mount");
                Ok(self.directory.to_path_buf())
            }

            fn unmount(&self, mountpoint: &Path) -> Result<(), StorageInspectError> {
                assert_eq!(mountpoint, self.directory);
                self.events.borrow_mut().push("unmount");
                Ok(())
            }
        }

        let directory = tempfile::tempdir().expect("temporary directory");
        let events = std::rc::Rc::new(RefCell::new(Vec::new()));

        let inspector = MockInspector {
            events: events.clone(),
        };
        let mounter = MockMounter {
            directory: directory.path(),
            events: events.clone(),
        };

        let report = super::discover_models_on_fresh_volume_with(
            &inspector,
            &mounter,
            Path::new("/dev/mock1"),
        )
        .expect("fresh volume discovery should succeed");

        assert!(report.models.is_empty());
        assert!(report.issues.is_empty());
        assert_eq!(*events.borrow(), vec!["discover", "mount", "unmount"]);
    }

    #[test]
    fn fresh_model_volume_unknown_device_prevents_mounting() {
        use inspector::ContentVolumeInspector;

        struct MockInspector;

        impl ContentVolumeInspector for MockInspector {
            fn inspect(&self) -> Result<Vec<model::DiscoveredContentVolume>, StorageInspectError> {
                Ok(vec![model::DiscoveredContentVolume::new(
                    "/dev/mock1",
                    "/dev/mock",
                    "ext4",
                )])
            }
        }

        struct RejectMount;

        impl ContentVolumeMounter for RejectMount {
            fn mount_read_only(
                &self,
                _volume: &model::DiscoveredContentVolume,
            ) -> Result<PathBuf, StorageInspectError> {
                panic!("unknown device must never be mounted");
            }

            fn unmount(&self, _mountpoint: &Path) -> Result<(), StorageInspectError> {
                panic!("unknown device must never be unmounted");
            }
        }

        let result = super::discover_models_on_fresh_volume_with(
            &MockInspector,
            &RejectMount,
            Path::new("/dev/unknown"),
        );

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not discovered"));
    }

    #[test]
    fn fresh_model_volume_discovery_failure_prevents_mounting() {
        use inspector::ContentVolumeInspector;

        struct FailingInspector;

        impl ContentVolumeInspector for FailingInspector {
            fn inspect(&self) -> Result<Vec<model::DiscoveredContentVolume>, StorageInspectError> {
                Err(StorageInspectError::Io(std::io::Error::other(
                    "mock discovery failure",
                )))
            }
        }

        struct RejectMount;

        impl ContentVolumeMounter for RejectMount {
            fn mount_read_only(
                &self,
                _volume: &model::DiscoveredContentVolume,
            ) -> Result<PathBuf, StorageInspectError> {
                panic!("discovery failure must prevent mounting");
            }

            fn unmount(&self, _mountpoint: &Path) -> Result<(), StorageInspectError> {
                panic!("discovery failure must prevent unmounting");
            }
        }

        let result = super::discover_models_on_fresh_volume_with(
            &FailingInspector,
            &RejectMount,
            Path::new("/dev/mock1"),
        );

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("Error discovering model content volumes")
        );
    }

    #[test]
    fn selected_model_volume_mounts_scans_and_unmounts() {
        struct RecordingMounter<'a> {
            directory: &'a Path,
            events: RefCell<Vec<&'static str>>,
        }

        impl ContentVolumeMounter for RecordingMounter<'_> {
            fn mount_read_only(
                &self,
                volume: &model::DiscoveredContentVolume,
            ) -> Result<PathBuf, StorageInspectError> {
                assert_eq!(volume.device_path(), Path::new("/dev/mock2"));
                self.events.borrow_mut().push("mount");
                Ok(self.directory.to_path_buf())
            }

            fn unmount(&self, mountpoint: &Path) -> Result<(), StorageInspectError> {
                assert_eq!(mountpoint, self.directory);
                self.events.borrow_mut().push("unmount");
                Ok(())
            }
        }

        let directory = tempfile::tempdir().expect("temporary directory");
        let mounter = RecordingMounter {
            directory: directory.path(),
            events: RefCell::new(Vec::new()),
        };

        let volumes = vec![
            model::DiscoveredContentVolume::new("/dev/mock1", "/dev/mock", "ext4"),
            model::DiscoveredContentVolume::new("/dev/mock2", "/dev/mock", "vfat"),
        ];

        let report = super::discover_models_on_selected_volume_with(
            &mounter,
            &volumes,
            Path::new("/dev/mock2"),
        )
        .expect("selected volume discovery should succeed");

        assert!(report.models.is_empty());
        assert!(report.issues.is_empty());
        assert_eq!(*mounter.events.borrow(), vec!["mount", "unmount"]);
    }

    #[test]
    fn selected_model_volume_rejects_ambiguous_device_without_mounting() {
        struct RejectMount;

        impl ContentVolumeMounter for RejectMount {
            fn mount_read_only(
                &self,
                _volume: &model::DiscoveredContentVolume,
            ) -> Result<PathBuf, StorageInspectError> {
                panic!("ambiguous device must never be mounted");
            }

            fn unmount(&self, _mountpoint: &Path) -> Result<(), StorageInspectError> {
                panic!("ambiguous device must never be unmounted");
            }
        }

        let volumes = vec![
            model::DiscoveredContentVolume::new("/dev/mock1", "/dev/mock", "ext4"),
            model::DiscoveredContentVolume::new("/dev/mock1", "/dev/mock", "vfat"),
        ];

        let result = super::discover_models_on_selected_volume_with(
            &RejectMount,
            &volumes,
            Path::new("/dev/mock1"),
        );

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Ambiguous"));
    }

    #[test]
    fn selected_model_volume_rejects_unknown_device_without_mounting() {
        struct RecordingMounter {
            events: RefCell<Vec<&'static str>>,
        }

        impl ContentVolumeMounter for RecordingMounter {
            fn mount_read_only(
                &self,
                _volume: &model::DiscoveredContentVolume,
            ) -> Result<PathBuf, StorageInspectError> {
                self.events.borrow_mut().push("mount");
                panic!("unknown device must never be mounted");
            }

            fn unmount(&self, _mountpoint: &Path) -> Result<(), StorageInspectError> {
                self.events.borrow_mut().push("unmount");
                panic!("unknown device must never be unmounted");
            }
        }

        let volumes = vec![model::DiscoveredContentVolume::new(
            "/dev/mock1",
            "/dev/mock",
            "ext4",
        )];
        let mounter = RecordingMounter {
            events: RefCell::new(Vec::new()),
        };

        let result = super::discover_models_on_selected_volume_with(
            &mounter,
            &volumes,
            Path::new("/dev/unknown"),
        );

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not discovered"));
        assert!(mounter.events.borrow().is_empty());
    }

    #[test]
    fn model_volume_selection_accepts_exact_match() {
        let volumes = vec![
            model::DiscoveredContentVolume::new("/dev/mock1", "/dev/mock", "ext4"),
            model::DiscoveredContentVolume::new("/dev/mock2", "/dev/mock", "vfat"),
        ];

        let selected = super::select_post_install_model_volume(&volumes, Path::new("/dev/mock2"))
            .expect("matching volume should be selected");

        assert_eq!(selected.device_path(), Path::new("/dev/mock2"));
    }

    #[test]
    fn model_volume_selection_rejects_unknown_device() {
        let volumes = vec![model::DiscoveredContentVolume::new(
            "/dev/mock1",
            "/dev/mock",
            "ext4",
        )];

        let result = super::select_post_install_model_volume(&volumes, Path::new("/dev/unknown"));

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not discovered"));
    }

    #[test]
    fn model_volume_selection_rejects_ambiguous_device() {
        let volumes = vec![
            model::DiscoveredContentVolume::new("/dev/mock1", "/dev/mock", "ext4"),
            model::DiscoveredContentVolume::new("/dev/mock1", "/dev/mock", "vfat"),
        ];

        let result = super::select_post_install_model_volume(&volumes, Path::new("/dev/mock1"));

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Ambiguous"));
    }

    #[test]
    fn model_discovery_on_volume_unmounts_after_scan_failure() {
        struct MissingDirectoryMounter {
            events: RefCell<Vec<&'static str>>,
        }

        impl ContentVolumeMounter for MissingDirectoryMounter {
            fn mount_read_only(
                &self,
                _volume: &model::DiscoveredContentVolume,
            ) -> Result<PathBuf, StorageInspectError> {
                self.events.borrow_mut().push("mount");
                Ok(PathBuf::from("/nonexistent-daia-test-model-directory"))
            }

            fn unmount(&self, _mountpoint: &Path) -> Result<(), StorageInspectError> {
                self.events.borrow_mut().push("unmount");
                Ok(())
            }
        }

        let mounter = MissingDirectoryMounter {
            events: RefCell::new(Vec::new()),
        };
        let volume = model::DiscoveredContentVolume::new("/dev/mock1", "/dev/mock", "ext4");

        let result = super::discover_models_on_volume_with(&mounter, &volume);

        assert!(result.is_err());
        assert_eq!(*mounter.events.borrow(), vec!["mount", "unmount"]);
    }

    #[test]
    fn model_discovery_on_volume_reports_malformed_gguf() {
        use std::fs;

        struct DirectoryMounter<'a>(&'a Path);

        impl ContentVolumeMounter for DirectoryMounter<'_> {
            fn mount_read_only(
                &self,
                _volume: &model::DiscoveredContentVolume,
            ) -> Result<PathBuf, StorageInspectError> {
                Ok(self.0.to_path_buf())
            }

            fn unmount(&self, _mountpoint: &Path) -> Result<(), StorageInspectError> {
                Ok(())
            }
        }

        let directory = tempfile::tempdir().expect("temporary directory");
        let broken_path = directory.path().join("broken.gguf");
        fs::write(&broken_path, b"invalid GGUF data").expect("write malformed GGUF");

        let mounter = DirectoryMounter(directory.path());
        let volume = model::DiscoveredContentVolume::new("/dev/mock1", "/dev/mock", "ext4");

        let report = super::discover_models_on_volume_with(&mounter, &volume)
            .expect("discovery should return a report");

        assert!(report.models.is_empty());
        assert_eq!(report.issues.len(), 1);
        assert_eq!(report.issues[0].path, broken_path);
        assert!(!report.issues[0].message.is_empty());
    }

    #[test]
    fn model_discovery_on_volume_scans_and_unmounts() {
        use std::{fs, io::Write};

        struct DirectoryMounter<'a> {
            directory: &'a Path,
            events: RefCell<Vec<&'static str>>,
        }

        impl ContentVolumeMounter for DirectoryMounter<'_> {
            fn mount_read_only(
                &self,
                _volume: &model::DiscoveredContentVolume,
            ) -> Result<PathBuf, StorageInspectError> {
                self.events.borrow_mut().push("mount");
                Ok(self.directory.to_path_buf())
            }

            fn unmount(&self, _mountpoint: &Path) -> Result<(), StorageInspectError> {
                self.events.borrow_mut().push("unmount");
                Ok(())
            }
        }

        let directory = tempfile::tempdir().expect("temporary directory");
        let model_path = directory.path().join("test.gguf");

        let mut file = fs::File::create(&model_path).expect("create GGUF");
        file.write_all(b"GGUF").expect("magic");
        file.write_all(&3_u32.to_le_bytes()).expect("version");
        file.write_all(&0_u64.to_le_bytes()).expect("tensor count");
        file.write_all(&1_u64.to_le_bytes())
            .expect("metadata count");
        file.write_all(&20_u64.to_le_bytes()).expect("key length");
        file.write_all(b"general.architecture").expect("key");
        file.write_all(&8_u32.to_le_bytes()).expect("value type");
        file.write_all(&5_u64.to_le_bytes()).expect("value length");
        file.write_all(b"llama").expect("architecture");
        drop(file);

        let mounter = DirectoryMounter {
            directory: directory.path(),
            events: RefCell::new(Vec::new()),
        };
        let volume = model::DiscoveredContentVolume::new("/dev/mock1", "/dev/mock", "ext4");

        let report = super::discover_models_on_volume_with(&mounter, &volume)
            .expect("volume discovery should succeed");

        assert_eq!(report.models.len(), 1);
        assert_eq!(report.models[0].item.path(), model_path);
        assert!(report.issues.is_empty());
        assert_eq!(*mounter.events.borrow(), vec!["mount", "unmount"]);
    }

    #[test]
    fn model_content_volume_reports_operation_and_cleanup_failures() {
        let mounter = MockMounter {
            fail_unmount: true,
            ..Default::default()
        };
        let volume = model::DiscoveredContentVolume::new("/dev/mock1", "/dev/mock", "ext4");

        let result = super::with_model_content_volume(&mounter, &volume, |_| {
            mounter.events.borrow_mut().push("operation");
            Err::<(), String>("mock operation failure".to_owned())
        });

        let error = result.unwrap_err();

        assert!(error.contains("mock operation failure"));
        assert!(error.contains("mock unmount failure"));
        assert_eq!(
            *mounter.events.borrow(),
            vec!["mount", "operation", "unmount"]
        );
    }

    #[test]
    fn model_content_volume_does_not_run_after_mount_failure() {
        let mounter = MockMounter {
            fail_mount: true,
            ..Default::default()
        };
        let volume = model::DiscoveredContentVolume::new("/dev/mock1", "/dev/mock", "ext4");

        let result =
            super::with_model_content_volume(&mounter, &volume, |_| -> Result<(), String> {
                panic!("operation must not run after mount failure");
            });

        assert!(result.unwrap_err().contains("mock mount failure"));
        assert_eq!(*mounter.events.borrow(), vec!["mount"]);
    }

    #[test]
    fn model_content_volume_unmounts_during_panic() {
        let mounter = MockMounter::default();
        let volume = model::DiscoveredContentVolume::new("/dev/mock1", "/dev/mock", "ext4");

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = super::with_model_content_volume(&mounter, &volume, |_| {
                mounter.events.borrow_mut().push("operation");
                panic!("mock operation panic");
                #[allow(unreachable_code)]
                Ok::<(), String>(())
            });
        }));

        assert!(result.is_err());
        assert_eq!(
            *mounter.events.borrow(),
            vec!["mount", "operation", "unmount"]
        );
    }

    #[test]
    fn model_content_volume_unmounts_after_operation_failure() {
        let mounter = MockMounter::default();
        let volume = model::DiscoveredContentVolume::new("/dev/mock1", "/dev/mock", "ext4");

        let result = super::with_model_content_volume(&mounter, &volume, |_| {
            mounter.events.borrow_mut().push("operation");
            Err::<(), String>("mock operation failure".to_owned())
        });

        assert_eq!(result, Err("mock operation failure".to_owned()));
        assert_eq!(
            *mounter.events.borrow(),
            vec!["mount", "operation", "unmount"]
        );
    }

    #[test]
    fn model_content_volume_unmounts_after_success() {
        let mounter = MockMounter::default();
        let volume = model::DiscoveredContentVolume::new("/dev/mock1", "/dev/mock", "ext4");

        let result = super::with_model_content_volume(&mounter, &volume, |mountpoint| {
            assert_eq!(mountpoint, Path::new("/run/daia/content/mock"));
            mounter.events.borrow_mut().push("operation");
            Ok::<_, String>(42)
        });

        assert_eq!(result, Ok(42));
        assert_eq!(
            *mounter.events.borrow(),
            vec!["mount", "operation", "unmount"]
        );
    }
}

#[cfg(test)]
mod mounted_model_import_tests {
    use std::{
        cell::RefCell,
        path::{Path, PathBuf},
    };

    use inspector::{ContentVolumeMounter, StorageInspectError};

    struct MockMounter<'a> {
        mountpoint: &'a Path,
        root: &'a Path,
        expected_bytes: &'a [u8],
        events: RefCell<Vec<&'static str>>,
    }

    impl ContentVolumeMounter for MockMounter<'_> {
        fn mount_read_only(
            &self,
            _volume: &model::DiscoveredContentVolume,
        ) -> Result<PathBuf, StorageInspectError> {
            self.events.borrow_mut().push("mount");
            Ok(self.mountpoint.to_path_buf())
        }

        fn unmount(&self, _mountpoint: &Path) -> Result<(), StorageInspectError> {
            let imported = self.root.join("var/lib/daia/content/model.gguf");

            assert_eq!(
                std::fs::read(imported).expect("model copied before unmount"),
                self.expected_bytes,
            );

            let appliance_state = state::ApplianceState::read_from_root(self.root)
                .expect("state persisted before unmount");

            assert!(
                appliance_state
                    .model_realizations()
                    .iter()
                    .any(|realization| realization.id().as_str() == "test-model"),
                "new model must be persisted before unmount"
            );

            self.events.borrow_mut().push("unmount");
            Ok(())
        }
    }

    struct OrderedExecutor<'a> {
        events: &'a RefCell<Vec<&'static str>>,
    }

    impl engine::ModelRealizationExecutor for OrderedExecutor<'_> {
        type Error = std::io::Error;

        fn execute(&mut self, realization: &model::ModelRealization) -> Result<(), Self::Error> {
            assert_eq!(realization.id().as_str(), "test-model");
            assert_eq!(
                *self.events.borrow(),
                vec!["mount", "unmount"],
                "realization must happen after unmount"
            );
            self.events.borrow_mut().push("realize");
            Ok(())
        }
    }

    struct FailingUnmountMounter<'a> {
        mountpoint: &'a Path,
        events: RefCell<Vec<&'static str>>,
    }

    impl ContentVolumeMounter for FailingUnmountMounter<'_> {
        fn mount_read_only(
            &self,
            _volume: &model::DiscoveredContentVolume,
        ) -> Result<PathBuf, StorageInspectError> {
            self.events.borrow_mut().push("mount");
            Ok(self.mountpoint.to_path_buf())
        }

        fn unmount(&self, _mountpoint: &Path) -> Result<(), StorageInspectError> {
            self.events.borrow_mut().push("unmount-failed");
            Err(StorageInspectError::Io(std::io::Error::other(
                "simulated unmount failure",
            )))
        }
    }

    #[test]
    fn failed_unmount_prevents_model_realization() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let mountpoint = directory.path().join("usb");
        let root = directory.path().join("root");

        std::fs::create_dir(&mountpoint).expect("create mock mountpoint");

        let mut bytes = Vec::from(*b"GGUF");
        bytes.extend_from_slice(&3_u32.to_le_bytes());
        bytes.extend_from_slice(&0_u64.to_le_bytes());
        bytes.extend_from_slice(&1_u64.to_le_bytes());

        let key = b"general.architecture";
        bytes.extend_from_slice(&(key.len() as u64).to_le_bytes());
        bytes.extend_from_slice(key);
        bytes.extend_from_slice(&8_u32.to_le_bytes());

        let value = b"llama";
        bytes.extend_from_slice(&(value.len() as u64).to_le_bytes());
        bytes.extend_from_slice(value);

        std::fs::write(mountpoint.join("model.gguf"), bytes).expect("write GGUF fixture");

        state::ApplianceState::new("desktop")
            .write_to_root(&root)
            .expect("write initial appliance state");

        let mounter = FailingUnmountMounter {
            mountpoint: &mountpoint,
            events: RefCell::new(Vec::new()),
        };

        let mut executor = OrderedExecutor {
            events: &mounter.events,
        };

        let volume = model::DiscoveredContentVolume::new("/dev/mock1", "/dev/mock", "mock");

        let result = super::import_and_realize_selected_model_on_volume_with(
            &mounter,
            &volume,
            Path::new("model.gguf"),
            "test-model",
            &root,
            &mut executor,
        );

        assert!(result.is_err(), "unmount failure must propagate");
        assert_eq!(
            *mounter.events.borrow(),
            vec!["mount", "unmount-failed"],
            "executor must not run after unmount failure"
        );
    }

    #[test]
    fn duplicate_model_name_is_rejected_before_mount() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let root = temporary.path().join("appliance");
        let mountpoint = temporary.path().join("mounted");
        std::fs::create_dir_all(&mountpoint).expect("create mountpoint");

        let mut appliance_state = state::ApplianceState::new("desktop");
        appliance_state.record_model_realization(model::ModelRealization::new(
            model::ModelRealizationId::new("test-model"),
            model::InferenceEngineId::ollama(),
            model::ImportedContentItem::new(
                model::ExternalContentItemId::new("existing-source"),
                "/var/lib/daia/content/existing.gguf",
            ),
        ));
        appliance_state
            .write_to_root(&root)
            .expect("write existing appliance state");

        let mounter = MockMounter {
            mountpoint: &mountpoint,
            root: &root,
            expected_bytes: &[],
            events: RefCell::new(Vec::new()),
        };

        let volume = model::DiscoveredContentVolume::new("/dev/mock1", "/dev/mock", "mock");

        let result = super::import_selected_model_on_volume_with(
            &mounter,
            &volume,
            Path::new("new.gguf"),
            "test-model",
            &root,
        );

        assert!(result.is_err(), "duplicate model name must be rejected");
        assert!(
            result.unwrap_err().contains("already exists"),
            "error should identify duplicate model name"
        );
        assert!(
            mounter.events.borrow().is_empty(),
            "volume must not be mounted"
        );
    }

    #[test]
    fn realizes_model_only_after_successful_unmount() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let mountpoint = directory.path().join("usb");
        let root = directory.path().join("root");

        std::fs::create_dir(&mountpoint).expect("create mock mountpoint");

        let mut bytes = Vec::from(*b"GGUF");
        bytes.extend_from_slice(&3_u32.to_le_bytes());
        bytes.extend_from_slice(&0_u64.to_le_bytes());
        bytes.extend_from_slice(&1_u64.to_le_bytes());

        let key = b"general.architecture";
        bytes.extend_from_slice(&(key.len() as u64).to_le_bytes());
        bytes.extend_from_slice(key);
        bytes.extend_from_slice(&8_u32.to_le_bytes());

        let value = b"llama";
        bytes.extend_from_slice(&(value.len() as u64).to_le_bytes());
        bytes.extend_from_slice(value);

        std::fs::write(mountpoint.join("model.gguf"), &bytes).expect("write GGUF fixture");

        let mut initial_state = state::ApplianceState::new("desktop");

        initial_state.record_model_realization(model::ModelRealization::new(
            model::ModelRealizationId::new("existing-model"),
            model::InferenceEngineId::ollama(),
            model::ImportedContentItem::new(
                model::ExternalContentItemId::new("existing-source"),
                "/var/lib/daia/content/existing.gguf",
            ),
        ));

        initial_state
            .write_to_root(&root)
            .expect("write initial appliance state");

        let mounter = MockMounter {
            mountpoint: &mountpoint,
            root: &root,
            expected_bytes: &bytes,
            events: RefCell::new(Vec::new()),
        };

        let mut executor = OrderedExecutor {
            events: &mounter.events,
        };

        let volume = model::DiscoveredContentVolume::new("/dev/mock1", "/dev/mock", "mock");

        super::import_and_realize_selected_model_on_volume_with(
            &mounter,
            &volume,
            Path::new("model.gguf"),
            "test-model",
            &root,
            &mut executor,
        )
        .expect("import and realization should succeed");

        assert_eq!(
            *mounter.events.borrow(),
            vec!["mount", "unmount", "realize"]
        );

        let persisted = state::ApplianceState::read_from_root(&root)
            .expect("read appliance state after import");

        let ids: Vec<_> = persisted
            .model_realizations()
            .iter()
            .map(|realization| realization.id().as_str())
            .collect();

        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&"existing-model"));
        assert!(ids.contains(&"test-model"));
    }

    #[test]
    fn copies_and_persists_model_before_unmount() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let mountpoint = directory.path().join("usb");
        let root = directory.path().join("root");

        std::fs::create_dir(&mountpoint).expect("create mock mountpoint");

        let mut bytes = Vec::from(*b"GGUF");
        bytes.extend_from_slice(&3_u32.to_le_bytes());
        bytes.extend_from_slice(&0_u64.to_le_bytes());
        bytes.extend_from_slice(&1_u64.to_le_bytes());

        let key = b"general.architecture";
        bytes.extend_from_slice(&(key.len() as u64).to_le_bytes());
        bytes.extend_from_slice(key);
        bytes.extend_from_slice(&8_u32.to_le_bytes());

        let value = b"llama";
        bytes.extend_from_slice(&(value.len() as u64).to_le_bytes());
        bytes.extend_from_slice(value);

        std::fs::write(mountpoint.join("model.gguf"), &bytes).expect("write GGUF fixture");

        state::ApplianceState::new("desktop")
            .write_to_root(&root)
            .expect("write initial appliance state");

        let mounter = MockMounter {
            mountpoint: &mountpoint,
            root: &root,
            expected_bytes: &bytes,
            events: RefCell::new(Vec::new()),
        };

        let volume = model::DiscoveredContentVolume::new("/dev/mock1", "/dev/mock", "mock");

        super::import_selected_model_on_volume_with(
            &mounter,
            &volume,
            Path::new("model.gguf"),
            "test-model",
            &root,
        )
        .expect("mounted model import");

        assert_eq!(*mounter.events.borrow(), vec!["mount", "unmount"]);
    }
}

#[cfg(test)]
mod mounted_model_inspection_tests {
    use std::{
        cell::RefCell,
        path::{Path, PathBuf},
    };

    use inspector::{ContentVolumeMounter, StorageInspectError};

    struct MockMounter<'a> {
        mountpoint: &'a Path,
        events: RefCell<Vec<&'static str>>,
    }

    impl ContentVolumeMounter for MockMounter<'_> {
        fn mount_read_only(
            &self,
            _volume: &model::DiscoveredContentVolume,
        ) -> Result<PathBuf, StorageInspectError> {
            self.events.borrow_mut().push("mount");
            Ok(self.mountpoint.to_path_buf())
        }

        fn unmount(&self, _mountpoint: &Path) -> Result<(), StorageInspectError> {
            self.events.borrow_mut().push("unmount");
            Ok(())
        }
    }

    #[test]
    fn invalid_gguf_is_rejected_and_volume_unmounted() {
        let directory = tempfile::tempdir().expect("temporary directory");
        std::fs::write(directory.path().join("invalid.gguf"), b"not a GGUF")
            .expect("write invalid model");

        let mounter = MockMounter {
            mountpoint: directory.path(),
            events: RefCell::new(Vec::new()),
        };

        let volume = model::DiscoveredContentVolume::new("/dev/mock1", "/dev/mock", "mock");

        let result = super::inspect_selected_model_on_volume_with(
            &mounter,
            &volume,
            Path::new("invalid.gguf"),
        );

        assert!(result.is_err());
        assert_eq!(*mounter.events.borrow(), vec!["mount", "unmount"]);
    }
}

#[cfg(test)]
mod selected_model_path_tests {
    use super::resolve_selected_model_path;
    use std::path::Path;

    #[test]
    fn accepts_regular_gguf_inside_mountpoint() {
        let directory = tempfile::tempdir().expect("temporary directory");
        std::fs::write(directory.path().join("model.gguf"), b"model").expect("write model");

        let path = resolve_selected_model_path(directory.path(), Path::new("model.gguf"))
            .expect("resolve model");

        assert_eq!(path, directory.path().join("model.gguf"));
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlink_to_file_outside_mountpoint() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().expect("temporary directory");
        let outside = tempfile::tempdir().expect("outside directory");
        let outside_file = outside.path().join("model.gguf");

        std::fs::write(&outside_file, b"model").expect("write model");
        symlink(&outside_file, directory.path().join("model.gguf")).expect("create symlink");

        assert!(resolve_selected_model_path(directory.path(), Path::new("model.gguf"),).is_err());
    }
}

#[cfg(test)]
mod selected_model_filename_tests {
    use super::validate_selected_model_filename;
    use std::path::Path;

    #[test]
    fn accepts_gguf_filename() {
        assert!(validate_selected_model_filename(Path::new("model.GGUF")).is_ok());
    }

    #[test]
    fn rejects_paths_and_non_gguf_files() {
        for value in [
            "",
            "../model.gguf",
            "folder/model.gguf",
            "/tmp/model.gguf",
            "model.txt",
            ".",
        ] {
            assert!(
                validate_selected_model_filename(Path::new(value)).is_err(),
                "unexpectedly accepted {value:?}"
            );
        }
    }
}

#[cfg(test)]
mod post_install_model_import_tests {
    use super::apply_post_install_model_import_with;

    #[derive(Default)]
    struct RecordingExecutor {
        ids: Vec<String>,
    }

    impl engine::ModelRealizationExecutor for RecordingExecutor {
        type Error = std::io::Error;

        fn execute(&mut self, realization: &model::ModelRealization) -> Result<(), Self::Error> {
            self.ids.push(realization.id().as_str().to_owned());
            Ok(())
        }
    }

    #[test]
    fn imports_persists_and_realizes_model_in_temporary_root() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let root = directory.path().join("root");
        let source = directory.path().join("model.gguf");

        std::fs::write(&source, "model").expect("write test model");

        let item = model::ExternalContentItem::new(
            model::ContentSourceId::new("local-models-directory"),
            &source,
        );

        let engine = engine::Engine::from_registry(registry::Registry::default());

        let prepared = engine
            .prepare_content_import(
                model::ContentImportIntent::new(vec![item.id().clone()]),
                vec![item.clone()],
                model::ContentImportDestination::new("/var/lib/daia/content"),
            )
            .expect("prepare model import");

        let intent = model::ModelRealizationIntent::new(
            model::ModelRealizationId::new("post-install-model"),
            model::InferenceEngineId::ollama(),
            item.id().clone(),
        );

        state::ApplianceState::new("desktop")
            .write_to_root(&root)
            .expect("write initial appliance state");

        let mut executor = RecordingExecutor::default();

        apply_post_install_model_import_with(&engine, &prepared, &[intent], &root, &mut executor)
            .expect("post-install import should succeed");

        assert_eq!(executor.ids, vec!["post-install-model"]);

        let imported = root.join("var/lib/daia/content/model.gguf");

        assert_eq!(
            std::fs::read(&imported).expect("read imported model"),
            b"model"
        );

        let persisted =
            state::ApplianceState::read_from_root(&root).expect("read persisted appliance state");

        assert_eq!(persisted.model_realizations().len(), 1);
        assert_eq!(
            persisted.model_realizations()[0].id().as_str(),
            "post-install-model"
        );
    }
}

#[cfg(test)]
mod model_realization_recovery_tests {
    use engine::ModelRealizationExecutor;

    #[derive(Default)]
    struct RecordingExecutor {
        names: Vec<String>,
    }

    impl ModelRealizationExecutor for RecordingExecutor {
        type Error = String;

        fn execute(&mut self, realization: &model::ModelRealization) -> Result<(), Self::Error> {
            self.names.push(realization.id().as_str().to_owned());
            Ok(())
        }
    }

    #[cfg(unix)]
    #[test]
    fn refuses_recovery_for_symlinked_daia_directory() {
        use std::os::unix::fs::symlink;

        let temporary = tempfile::tempdir().expect("temporary directory");
        let root = temporary.path().join("appliance");
        let lib_directory = root.join("var/lib");
        let outside_daia = temporary.path().join("outside-daia");
        let outside_content = outside_daia.join("content");
        let outside_model = outside_content.join("model.gguf");

        std::fs::create_dir_all(&lib_directory).expect("create lib directory");
        std::fs::create_dir_all(&outside_content).expect("create outside content directory");
        std::fs::write(&outside_model, b"outside-content").expect("write outside model");
        symlink(&outside_daia, lib_directory.join("daia"))
            .expect("create escaping ancestor symlink");

        let mut appliance_state = state::ApplianceState::new("desktop");
        appliance_state.record_model_realization(model::ModelRealization::new(
            model::ModelRealizationId::new("ancestor-symlink-model"),
            model::InferenceEngineId::ollama(),
            model::ImportedContentItem::new(
                model::ExternalContentItemId::new("ancestor-symlink-source"),
                "/var/lib/daia/content/model.gguf",
            ),
        ));
        appliance_state
            .write_to_root(&root)
            .expect("persist test state");

        let mut executor = RecordingExecutor::default();
        let result =
            super::resume_model_realization_with("ancestor-symlink-model", &root, &mut executor);

        assert!(
            result.is_err(),
            "symlinked DAIA ancestor directory must be rejected"
        );
        assert!(
            executor.names.is_empty(),
            "executor must not run for escaping ancestor symlink"
        );
    }

    #[cfg(unix)]
    #[test]
    fn refuses_recovery_for_symlinked_content_directory() {
        use std::os::unix::fs::symlink;

        let temporary = tempfile::tempdir().expect("temporary directory");
        let root = temporary.path().join("appliance");
        let daia_directory = root.join("var/lib/daia");
        let outside_directory = temporary.path().join("outside");
        let outside = outside_directory.join("model.gguf");

        std::fs::create_dir_all(&daia_directory).expect("create DAIA directory");
        std::fs::create_dir_all(&outside_directory).expect("create outside directory");
        std::fs::write(&outside, b"outside-content").expect("write outside content");
        symlink(&outside_directory, daia_directory.join("content"))
            .expect("create escaping directory symlink");

        let mut appliance_state = state::ApplianceState::new("desktop");
        appliance_state.record_model_realization(model::ModelRealization::new(
            model::ModelRealizationId::new("directory-symlink-model"),
            model::InferenceEngineId::ollama(),
            model::ImportedContentItem::new(
                model::ExternalContentItemId::new("directory-symlink-source"),
                "/var/lib/daia/content/model.gguf",
            ),
        ));
        appliance_state
            .write_to_root(&root)
            .expect("persist test state");

        let mut executor = RecordingExecutor::default();

        let result =
            super::resume_model_realization_with("directory-symlink-model", &root, &mut executor);

        assert!(
            result.is_err(),
            "symlinked content directory must be rejected"
        );
        assert!(
            executor.names.is_empty(),
            "executor must not run for escaping directory symlink"
        );
    }

    #[cfg(unix)]
    #[test]
    fn refuses_recovery_for_symlink_escape() {
        use std::os::unix::fs::symlink;

        let temporary = tempfile::tempdir().expect("temporary directory");
        let root = temporary.path().join("appliance");
        let content_directory = root.join("var/lib/daia/content");
        let outside = root.join("var/lib/daia/outside.gguf");
        let link = content_directory.join("linked.gguf");

        std::fs::create_dir_all(&content_directory).expect("create content directory");
        std::fs::write(&outside, b"outside-content").expect("write outside content");
        symlink(&outside, &link).expect("create escaping symlink");

        let mut appliance_state = state::ApplianceState::new("desktop");
        appliance_state.record_model_realization(model::ModelRealization::new(
            model::ModelRealizationId::new("symlink-model"),
            model::InferenceEngineId::ollama(),
            model::ImportedContentItem::new(
                model::ExternalContentItemId::new("symlink-source"),
                "/var/lib/daia/content/linked.gguf",
            ),
        ));
        appliance_state
            .write_to_root(&root)
            .expect("persist test state");

        let mut executor = RecordingExecutor::default();

        let result = super::resume_model_realization_with("symlink-model", &root, &mut executor);

        assert!(result.is_err(), "symlink escape must be rejected");
        assert!(
            executor.names.is_empty(),
            "executor must not run for escaping symlink"
        );
        assert_eq!(
            std::fs::read(&outside).expect("read outside content"),
            b"outside-content"
        );
    }

    #[test]
    fn refuses_recovery_for_path_traversal() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let root = temporary.path().join("appliance");
        let outside = root.join("var/lib/daia/outside.gguf");

        std::fs::create_dir_all(root.join("var/lib/daia/content"))
            .expect("create content directory");

        std::fs::create_dir_all(outside.parent().unwrap()).expect("create test directory");
        std::fs::write(&outside, b"outside-content").expect("write outside content");

        let mut appliance_state = state::ApplianceState::new("desktop");
        appliance_state.record_model_realization(model::ModelRealization::new(
            model::ModelRealizationId::new("traversal-model"),
            model::InferenceEngineId::ollama(),
            model::ImportedContentItem::new(
                model::ExternalContentItemId::new("traversal-source"),
                "/var/lib/daia/content/../outside.gguf",
            ),
        ));
        appliance_state
            .write_to_root(&root)
            .expect("persist test state");

        let mut executor = RecordingExecutor::default();

        let result = super::resume_model_realization_with("traversal-model", &root, &mut executor);

        assert!(result.is_err(), "path traversal must be rejected");
        assert!(
            executor.names.is_empty(),
            "executor must not run for unsafe paths"
        );
        assert_eq!(
            std::fs::read(&outside).expect("read outside content"),
            b"outside-content"
        );
    }

    #[test]
    fn refuses_recovery_for_invalid_gguf() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let root = temporary.path().join("appliance");
        let content = root.join("var/lib/daia/content/invalid.gguf");

        std::fs::create_dir_all(content.parent().unwrap()).expect("create content directory");
        std::fs::write(&content, b"not-a-gguf-model").expect("write invalid GGUF");

        let mut appliance_state = state::ApplianceState::new("desktop");
        appliance_state.record_model_realization(model::ModelRealization::new(
            model::ModelRealizationId::new("invalid-gguf-model"),
            model::InferenceEngineId::ollama(),
            model::ImportedContentItem::new(
                model::ExternalContentItemId::new("invalid-gguf-source"),
                "/var/lib/daia/content/invalid.gguf",
            ),
        ));
        appliance_state
            .write_to_root(&root)
            .expect("persist test state");

        let mut executor = RecordingExecutor::default();

        let result =
            super::resume_model_realization_with("invalid-gguf-model", &root, &mut executor);

        assert!(result.is_err(), "invalid GGUF must be rejected");
        assert!(
            executor.names.is_empty(),
            "executor must not run for invalid GGUF"
        );
    }

    #[test]
    fn refuses_recovery_when_imported_content_is_missing() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let root = temporary.path().join("appliance");

        let mut appliance_state = state::ApplianceState::new("desktop");
        appliance_state.record_model_realization(model::ModelRealization::new(
            model::ModelRealizationId::new("missing-model"),
            model::InferenceEngineId::ollama(),
            model::ImportedContentItem::new(
                model::ExternalContentItemId::new("missing-source"),
                "/var/lib/daia/content/missing.gguf",
            ),
        ));
        appliance_state
            .write_to_root(&root)
            .expect("persist existing state");

        let mut executor = RecordingExecutor::default();

        let result = super::resume_model_realization_with("missing-model", &root, &mut executor);

        assert!(result.is_err(), "missing content must prevent recovery");
        assert!(
            executor.names.is_empty(),
            "Ollama executor must not run when content is missing"
        );

        let persisted = state::ApplianceState::read_from_root(&root).expect("read state");
        assert_eq!(persisted.model_realizations().len(), 1);
    }

    #[test]
    fn failed_recovery_preserves_imported_model_for_retry() {
        struct FailingExecutor;

        impl engine::ModelRealizationExecutor for FailingExecutor {
            type Error = String;

            fn execute(
                &mut self,
                _realization: &model::ModelRealization,
            ) -> Result<(), Self::Error> {
                Err("simulated Ollama registration failure".to_owned())
            }
        }

        let temporary = tempfile::tempdir().expect("temporary directory");
        let root = temporary.path().join("appliance");
        let content = root.join("var/lib/daia/content/model.gguf");

        std::fs::create_dir_all(content.parent().unwrap()).expect("create content directory");

        let mut bytes = Vec::from(*b"GGUF");
        bytes.extend_from_slice(&3_u32.to_le_bytes());
        bytes.extend_from_slice(&0_u64.to_le_bytes());
        bytes.extend_from_slice(&1_u64.to_le_bytes());

        let key = b"general.architecture";
        bytes.extend_from_slice(&(key.len() as u64).to_le_bytes());
        bytes.extend_from_slice(key);
        bytes.extend_from_slice(&8_u32.to_le_bytes());

        let value = b"llama";
        bytes.extend_from_slice(&(value.len() as u64).to_le_bytes());
        bytes.extend_from_slice(value);

        std::fs::write(&content, &bytes).expect("write GGUF fixture");

        let mut appliance_state = state::ApplianceState::new("desktop");
        appliance_state.record_model_realization(model::ModelRealization::new(
            model::ModelRealizationId::new("retry-model"),
            model::InferenceEngineId::ollama(),
            model::ImportedContentItem::new(
                model::ExternalContentItemId::new("existing-source"),
                "/var/lib/daia/content/model.gguf",
            ),
        ));
        appliance_state
            .write_to_root(&root)
            .expect("persist model state");

        let result =
            super::resume_model_realization_with("retry-model", &root, &mut FailingExecutor);

        assert!(result.is_err(), "executor failure must propagate");
        assert_eq!(
            std::fs::read(&content).expect("read retained GGUF"),
            bytes,
            "imported content must remain unchanged"
        );

        let persisted = state::ApplianceState::read_from_root(&root).expect("read retained state");

        assert_eq!(
            persisted.model_realizations().len(),
            1,
            "desired model state must remain available for retry"
        );

        let mut retry_executor = RecordingExecutor::default();
        super::resume_model_realization_with("retry-model", &root, &mut retry_executor)
            .expect("retry should succeed");

        assert_eq!(retry_executor.names, vec!["retry-model"]);
    }

    #[test]
    fn resumes_existing_model_without_reimporting_content() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let root = temporary.path().join("appliance");
        let content = root.join("var/lib/daia/content/model.gguf");

        std::fs::create_dir_all(content.parent().unwrap()).expect("create content directory");
        std::fs::write(&content, {
            let mut bytes = Vec::from(*b"GGUF");
            bytes.extend_from_slice(&3_u32.to_le_bytes());
            bytes.extend_from_slice(&0_u64.to_le_bytes());
            bytes.extend_from_slice(&1_u64.to_le_bytes());

            let key = b"general.architecture";
            bytes.extend_from_slice(&(key.len() as u64).to_le_bytes());
            bytes.extend_from_slice(key);
            bytes.extend_from_slice(&8_u32.to_le_bytes());

            let value = b"llama";
            bytes.extend_from_slice(&(value.len() as u64).to_le_bytes());
            bytes.extend_from_slice(value);
            bytes
        })
        .expect("write existing content");

        let mut appliance_state = state::ApplianceState::new("desktop");
        appliance_state.record_model_realization(model::ModelRealization::new(
            model::ModelRealizationId::new("recovery-model"),
            model::InferenceEngineId::ollama(),
            model::ImportedContentItem::new(
                model::ExternalContentItemId::new("existing-source"),
                "/var/lib/daia/content/model.gguf",
            ),
        ));
        appliance_state
            .write_to_root(&root)
            .expect("persist existing state");

        let original_content = std::fs::read(&content).expect("read content");
        let mut executor = RecordingExecutor::default();

        super::resume_model_realization_with("recovery-model", &root, &mut executor)
            .expect("existing model should resume");

        assert_eq!(executor.names, vec!["recovery-model"]);
        assert_eq!(
            std::fs::read(&content).expect("read content after recovery"),
            original_content,
        );

        let persisted = state::ApplianceState::read_from_root(&root).expect("read state");
        assert_eq!(persisted.model_realizations().len(), 1);
    }
}

#[cfg(test)]
mod model_realization_helper_tests {
    use super::realize_models_with;
    use engine::ModelRealizationExecutor;
    use model::{
        ExternalContentItemId, ImportedContentItem, InferenceEngineId, ModelRealization,
        ModelRealizationId,
    };

    struct FailingExecutor {
        calls: usize,
    }

    impl ModelRealizationExecutor for FailingExecutor {
        type Error = &'static str;

        fn execute(&mut self, _: &ModelRealization) -> Result<(), Self::Error> {
            self.calls += 1;
            Err("realization failed")
        }
    }

    #[test]
    fn realization_helper_preserves_executor_failure() {
        let realization = ModelRealization::new(
            ModelRealizationId::new("model"),
            InferenceEngineId::ollama(),
            ImportedContentItem::new(
                ExternalContentItemId::new("source-model"),
                "/var/lib/daia/content/model.gguf",
            ),
        );

        let mut executor = FailingExecutor { calls: 0 };

        assert_eq!(
            realize_models_with(&[realization], &mut executor),
            Err("realization failed")
        );
        assert_eq!(executor.calls, 1);
    }

    #[test]
    fn empty_realizations_do_not_invoke_executor() {
        let mut executor = FailingExecutor { calls: 0 };

        assert_eq!(realize_models_with(&[], &mut executor), Ok(()));
        assert_eq!(executor.calls, 0);
    }
}

#[cfg(test)]
mod model_directory_discovery_tests {
    #[test]
    fn model_discovery_report_preserves_valid_models_and_errors() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let valid_path = directory.path().join("valid.gguf");
        let invalid_path = directory.path().join("broken.gguf");

        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"GGUF");
        bytes.extend_from_slice(&3_u32.to_le_bytes());
        bytes.extend_from_slice(&0_u64.to_le_bytes());
        bytes.extend_from_slice(&1_u64.to_le_bytes());

        let key = b"general.architecture";
        bytes.extend_from_slice(&(key.len() as u64).to_le_bytes());
        bytes.extend_from_slice(key);
        bytes.extend_from_slice(&8_u32.to_le_bytes());

        let architecture = b"llama";
        bytes.extend_from_slice(&(architecture.len() as u64).to_le_bytes());
        bytes.extend_from_slice(architecture);

        std::fs::write(&valid_path, bytes).expect("write valid model");
        std::fs::write(&invalid_path, b"INVALID").expect("write invalid model");
        std::fs::write(directory.path().join("readme.txt"), b"Documentation")
            .expect("write unrelated file");

        let report = super::discover_models_with_report(directory.path())
            .expect("directory discovery should succeed");

        assert_eq!(report.models.len(), 1);
        assert_eq!(report.models[0].item.path(), valid_path.as_path());
        assert_eq!(report.models[0].metadata.architecture(), "llama");

        assert_eq!(report.issues.len(), 1);
        assert_eq!(report.issues[0].path, invalid_path);
        assert!(!report.issues[0].message.is_empty());
    }

    #[test]
    fn model_discovery_reports_malformed_file_in_mixed_directory() {
        let directory = tempfile::tempdir().expect("temporary directory");

        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"GGUF");
        bytes.extend_from_slice(&3_u32.to_le_bytes());
        bytes.extend_from_slice(&0_u64.to_le_bytes());
        bytes.extend_from_slice(&1_u64.to_le_bytes());

        let key = b"general.architecture";
        bytes.extend_from_slice(&(key.len() as u64).to_le_bytes());
        bytes.extend_from_slice(key);
        bytes.extend_from_slice(&8_u32.to_le_bytes());

        let architecture = b"llama";
        bytes.extend_from_slice(&(architecture.len() as u64).to_le_bytes());
        bytes.extend_from_slice(architecture);

        std::fs::write(directory.path().join("valid.gguf"), bytes).expect("write valid GGUF");
        std::fs::write(directory.path().join("broken.gguf"), b"INVALID")
            .expect("write malformed GGUF");

        let result = super::discover_models_in_directory(directory.path());
        assert!(result.is_err(), "malformed model should be reported");
    }

    #[test]
    fn model_discovery_recognizes_valid_gguf() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let model_path = directory.path().join("test-model.gguf");

        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"GGUF");
        bytes.extend_from_slice(&3_u32.to_le_bytes());
        bytes.extend_from_slice(&0_u64.to_le_bytes());
        bytes.extend_from_slice(&1_u64.to_le_bytes());

        let key = b"general.architecture";
        bytes.extend_from_slice(&(key.len() as u64).to_le_bytes());
        bytes.extend_from_slice(key);
        bytes.extend_from_slice(&8_u32.to_le_bytes());

        let architecture = b"llama";
        bytes.extend_from_slice(&(architecture.len() as u64).to_le_bytes());
        bytes.extend_from_slice(architecture);

        std::fs::write(&model_path, bytes).expect("write valid GGUF fixture");

        let models = super::discover_models_in_directory(directory.path())
            .expect("valid GGUF discovery should succeed");

        assert_eq!(models.len(), 1);
        assert_eq!(models[0].0.path(), model_path.as_path());
        assert_eq!(models[0].1.version(), 3);
        assert_eq!(models[0].1.architecture(), "llama");
    }

    #[test]
    fn model_discovery_rejects_malformed_gguf() {
        let directory = tempfile::tempdir().expect("temporary directory");

        std::fs::write(
            directory.path().join("broken.gguf"),
            b"NOT A VALID GGUF MODEL",
        )
        .expect("write malformed model");

        let result = super::discover_models_in_directory(directory.path());

        assert!(result.is_err(), "malformed GGUF must not be accepted");
    }

    #[test]
    fn model_discovery_ignores_non_model_files() {
        let directory = tempfile::tempdir().expect("temporary directory");

        std::fs::write(directory.path().join("readme.txt"), b"Documentation")
            .expect("write documentation");
        std::fs::write(directory.path().join("notes.json"), b"{}").expect("write JSON");

        let models = super::discover_models_in_directory(directory.path())
            .expect("non-model files should be ignored");

        assert!(models.is_empty());
    }

    #[test]
    fn model_discovery_returns_empty_for_empty_directory() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let models = super::discover_models_in_directory(directory.path())
            .expect("empty directory discovery should succeed");

        assert!(models.is_empty());
    }
}
