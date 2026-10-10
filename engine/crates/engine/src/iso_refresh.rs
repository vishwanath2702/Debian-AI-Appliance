//! Validation boundary for refreshing an ISO from prepared artifacts.

use std::io;
use std::path::{Path, PathBuf};

/// Inputs for a refresh that must not mutate the original build artifacts.
#[derive(Clone, Debug)]
pub struct IsoRefreshInputs {
    pub rootfs: PathBuf,
    pub payload: PathBuf,
    pub workspace: PathBuf,
    pub output_iso: PathBuf,
}

impl IsoRefreshInputs {
    /// Validates the filesystem boundary before refresh operations begin.
    pub fn validate(&self) -> io::Result<()> {
        let rootfs = existing_directory(&self.rootfs)?;
        let payload = existing_directory(&self.payload)?;

        if overlaps(&rootfs, &payload) {
            return Err(invalid("refresh input directories must not overlap"));
        }

        if self.workspace.exists() || self.workspace.is_symlink() {
            return Err(invalid("refresh workspace must not already exist"));
        }

        if self.output_iso.exists() || self.output_iso.is_symlink() {
            return Err(invalid("refresh output ISO must not already exist"));
        }

        let workspace = prospective_path(&self.workspace)?;
        let output_iso = prospective_path(&self.output_iso)?;

        if workspace == output_iso {
            return Err(invalid("refresh workspace and output ISO must differ"));
        }

        if overlaps(&workspace, &rootfs)
            || overlaps(&workspace, &payload)
            || overlaps(&workspace, &output_iso)
            || overlaps(&output_iso, &rootfs)
            || overlaps(&output_iso, &payload)
        {
            return Err(invalid("refresh paths overlap protected artifacts"));
        }

        Ok(())
    }

    /// Exclusively creates a new candidate workspace after validation.
    pub fn create_workspace(&self) -> io::Result<()> {
        self.validate()?;
        std::fs::create_dir(&self.workspace)
    }

    /// Packages an already staged candidate without bootstrapping Debian.
    ///
    /// This does not stage inputs or deploy refreshed artifacts.
    pub fn package_candidate(&self, source_iso: &Path) -> io::Result<()> {
        use crate::IsoBackend;
        use crate::backend::BuildBackend;
        use inspector::DaiaLiveIsoInspector;
        use model::{Capability, Plan, ProviderId};

        let rootfs = self.workspace.join("rootfs");
        let payload = self.workspace.join("payload");
        let iso_work = self.workspace.join("iso-build");

        let required_artifacts = [
            self.workspace.join(".daia-stage-complete"),
            self.workspace.join(".daia-deploy-complete"),
            rootfs.join("usr/bin/daia"),
            rootfs.join("usr/bin/daia-tui"),
            payload.join("etc/systemd/system/daia-firstboot.service"),
        ];

        if !self.workspace.is_dir()
            || self.workspace.is_symlink()
            || !rootfs.is_dir()
            || rootfs.is_symlink()
            || !payload.is_dir()
            || payload.is_symlink()
            || required_artifacts.iter().any(|path| {
                ensure_candidate_destination(&self.workspace, path).is_err()
                    || ensure_candidate_file(path).is_err()
            })
            || iso_work.exists()
            || iso_work.is_symlink()
            || self.output_iso.exists()
            || self.output_iso.is_symlink()
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "refresh candidate is missing, already packaged, or output exists",
            ));
        }

        if source_iso.is_symlink() || !source_iso.is_file() {
            return Err(invalid("refresh source ISO must be a regular file"));
        }

        let canonical_source = source_iso.canonicalize()?;
        let canonical_workspace = self.workspace.canonicalize()?;
        let prospective_output = prospective_path(&self.output_iso)?;

        if canonical_source.starts_with(&canonical_workspace)
            || canonical_source == prospective_output
        {
            return Err(invalid(
                "refresh source ISO must be outside the candidate workspace and output",
            ));
        }

        #[cfg(unix)]
        if self.output_iso.exists() {
            use std::os::unix::fs::MetadataExt;

            let source_metadata = std::fs::metadata(source_iso)?;
            let output_metadata = std::fs::metadata(&self.output_iso)?;

            if source_metadata.dev() == output_metadata.dev()
                && source_metadata.ino() == output_metadata.ino()
            {
                return Err(invalid(
                    "refresh source ISO and output refer to the same file",
                ));
            }
        }

        let plan = Plan {
            capability: Capability::new("iso-refresh"),
            provider: ProviderId::new("iso-refresh"),
            steps: Vec::new(),
        };

        let mut backend = IsoBackend::new(&rootfs, source_iso, &iso_work, &self.output_iso)
            .with_daia_payload_directory(&payload)
            .with_iso_inspector(DaiaLiveIsoInspector::new(&rootfs));

        backend
            .build(&plan)
            .map_err(|error| io::Error::other(error.to_string()))
    }

    /// Replaces installer binaries and firstboot service in the staged candidate.
    pub fn deploy_artifacts(
        &self,
        cli_binary: &Path,
        tui_binary: &Path,
        firstboot_service: &Path,
    ) -> io::Result<()> {
        ensure_candidate_file(&self.workspace.join(".daia-stage-complete"))?;

        let deployment_marker = self.workspace.join(".daia-deploy-complete");
        if deployment_marker.exists() || deployment_marker.is_symlink() {
            return Err(invalid("refresh candidate has already been deployed"));
        }

        let bundled_repository = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../registry/content-repositories/bundled-models.yaml");

        let replacements = [
            (cli_binary, self.workspace.join("rootfs/usr/bin/daia")),
            (tui_binary, self.workspace.join("rootfs/usr/bin/daia-tui")),
            (
                firstboot_service,
                self.workspace
                    .join("payload/etc/systemd/system/daia-firstboot.service"),
            ),
            (
                &bundled_repository,
                self.workspace
                    .join("rootfs/usr/share/daia/content-repositories/bundled-models.yaml"),
            ),
        ];

        for (source, destination) in &replacements {
            ensure_candidate_destination(&self.workspace, &destination)?;
            ensure_candidate_file(source)?;
        }

        for (source, destination) in &replacements {
            replace_candidate_file(&self.workspace, source, &destination)?;
        }

        std::fs::write(
            self.workspace.join(".daia-deploy-complete"),
            b"deployment completed\n",
        )?;

        Ok(())
    }

    /// Stages prepared inputs using the system rsync executable.
    pub fn stage(&self) -> io::Result<()> {
        self.stage_with(Path::new("/usr/bin/rsync"))
    }

    /// Stages prepared inputs into an exclusive candidate workspace.
    fn stage_with(&self, rsync: &Path) -> io::Result<()> {
        self.validate()?;
        ensure_no_source_mounts(&self.rootfs, &self.payload)?;
        self.create_workspace()?;

        let rootfs_destination = self.workspace.join("rootfs");
        let payload_destination = self.workspace.join("payload");

        std::fs::create_dir(&rootfs_destination)?;
        std::fs::create_dir(&payload_destination)?;

        execute_rsync(rsync, &self.rootfs, &rootfs_destination)?;
        execute_rsync(rsync, &self.payload, &payload_destination)?;

        let bundled_repository =
            rootfs_destination.join("usr/share/daia/content-repositories/bundled-models.yaml");
        let repository_directory = bundled_repository
            .parent()
            .ok_or_else(|| invalid("bundled repository destination has no parent"))?;

        let canonical_repository_directory = repository_directory.canonicalize()?;
        if !canonical_repository_directory.starts_with(rootfs_destination.canonicalize()?)
            || bundled_repository.exists()
            || bundled_repository.is_symlink()
        {
            return Err(invalid("unsafe bundled repository staging destination"));
        }

        std::fs::write(&bundled_repository, b"pending candidate deployment\n")?;

        std::fs::write(
            self.workspace.join(".daia-stage-complete"),
            b"staging completed\n",
        )?;

        Ok(())
    }
}

/// Rejects active mountpoints within either prepared input tree.
#[cfg(target_os = "linux")]
fn ensure_no_source_mounts(rootfs: &Path, payload: &Path) -> io::Result<()> {
    let rootfs = rootfs.canonicalize()?;
    let payload = payload.canonicalize()?;
    let mountinfo = std::fs::read_to_string("/proc/self/mountinfo")?;

    for line in mountinfo.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();

        if fields.len() < 5 {
            return Err(invalid("malformed mountinfo entry"));
        }

        let mountpoint = decode_mount_path(fields[4])?;

        if mountpoint.starts_with(&rootfs) || mountpoint.starts_with(&payload) {
            return Err(invalid("prepared input contains an active mountpoint"));
        }
    }

    Ok(())
}

#[cfg(target_os = "linux")]
fn decode_mount_path(encoded: &str) -> io::Result<PathBuf> {
    let mut decoded = Vec::new();
    let bytes = encoded.as_bytes();
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index] == b'\\' {
            if index + 3 >= bytes.len()
                || !bytes[index + 1..index + 4]
                    .iter()
                    .all(|byte| (b'0'..=b'7').contains(byte))
            {
                return Err(invalid("invalid mountinfo path escape"));
            }

            let value = ((bytes[index + 1] - b'0') << 6)
                | ((bytes[index + 2] - b'0') << 3)
                | (bytes[index + 3] - b'0');

            decoded.push(value);
            index += 4;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }

    use std::os::unix::ffi::OsStringExt;
    Ok(PathBuf::from(std::ffi::OsString::from_vec(decoded)))
}

#[cfg(not(target_os = "linux"))]
fn ensure_no_source_mounts(_rootfs: &Path, _payload: &Path) -> io::Result<()> {
    Err(invalid("ISO refresh staging requires Linux"))
}

/// Constructs the filesystem-preserving rsync argument list.
fn rsync_arguments(source: &Path, destination: &Path) -> Vec<String> {
    vec![
        "--archive".to_owned(),
        "--hard-links".to_owned(),
        "--acls".to_owned(),
        "--xattrs".to_owned(),
        "--numeric-ids".to_owned(),
        "--one-file-system".to_owned(),
        "--".to_owned(),
        format!("{}/", source.display()),
        format!("{}/", destination.display()),
    ]
}

/// Executes one filesystem staging command and checks its exit status.
fn execute_rsync(executable: &Path, source: &Path, destination: &Path) -> io::Result<()> {
    let arguments = rsync_arguments(source, destination);

    let status = std::process::Command::new(executable)
        .args(&arguments)
        .status()?;

    if !status.success() {
        return Err(io::Error::other(format!(
            "filesystem staging command failed with status {status}"
        )));
    }

    Ok(())
}

/// Verifies that a replacement remains within the candidate workspace.
fn ensure_candidate_destination(workspace: &Path, destination: &Path) -> io::Result<()> {
    let workspace = existing_directory(workspace)?;
    let parent = destination
        .parent()
        .ok_or_else(|| invalid("candidate replacement requires a parent"))?
        .canonicalize()?;

    if !parent.starts_with(&workspace) {
        return Err(invalid("candidate replacement escapes workspace"));
    }

    ensure_candidate_file(destination)
}

/// Rejects symlinks and non-regular files at a candidate replacement path.
fn ensure_candidate_file(destination: &Path) -> io::Result<()> {
    let metadata = std::fs::symlink_metadata(destination)?;

    if !metadata.file_type().is_file() {
        return Err(invalid("candidate replacement must be a regular file"));
    }

    Ok(())
}

/// Replaces an existing regular file inside the candidate workspace.
fn replace_candidate_file(workspace: &Path, source: &Path, destination: &Path) -> io::Result<()> {
    ensure_candidate_file(source)?;
    ensure_candidate_destination(workspace, destination)?;

    let parent = destination
        .parent()
        .ok_or_else(|| invalid("candidate replacement requires a parent"))?;

    let temporary = tempfile::NamedTempFile::new_in(parent)?;
    std::fs::copy(source, temporary.path())?;

    let permissions = std::fs::metadata(destination)?.permissions();
    std::fs::set_permissions(temporary.path(), permissions)?;

    temporary
        .persist(destination)
        .map_err(|error| error.error)?;

    Ok(())
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

fn existing_directory(path: &Path) -> io::Result<PathBuf> {
    if path.is_symlink() || !path.is_dir() {
        return Err(invalid(
            "refresh input must be an existing non-symlink directory",
        ));
    }

    path.canonicalize()
}

fn prospective_path(path: &Path) -> io::Result<PathBuf> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .ok_or_else(|| invalid("refresh path requires a parent directory"))?;

    let filename = path
        .file_name()
        .ok_or_else(|| invalid("refresh path requires a filename"))?;

    if filename == "." || filename == ".." {
        return Err(invalid("refresh path must name a new entry"));
    }

    if parent.ancestors().any(|ancestor| ancestor.is_symlink()) {
        return Err(invalid("refresh path must not contain symlinked ancestors"));
    }

    if !parent.is_dir() {
        return Err(invalid("refresh path parent must be an existing directory"));
    }

    let parent = parent.canonicalize()?;
    Ok(parent.join(filename))
}

fn overlaps(left: &Path, right: &Path) -> bool {
    left.starts_with(right) || right.starts_with(left)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[cfg(unix)]
    #[test]
    fn successful_staging_writes_completion_marker() {
        let temp = tempfile::tempdir().unwrap();
        let rootfs = temp.path().join("source-rootfs");
        let payload = temp.path().join("source-payload");
        std::fs::create_dir(&rootfs).unwrap();
        std::fs::create_dir(&payload).unwrap();

        let inputs = IsoRefreshInputs {
            rootfs,
            payload,
            workspace: temp.path().join("candidate"),
            output_iso: temp.path().join("candidate.iso"),
        };

        let repository_directory = inputs.rootfs.join("usr/share/daia/content-repositories");
        std::fs::create_dir_all(&repository_directory).unwrap();

        let mock_rsync = temp.path().join("mock-rsync.sh");
        std::fs::write(
            &mock_rsync,
            b"#!/bin/sh\nprevious=\nfor argument do\n    source_dir=\"$previous\"\n    destination_dir=\"$argument\"\n    previous=\"$argument\"\ndone\ncp -a -- \"$source_dir/.\" \"$destination_dir/\"\n",
        )
        .unwrap();

        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&mock_rsync, std::fs::Permissions::from_mode(0o755)).unwrap();

        inputs.stage_with(&mock_rsync).unwrap();

        let candidate_repository = inputs
            .workspace
            .join("rootfs/usr/share/daia/content-repositories/bundled-models.yaml");
        assert_eq!(
            std::fs::read(candidate_repository).unwrap(),
            b"pending candidate deployment\n"
        );
        assert!(!repository_directory.join("bundled-models.yaml").exists());

        assert!(inputs.workspace.join(".daia-stage-complete").is_file());
    }

    #[cfg(unix)]
    #[test]
    fn failed_staging_does_not_write_completion_marker() {
        let temp = tempfile::tempdir().unwrap();
        let rootfs = temp.path().join("source-rootfs");
        let payload = temp.path().join("source-payload");
        std::fs::create_dir(&rootfs).unwrap();
        std::fs::create_dir(&payload).unwrap();

        let inputs = IsoRefreshInputs {
            rootfs,
            payload,
            workspace: temp.path().join("candidate"),
            output_iso: temp.path().join("candidate.iso"),
        };

        assert!(inputs.stage_with(Path::new("/bin/false")).is_err());
        assert!(inputs.workspace.is_dir());
        assert!(!inputs.workspace.join(".daia-stage-complete").exists());
    }

    #[test]
    fn deployment_rejects_completed_candidate_without_modification() {
        let temp = tempfile::tempdir().unwrap();
        let rootfs = temp.path().join("source-rootfs");
        let payload = temp.path().join("source-payload");
        let workspace = temp.path().join("candidate");

        std::fs::create_dir(&rootfs).unwrap();
        std::fs::create_dir(&payload).unwrap();
        std::fs::create_dir(&workspace).unwrap();

        std::fs::write(workspace.join(".daia-stage-complete"), b"staged").unwrap();
        std::fs::write(workspace.join(".daia-deploy-complete"), b"completed").unwrap();

        let inputs = IsoRefreshInputs {
            rootfs,
            payload,
            workspace: workspace.clone(),
            output_iso: temp.path().join("candidate.iso"),
        };

        let missing = temp.path().join("missing-artifact");
        let result = inputs.deploy_artifacts(&missing, &missing, &missing);

        assert!(result.is_err());
        assert_eq!(
            std::fs::read(workspace.join(".daia-deploy-complete")).unwrap(),
            b"completed"
        );
        assert!(!inputs.output_iso.exists());
    }

    #[test]
    fn deployment_rejects_candidate_without_staging_marker() {
        let temp = tempfile::tempdir().unwrap();
        let rootfs = temp.path().join("source-rootfs");
        let payload = temp.path().join("source-payload");
        let workspace = temp.path().join("candidate");

        std::fs::create_dir(&rootfs).unwrap();
        std::fs::create_dir(&payload).unwrap();
        std::fs::create_dir(&workspace).unwrap();

        let inputs = IsoRefreshInputs {
            rootfs,
            payload,
            workspace: workspace.clone(),
            output_iso: temp.path().join("candidate.iso"),
        };

        let missing = temp.path().join("missing-artifact");
        let result = inputs.deploy_artifacts(&missing, &missing, &missing);

        assert!(result.is_err());
        assert!(!workspace.join(".daia-stage-complete").exists());
        assert!(!workspace.join(".daia-deploy-complete").exists());
    }

    #[test]
    fn package_candidate_rejects_invalid_source_iso() {
        let temp = tempfile::tempdir().unwrap();
        let rootfs = temp.path().join("source-rootfs");
        let payload = temp.path().join("source-payload");
        let workspace = temp.path().join("candidate");

        std::fs::create_dir(&rootfs).unwrap();
        std::fs::create_dir(&payload).unwrap();
        std::fs::create_dir(&workspace).unwrap();

        for marker in [".daia-stage-complete", ".daia-deploy-complete"] {
            std::fs::write(workspace.join(marker), b"completed").unwrap();
        }

        for artifact in [
            workspace.join("rootfs/usr/bin/daia"),
            workspace.join("rootfs/usr/bin/daia-tui"),
            workspace.join("payload/etc/systemd/system/daia-firstboot.service"),
        ] {
            std::fs::create_dir_all(artifact.parent().unwrap()).unwrap();
            std::fs::write(artifact, b"present").unwrap();
        }

        let inputs = IsoRefreshInputs {
            rootfs,
            payload,
            workspace: workspace.clone(),
            output_iso: temp.path().join("candidate.iso"),
        };

        let missing = temp.path().join("missing.iso");
        assert!(inputs.package_candidate(&missing).is_err());

        let directory = temp.path().join("directory.iso");
        std::fs::create_dir(&directory).unwrap();
        assert!(inputs.package_candidate(&directory).is_err());

        let inside = workspace.join("source.iso");
        std::fs::write(&inside, b"source").unwrap();
        assert!(inputs.package_candidate(&inside).is_err());

        assert!(!workspace.join("iso-build").exists());
        assert!(!inputs.output_iso.exists());
    }

    #[test]
    fn package_candidate_rejects_missing_deployment_marker() {
        let temp = tempfile::tempdir().unwrap();
        let rootfs = temp.path().join("source-rootfs");
        let payload = temp.path().join("source-payload");
        let workspace = temp.path().join("candidate");
        let output_iso = temp.path().join("candidate.iso");

        std::fs::create_dir(&rootfs).unwrap();
        std::fs::create_dir(&payload).unwrap();

        let cli = workspace.join("rootfs/usr/bin/daia");
        let tui = workspace.join("rootfs/usr/bin/daia-tui");
        let service = workspace.join("payload/etc/systemd/system/daia-firstboot.service");

        for artifact in [&cli, &tui, &service] {
            std::fs::create_dir_all(artifact.parent().unwrap()).unwrap();
            std::fs::write(artifact, b"present").unwrap();
        }

        std::fs::write(
            workspace.join(".daia-stage-complete"),
            b"staging completed\n",
        )
        .unwrap();

        let inputs = IsoRefreshInputs {
            rootfs,
            payload,
            workspace: workspace.clone(),
            output_iso: output_iso.clone(),
        };

        assert!(
            inputs
                .package_candidate(&temp.path().join("source.iso"))
                .is_err()
        );

        assert!(!workspace.join("iso-build").exists());
        assert!(!output_iso.exists());
    }

    #[test]
    fn package_candidate_rejects_missing_artifacts() {
        let temp = tempfile::tempdir().unwrap();
        let rootfs = temp.path().join("source-rootfs");
        let payload = temp.path().join("source-payload");
        let workspace = temp.path().join("candidate");
        let output_iso = temp.path().join("candidate.iso");

        std::fs::create_dir(&rootfs).unwrap();
        std::fs::create_dir(&payload).unwrap();
        std::fs::create_dir(&workspace).unwrap();
        std::fs::create_dir(workspace.join("rootfs")).unwrap();
        std::fs::create_dir(workspace.join("payload")).unwrap();

        let inputs = IsoRefreshInputs {
            rootfs,
            payload,
            workspace: workspace.clone(),
            output_iso: output_iso.clone(),
        };

        let result = inputs.package_candidate(&temp.path().join("source.iso"));

        assert!(result.is_err());
        assert!(!workspace.join("iso-build").exists());
        assert!(!output_iso.exists());
    }

    #[test]
    fn rsync_preserves_filesystem_metadata_without_deletion() {
        let args = rsync_arguments(
            Path::new("/prepared/rootfs"),
            Path::new("/candidate/rootfs"),
        );

        assert_eq!(
            args,
            vec![
                "--archive",
                "--hard-links",
                "--acls",
                "--xattrs",
                "--numeric-ids",
                "--one-file-system",
                "--",
                "/prepared/rootfs/",
                "/candidate/rootfs/",
            ]
        );

        assert!(!args.iter().any(|arg| arg == "--delete"));
        assert!(!args.iter().any(|arg| arg == "--copy-links"));
        assert!(!args.iter().any(|arg| arg == "--inplace"));
    }

    #[cfg(unix)]
    #[test]
    fn staging_runner_accepts_successful_command() {
        execute_rsync(
            Path::new("/bin/true"),
            Path::new("/prepared/rootfs"),
            Path::new("/candidate/rootfs"),
        )
        .unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn staging_runner_rejects_failed_command() {
        let error = execute_rsync(
            Path::new("/bin/false"),
            Path::new("/prepared/rootfs"),
            Path::new("/candidate/rootfs"),
        )
        .unwrap_err();

        assert!(error.to_string().contains("failed with status"));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn decodes_mountinfo_escaped_paths() {
        assert_eq!(
            decode_mount_path("/tmp/with\\040space").unwrap(),
            PathBuf::from("/tmp/with space")
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn rejects_invalid_mountinfo_escape() {
        assert!(decode_mount_path("/tmp/bad\\xyz").is_err());
    }

    #[test]
    fn accepts_file_inside_candidate_workspace() {
        let temp = tempdir().unwrap();
        let workspace = temp.path().join("candidate");
        let destination_dir = workspace.join("rootfs/usr/bin");

        std::fs::create_dir_all(&destination_dir).unwrap();
        let destination = destination_dir.join("daia");
        std::fs::write(&destination, b"old").unwrap();

        ensure_candidate_destination(&workspace, &destination).unwrap();
    }

    #[test]
    fn rejects_file_outside_candidate_workspace() {
        let temp = tempdir().unwrap();
        let workspace = temp.path().join("candidate");
        std::fs::create_dir(&workspace).unwrap();

        let destination = temp.path().join("protected");
        std::fs::write(&destination, b"original").unwrap();

        assert!(ensure_candidate_destination(&workspace, &destination).is_err());
        assert_eq!(std::fs::read(&destination).unwrap(), b"original");
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlinked_candidate_parent_escape() {
        use std::os::unix::fs::symlink;

        let temp = tempdir().unwrap();
        let workspace = temp.path().join("candidate");
        let outside = temp.path().join("outside");

        std::fs::create_dir(&workspace).unwrap();
        std::fs::create_dir(&outside).unwrap();
        std::fs::write(outside.join("daia"), b"protected").unwrap();

        symlink(&outside, workspace.join("bin")).unwrap();

        assert!(ensure_candidate_destination(&workspace, &workspace.join("bin/daia")).is_err());
        assert_eq!(std::fs::read(outside.join("daia")).unwrap(), b"protected");
    }

    #[test]
    fn deploys_artifacts_only_to_candidate() {
        let temp = tempdir().unwrap();
        let rootfs = temp.path().join("original-rootfs");
        let payload = temp.path().join("original-payload");
        let workspace = temp.path().join("candidate");

        std::fs::create_dir(&rootfs).unwrap();
        std::fs::create_dir(&payload).unwrap();

        let cli_target = workspace.join("rootfs/usr/bin/daia");
        let tui_target = workspace.join("rootfs/usr/bin/daia-tui");
        let service_target = workspace.join("payload/etc/systemd/system/daia-firstboot.service");
        let repository_target =
            workspace.join("rootfs/usr/share/daia/content-repositories/bundled-models.yaml");

        for destination in [
            &cli_target,
            &tui_target,
            &service_target,
            &repository_target,
        ] {
            std::fs::create_dir_all(destination.parent().unwrap()).unwrap();
            std::fs::write(destination, b"old").unwrap();
        }

        let cli = temp.path().join("new-cli");
        let tui = temp.path().join("new-tui");
        let service = temp.path().join("new-service");

        std::fs::write(&cli, b"new-cli").unwrap();
        std::fs::write(&tui, b"new-tui").unwrap();
        std::fs::write(&service, b"new-service").unwrap();

        let inputs = IsoRefreshInputs {
            rootfs,
            payload,
            workspace,
            output_iso: temp.path().join("candidate.iso"),
        };

        std::fs::write(
            inputs.workspace.join(".daia-stage-complete"),
            b"staging completed\n",
        )
        .unwrap();

        inputs.deploy_artifacts(&cli, &tui, &service).unwrap();

        assert_eq!(std::fs::read(cli_target).unwrap(), b"new-cli");
        assert_eq!(std::fs::read(tui_target).unwrap(), b"new-tui");
        assert_eq!(std::fs::read(service_target).unwrap(), b"new-service");
        assert_eq!(
            std::fs::read(repository_target).unwrap(),
            std::fs::read(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../../registry/content-repositories/bundled-models.yaml")
            )
            .unwrap()
        );
        assert!(
            !inputs
                .rootfs
                .join("usr/share/daia/content-repositories/bundled-models.yaml")
                .exists()
        );
        assert!(
            !inputs
                .payload
                .join("usr/share/daia/content-repositories/bundled-models.yaml")
                .exists()
        );
        assert!(inputs.workspace.join(".daia-deploy-complete").is_file());
        assert!(!inputs.output_iso.exists());
    }

    #[cfg(unix)]
    #[test]
    fn replacement_preserves_candidate_executable_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempdir().unwrap();
        let workspace = temp.path().join("candidate");
        std::fs::create_dir(&workspace).unwrap();

        let original = temp.path().join("new-binary");
        let destination = workspace.join("daia");

        std::fs::write(&original, b"new").unwrap();
        std::fs::write(&destination, b"old").unwrap();

        std::fs::set_permissions(&destination, std::fs::Permissions::from_mode(0o755)).unwrap();

        replace_candidate_file(&workspace, &original, &destination).unwrap();

        let mode = std::fs::metadata(&destination)
            .unwrap()
            .permissions()
            .mode()
            & 0o777;

        assert_eq!(mode, 0o755);
        assert_eq!(std::fs::read(&destination).unwrap(), b"new");
    }

    #[test]
    fn replaces_only_candidate_file() {
        let temp = tempdir().unwrap();
        let workspace = temp.path().join("candidate");
        std::fs::create_dir(&workspace).unwrap();

        let original = temp.path().join("original");
        let destination = workspace.join("daia");

        std::fs::write(&original, b"new-binary").unwrap();
        std::fs::write(&destination, b"old-binary").unwrap();

        replace_candidate_file(&workspace, &original, &destination).unwrap();

        assert_eq!(std::fs::read(&destination).unwrap(), b"new-binary");
        assert_eq!(std::fs::read(&original).unwrap(), b"new-binary");
    }

    #[test]
    fn rejects_replacement_outside_candidate() {
        let temp = tempdir().unwrap();
        let workspace = temp.path().join("candidate");
        std::fs::create_dir(&workspace).unwrap();

        let original = temp.path().join("source");
        let protected = temp.path().join("protected");

        std::fs::write(&original, b"replacement").unwrap();
        std::fs::write(&protected, b"original").unwrap();

        assert!(replace_candidate_file(&workspace, &original, &protected).is_err());
        assert_eq!(std::fs::read(&protected).unwrap(), b"original");
    }

    #[test]
    fn accepts_regular_candidate_file() {
        let temp = tempdir().unwrap();
        let destination = temp.path().join("candidate");
        std::fs::write(&destination, b"old").unwrap();

        ensure_candidate_file(&destination).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlinked_candidate_file() {
        use std::os::unix::fs::symlink;

        let temp = tempdir().unwrap();
        let original = temp.path().join("original");
        let destination = temp.path().join("candidate");

        std::fs::write(&original, b"protected").unwrap();
        symlink(&original, &destination).unwrap();

        assert!(ensure_candidate_file(&destination).is_err());
        assert_eq!(std::fs::read(&original).unwrap(), b"protected");
    }

    #[test]
    fn rejects_directory_as_candidate_file() {
        let temp = tempdir().unwrap();
        let destination = temp.path().join("candidate");
        std::fs::create_dir(&destination).unwrap();

        assert!(ensure_candidate_file(&destination).is_err());
    }

    #[test]
    fn accepts_separate_candidate_paths() {
        let temp = tempdir().unwrap();
        let rootfs = temp.path().join("rootfs");
        let payload = temp.path().join("payload");
        std::fs::create_dir(&rootfs).unwrap();
        std::fs::create_dir(&payload).unwrap();

        let inputs = IsoRefreshInputs {
            rootfs,
            payload,
            workspace: temp.path().join("candidate"),
            output_iso: temp.path().join("candidate.iso"),
        };

        inputs.validate().unwrap();
    }

    #[test]
    fn rejects_identical_candidate_paths() {
        let temp = tempdir().unwrap();
        let rootfs = temp.path().join("rootfs");
        let payload = temp.path().join("payload");
        std::fs::create_dir(&rootfs).unwrap();
        std::fs::create_dir(&payload).unwrap();

        let candidate = temp.path().join("candidate");

        let inputs = IsoRefreshInputs {
            rootfs,
            payload,
            workspace: candidate.clone(),
            output_iso: candidate,
        };

        assert!(inputs.validate().is_err());
    }

    #[test]
    fn rejects_missing_candidate_parent() {
        let temp = tempdir().unwrap();
        let rootfs = temp.path().join("rootfs");
        let payload = temp.path().join("payload");
        std::fs::create_dir(&rootfs).unwrap();
        std::fs::create_dir(&payload).unwrap();

        let inputs = IsoRefreshInputs {
            rootfs,
            payload,
            workspace: temp.path().join("missing").join("candidate"),
            output_iso: temp.path().join("candidate.iso"),
        };

        assert!(inputs.validate().is_err());
    }

    #[test]
    fn creates_candidate_workspace_without_touching_inputs() {
        let temp = tempdir().unwrap();
        let rootfs = temp.path().join("rootfs");
        let payload = temp.path().join("payload");
        std::fs::create_dir(&rootfs).unwrap();
        std::fs::create_dir(&payload).unwrap();
        std::fs::write(rootfs.join("marker"), b"rootfs").unwrap();
        std::fs::write(payload.join("marker"), b"payload").unwrap();

        let inputs = IsoRefreshInputs {
            rootfs: rootfs.clone(),
            payload: payload.clone(),
            workspace: temp.path().join("candidate"),
            output_iso: temp.path().join("candidate.iso"),
        };

        inputs.create_workspace().unwrap();

        assert!(inputs.workspace.is_dir());
        assert_eq!(std::fs::read(rootfs.join("marker")).unwrap(), b"rootfs");
        assert_eq!(std::fs::read(payload.join("marker")).unwrap(), b"payload");
        assert!(!inputs.output_iso.exists());
    }

    #[test]
    fn existing_workspace_is_not_modified() {
        let temp = tempdir().unwrap();
        let rootfs = temp.path().join("rootfs");
        let payload = temp.path().join("payload");
        let workspace = temp.path().join("candidate");
        std::fs::create_dir(&rootfs).unwrap();
        std::fs::create_dir(&payload).unwrap();
        std::fs::create_dir(&workspace).unwrap();
        std::fs::write(workspace.join("marker"), b"preserve").unwrap();

        let inputs = IsoRefreshInputs {
            rootfs,
            payload,
            workspace: workspace.clone(),
            output_iso: temp.path().join("candidate.iso"),
        };

        assert!(inputs.create_workspace().is_err());
        assert_eq!(
            std::fs::read(workspace.join("marker")).unwrap(),
            b"preserve"
        );
    }

    #[cfg(unix)]
    #[test]
    fn stages_into_separate_candidate_directories() {
        let temp = tempdir().unwrap();
        let rootfs = temp.path().join("rootfs");
        let payload = temp.path().join("payload");
        std::fs::create_dir(&rootfs).unwrap();
        std::fs::create_dir(&payload).unwrap();

        let inputs = IsoRefreshInputs {
            rootfs,
            payload,
            workspace: temp.path().join("candidate"),
            output_iso: temp.path().join("candidate.iso"),
        };

        let repository_directory = inputs.rootfs.join("usr/share/daia/content-repositories");
        std::fs::create_dir_all(&repository_directory).unwrap();

        let mock_rsync = temp.path().join("mock-rsync.sh");
        std::fs::write(
            &mock_rsync,
            b"#!/bin/sh\nprevious=\nfor argument do\n    source_dir=\"$previous\"\n    destination_dir=\"$argument\"\n    previous=\"$argument\"\ndone\ncp -a -- \"$source_dir/.\" \"$destination_dir/\"\n",
        )
        .unwrap();

        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&mock_rsync, std::fs::Permissions::from_mode(0o755)).unwrap();

        inputs.stage_with(&mock_rsync).unwrap();

        let candidate_repository = inputs
            .workspace
            .join("rootfs/usr/share/daia/content-repositories/bundled-models.yaml");
        assert_eq!(
            std::fs::read(candidate_repository).unwrap(),
            b"pending candidate deployment\n"
        );
        assert!(!repository_directory.join("bundled-models.yaml").exists());

        assert!(inputs.workspace.join("rootfs").is_dir());
        assert!(inputs.workspace.join("payload").is_dir());
        assert!(!inputs.output_iso.exists());
    }

    #[cfg(unix)]
    #[test]
    fn failed_staging_preserves_original_inputs() {
        let temp = tempdir().unwrap();
        let rootfs = temp.path().join("rootfs");
        let payload = temp.path().join("payload");
        std::fs::create_dir(&rootfs).unwrap();
        std::fs::create_dir(&payload).unwrap();
        std::fs::write(rootfs.join("marker"), b"original-rootfs").unwrap();
        std::fs::write(payload.join("marker"), b"original-payload").unwrap();

        let inputs = IsoRefreshInputs {
            rootfs: rootfs.clone(),
            payload: payload.clone(),
            workspace: temp.path().join("candidate"),
            output_iso: temp.path().join("candidate.iso"),
        };

        assert!(inputs.stage_with(Path::new("/bin/false")).is_err());

        assert_eq!(
            std::fs::read(rootfs.join("marker")).unwrap(),
            b"original-rootfs"
        );
        assert_eq!(
            std::fs::read(payload.join("marker")).unwrap(),
            b"original-payload"
        );
        assert!(!inputs.output_iso.exists());
    }

    #[test]
    fn rejects_existing_output() {
        let temp = tempdir().unwrap();
        let rootfs = temp.path().join("rootfs");
        let payload = temp.path().join("payload");
        std::fs::create_dir(&rootfs).unwrap();
        std::fs::create_dir(&payload).unwrap();
        std::fs::write(temp.path().join("candidate.iso"), b"existing").unwrap();

        let inputs = IsoRefreshInputs {
            rootfs,
            payload,
            workspace: temp.path().join("candidate"),
            output_iso: temp.path().join("candidate.iso"),
        };

        assert!(inputs.validate().is_err());
    }

    #[test]
    fn rejects_overlapping_input_directories() {
        let temp = tempdir().unwrap();
        let rootfs = temp.path().join("rootfs");
        std::fs::create_dir(&rootfs).unwrap();

        let inputs = IsoRefreshInputs {
            rootfs: rootfs.clone(),
            payload: rootfs,
            workspace: temp.path().join("candidate"),
            output_iso: temp.path().join("candidate.iso"),
        };

        assert!(inputs.validate().is_err());
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlinked_workspace_parent() {
        use std::os::unix::fs::symlink;

        let temp = tempdir().unwrap();
        let rootfs = temp.path().join("rootfs");
        let payload = temp.path().join("payload");
        let destination = temp.path().join("destination");
        let alias = temp.path().join("alias");

        std::fs::create_dir(&rootfs).unwrap();
        std::fs::create_dir(&payload).unwrap();
        std::fs::create_dir(&destination).unwrap();
        symlink(&destination, &alias).unwrap();

        let inputs = IsoRefreshInputs {
            rootfs,
            payload,
            workspace: alias.join("candidate"),
            output_iso: temp.path().join("candidate.iso"),
        };

        assert!(inputs.validate().is_err());
    }

    #[test]
    fn rejects_workspace_inside_rootfs() {
        let temp = tempdir().unwrap();
        let rootfs = temp.path().join("rootfs");
        let payload = temp.path().join("payload");
        std::fs::create_dir(&rootfs).unwrap();
        std::fs::create_dir(&payload).unwrap();

        let inputs = IsoRefreshInputs {
            workspace: rootfs.join("candidate"),
            rootfs,
            payload,
            output_iso: temp.path().join("candidate.iso"),
        };

        assert!(inputs.validate().is_err());
    }
}
