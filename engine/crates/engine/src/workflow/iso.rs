//! Bootable ISO build workflow.

use crate::{
    Bootstrapper, BuildBackend, BuildContext, BuildError, IsoBackend, MmdebstrapBootstrapper,
    MmdebstrapError, RootfsBackend,
};
use executor::RootfsRunError;
use model::Plan;
use registry::PackageRepository;
use std::fs;

use super::rootfs_verification::{
    verify_grub_modules, verify_installed_packages, verify_intel_8265_firmware,
    verify_nvidia_pci_id_databases, verify_setpriv,
};

/// Coordinates creation of a bootable ISO appliance.
pub struct IsoWorkflow;

trait LiveRootfsPreparer {
    fn prepare(&mut self, build_context: &BuildContext) -> Result<(), BuildError>;
}

#[derive(Clone, Copy, Debug, Default)]
struct SystemLiveRootfsPreparer;

impl LiveRootfsPreparer for SystemLiveRootfsPreparer {
    fn prepare(&mut self, build_context: &BuildContext) -> Result<(), BuildError> {
        let daia_directory = build_context.rootfs().join("usr/share/daia");

        let registry_directory = build_context.asset_directory().parent().ok_or_else(|| {
            BuildError::Workspace(std::io::Error::other("asset directory has no parent"))
        })?;

        let package_manifest_source = registry_directory.join("package-manifests");
        let provider_source = registry_directory.join("providers");
        let appliance_profile_source = registry_directory.join("appliance-profiles");
        let content_repository_source = registry_directory.join("content-repositories");

        copy_directory_contents(
            &package_manifest_source,
            &daia_directory.join("package-manifests"),
        )?;

        copy_directory_contents(&provider_source, &daia_directory.join("providers"))?;

        copy_directory_contents(
            &appliance_profile_source,
            &daia_directory.join("appliance-profiles"),
        )?;

        copy_directory_contents(
            &content_repository_source,
            &daia_directory.join("content-repositories"),
        )?;

        copy_directory_contents(
            build_context.asset_directory(),
            &daia_directory.join("assets"),
        )?;

        if let Some(daia_binary) = build_context.daia_binary() {
            let usr_bin = build_context.rootfs().join("usr/bin");

            create_directory(&usr_bin)?;

            let destination = usr_bin.join("daia");

            copy_file(daia_binary, &destination)?;
        }

        if let Some(daia_tui_binary) = build_context.daia_tui_binary() {
            let usr_bin = build_context.rootfs().join("usr/bin");

            create_directory(&usr_bin)?;

            let destination = usr_bin.join("daia-tui");

            copy_file(daia_tui_binary, &destination)?;
        }

        if let Some(model_import_binary) = build_context.daia_model_import_binary() {
            let usr_bin = build_context.rootfs().join("usr/bin");

            create_directory(&usr_bin)?;

            let destination = usr_bin.join("daia-model-import");

            copy_file(model_import_binary, &destination)?;
        }

        install_exporter_binaries(build_context)?;

        if let Some(payload) = build_context.daia_payload_directory() {
            for relative_path in [
                "usr/bin/daia-model-importer",
                "usr/bin/daia-model-manager",
                "usr/share/applications/daia-model-manager.desktop",
            ] {
                let source = payload.join(relative_path);
                let destination = build_context.rootfs().join(relative_path);

                if let Some(parent) = destination.parent() {
                    create_directory(parent)?;
                }

                copy_file(&source, &destination)?;
            }
        }

        let systemd_directory = build_context.rootfs().join("etc/systemd/system");

        let multi_user_wants = systemd_directory.join("multi-user.target.wants");

        create_directory(&systemd_directory)?;
        create_directory(&multi_user_wants)?;

        let default_target = systemd_directory.join("default.target");

        if std::fs::symlink_metadata(&default_target).is_ok() {
            remove_file(&default_target)?;
        }

        create_symlink(
            std::path::Path::new("/usr/lib/systemd/system/multi-user.target"),
            &default_target,
        )?;

        let installer_service = systemd_directory.join("daia-installer.service");

        write_file(
            &installer_service,
            r#"[Unit]
Description=DAIA Rust Installer
ConditionKernelCommandLine=daia.mode=install
After=systemd-user-sessions.service
Before=graphical.target
Conflicts=graphical.target

[Service]
Type=simple
ExecStart=/usr/bin/daia-tui
StandardInput=tty
StandardOutput=tty
StandardError=tty
TTYPath=/dev/tty1
TTYReset=yes
TTYVHangup=yes
TTYVTDisallocate=no
Restart=no

[Install]
WantedBy=multi-user.target
"#,
        )?;

        let installer_link = multi_user_wants.join("daia-installer.service");

        if installer_link.exists() {
            remove_file(&installer_link)?;
        }

        create_symlink(
            std::path::Path::new("../daia-installer.service"),
            &installer_link,
        )?;

        // Recovery uses its own target so Debian's rescue.service does not
        // require sulogin authentication against the temporary Live root.
        // No installed filesystems are mounted or repaired automatically.
        write_file(
            &systemd_directory.join("daia-recovery.target"),
            r#"[Unit]
Description=DAIA Live Recovery Mode
Requires=sysinit.target daia-recovery.service
After=sysinit.target daia-recovery.service
AllowIsolate=yes
"#,
        )?;

        write_file(
            &systemd_directory.join("daia-recovery.service"),
            r#"[Unit]
Description=DAIA Live Recovery Console
ConditionKernelCommandLine=daia.mode=recovery
DefaultDependencies=no
After=sysinit.target
Before=daia-recovery.target
Conflicts=shutdown.target
Before=shutdown.target

[Service]
Type=idle
Environment=HOME=/root
WorkingDirectory=-/root
ExecStart=/usr/bin/bash --login
StandardInput=tty-force
StandardOutput=inherit
StandardError=inherit
TTYPath=/dev/tty1
TTYReset=yes
TTYVHangup=yes
KillMode=process
Restart=no
"#,
        )?;

        let getty_wants = build_context
            .rootfs()
            .join("etc/systemd/system/getty.target.wants");

        create_directory(&getty_wants)?;

        let tty1_getty_link = getty_wants.join("getty@tty1.service");

        if tty1_getty_link.exists() {
            remove_file(&tty1_getty_link)?;
        }

        for tty in 2..=6 {
            let getty_link = getty_wants.join(format!("getty@tty{tty}.service"));

            if std::fs::symlink_metadata(&getty_link).is_ok() {
                remove_file(&getty_link)?;
            }

            create_symlink(
                std::path::Path::new("/usr/lib/systemd/system/getty@.service"),
                &getty_link,
            )?;
        }

        // This configuration belongs only to the ephemeral Live USB.
        // NormalizeInstalledSystem must remove it from the installed rootfs.
        let live_sddm_directory = build_context.rootfs().join("etc/sddm.conf.d");
        create_directory(&live_sddm_directory)?;
        write_file(
            &live_sddm_directory.join("90-daia-live-autologin.conf"),
            "[Autologin]\nUser=daia\nSession=plasmax11.desktop\nRelogin=false\n",
        )?;

        clean_live_rootfs(build_context.rootfs())?;

        Ok(())
    }
}

trait RootfsVerifier {
    fn verify(&self, build_context: &BuildContext, plan: &Plan) -> Result<(), BuildError>;
}

#[derive(Clone, Debug)]
struct SystemRootfsVerifier {
    package_repository: PackageRepository,
}

impl RootfsVerifier for SystemRootfsVerifier {
    fn verify(&self, build_context: &BuildContext, plan: &Plan) -> Result<(), BuildError> {
        if plan.capability.as_str() != "desktop" {
            return Ok(());
        }

        let manifest = self.package_repository.manifest("desktop").ok_or_else(|| {
            BuildError::Workspace(std::io::Error::other(
                "rootfs verification failed: desktop package manifest missing",
            ))
        })?;

        let required: Vec<&str> = manifest.packages().iter().map(String::as_str).collect();

        verify_installed_packages(build_context.rootfs(), &required)
            .map_err(BuildError::Workspace)?;

        verify_installed_packages(
            build_context.rootfs(),
            &["grub-efi-amd64-bin", "grub-pc-bin"],
        )
        .map_err(BuildError::Workspace)?;

        verify_grub_modules(build_context.rootfs()).map_err(BuildError::Workspace)?;

        verify_intel_8265_firmware(build_context.rootfs()).map_err(BuildError::Workspace)?;
        verify_nvidia_pci_id_databases(build_context.rootfs()).map_err(BuildError::Workspace)?;
        verify_setpriv(build_context.rootfs()).map_err(BuildError::Workspace)?;

        if build_context.daia_payload_directory().is_some() {
            use std::os::unix::fs::PermissionsExt;

            for relative_path in [
                "usr/bin/daia-model-importer",
                "usr/bin/daia-model-manager",
                "usr/share/applications/daia-model-manager.desktop",
            ] {
                let path = build_context.rootfs().join(relative_path);
                if !path.is_file() {
                    return Err(BuildError::Workspace(std::io::Error::other(format!(
                        "rootfs verification failed: missing {}",
                        path.display()
                    ))));
                }
            }

            for executable in ["daia-model-importer", "daia-model-manager"] {
                let launcher = build_context.rootfs().join("usr/bin").join(executable);
                let mode = fs::metadata(&launcher)
                    .map_err(BuildError::Workspace)?
                    .permissions()
                    .mode();

                if mode & 0o111 == 0 {
                    return Err(BuildError::Workspace(std::io::Error::other(format!(
                        "rootfs verification failed: graphical model tool is not executable: {}",
                        launcher.display()
                    ))));
                }
            }
        }

        Ok(())
    }
}

struct IsoPipeline<B, R, L, V, I> {
    bootstrapper: B,
    rootfs_backend: R,
    live_rootfs_preparer: L,
    rootfs_verifier: V,
    iso_backend: I,
}

impl<B, R, L, V, I> IsoPipeline<B, R, L, V, I>
where
    B: Bootstrapper<Error = MmdebstrapError>,
    R: BuildBackend<Error = RootfsRunError>,
    L: LiveRootfsPreparer,
    V: RootfsVerifier,
    I: BuildBackend<Error = std::io::Error>,
{
    fn prepare(build_context: &BuildContext) -> Result<(), BuildError> {
        use std::path::Component;

        let destinations = [
            ("rootfs", build_context.rootfs()),
            ("ISO work directory", build_context.work_directory()),
            ("output ISO", build_context.output_iso()),
        ];

        for (label, path) in destinations {
            if !path.is_absolute()
                || path
                    .components()
                    .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
                || path == std::path::Path::new("/")
            {
                return Err(BuildError::Workspace(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!(
                        "fresh ISO build refused: unsafe {label} path: {}",
                        path.display()
                    ),
                )));
            }

            for ancestor in path.ancestors().skip(1) {
                match fs::symlink_metadata(ancestor) {
                    Ok(metadata) if metadata.file_type().is_symlink() => {
                        return Err(BuildError::Workspace(std::io::Error::new(
                            std::io::ErrorKind::InvalidInput,
                            format!(
                                "fresh ISO build refused: symlinked ancestor: {}",
                                ancestor.display()
                            ),
                        )));
                    }
                    Ok(metadata) if !metadata.is_dir() => {
                        return Err(BuildError::Workspace(std::io::Error::new(
                            std::io::ErrorKind::InvalidInput,
                            format!(
                                "fresh ISO build refused: non-directory ancestor: {}",
                                ancestor.display()
                            ),
                        )));
                    }
                    Ok(_) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(error) => return Err(BuildError::Workspace(error)),
                }
            }
        }

        for first in 0..destinations.len() {
            for second in (first + 1)..destinations.len() {
                let (first_label, first_path) = destinations[first];
                let (second_label, second_path) = destinations[second];

                if first_path.starts_with(second_path) || second_path.starts_with(first_path) {
                    return Err(BuildError::Workspace(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        format!(
                            "fresh ISO build refused: overlapping {first_label} and {second_label} paths"
                        ),
                    )));
                }
            }
        }

        // Fresh builds must not destroy existing workspaces or ISO images.
        // symlink_metadata also detects dangling symlinks.
        for (label, path) in [
            ("rootfs", build_context.rootfs()),
            ("ISO work directory", build_context.work_directory()),
            ("output ISO", build_context.output_iso()),
        ] {
            match fs::symlink_metadata(path) {
                Ok(_) => {
                    return Err(BuildError::Workspace(std::io::Error::new(
                        std::io::ErrorKind::AlreadyExists,
                        format!(
                            "fresh ISO build refused: {label} already exists: {}",
                            path.display()
                        ),
                    )));
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(BuildError::Workspace(error)),
            }
        }

        Ok(())
    }
    fn execute(mut self, build_context: &BuildContext, plan: &Plan) -> Result<(), BuildError> {
        Self::prepare(build_context)?;

        self.bootstrap(build_context)?;
        self.build_rootfs(plan)?;
        self.live_rootfs_preparer.prepare(build_context)?;
        self.rootfs_verifier.verify(build_context, plan)?;
        self.build_iso(plan)?;

        Ok(())
    }
    fn bootstrap(&self, build_context: &BuildContext) -> Result<(), BuildError> {
        self.bootstrapper.bootstrap(build_context)?;

        Ok(())
    }

    fn build_rootfs(&mut self, plan: &Plan) -> Result<(), BuildError> {
        self.rootfs_backend.build(plan).map_err(BuildError::Rootfs)
    }

    fn build_iso(&mut self, plan: &Plan) -> Result<(), BuildError> {
        self.iso_backend.build(plan).map_err(BuildError::Iso)
    }
}

fn create_directory(path: &std::path::Path) -> Result<(), BuildError> {
    match fs::create_dir_all(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
            let status = std::process::Command::new("sudo")
                .arg("mkdir")
                .arg("-p")
                .arg("--")
                .arg(path)
                .status()
                .map_err(BuildError::Workspace)?;

            if status.success() {
                Ok(())
            } else {
                Err(BuildError::Workspace(std::io::Error::other(format!(
                    "failed to create privileged directory `{}`",
                    path.display()
                ))))
            }
        }
        Err(error) => Err(BuildError::Workspace(error)),
    }
}

fn clean_directory(path: &std::path::Path) -> Result<(), BuildError> {
    if path.exists() {
        let mut entries = fs::read_dir(path).map_err(BuildError::Workspace)?;

        match entries.next() {
            None => return Ok(()),
            Some(Ok(_)) => {
                return Err(BuildError::Workspace(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    format!(
                        "live rootfs cleanup refused nonempty APT directory `{}`",
                        path.display()
                    ),
                )));
            }
            Some(Err(error)) => return Err(BuildError::Workspace(error)),
        }
    }

    create_directory(path)?;

    Ok(())
}
#[cfg(target_os = "linux")]
fn ensure_no_live_rootfs_mounts(rootfs: &std::path::Path) -> Result<(), BuildError> {
    let rootfs = rootfs.canonicalize().map_err(BuildError::Workspace)?;
    let mountinfo = fs::read_to_string("/proc/self/mountinfo").map_err(BuildError::Workspace)?;
    validate_live_rootfs_mountinfo(&rootfs, &mountinfo)
}

#[cfg(target_os = "linux")]
fn validate_live_rootfs_mountinfo(
    rootfs: &std::path::Path,
    mountinfo: &str,
) -> Result<(), BuildError> {
    use std::os::unix::ffi::OsStringExt;

    for line in mountinfo.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();

        if fields.len() < 5 {
            return Err(BuildError::Workspace(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "malformed mountinfo entry",
            )));
        }

        let encoded = fields[4].as_bytes();
        let mut decoded = Vec::new();
        let mut index = 0;

        while index < encoded.len() {
            if encoded[index] == b'\\' {
                if index + 3 >= encoded.len()
                    || !encoded[index + 1..index + 4]
                        .iter()
                        .all(|byte| (b'0'..=b'7').contains(byte))
                {
                    return Err(BuildError::Workspace(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "invalid mountinfo path escape",
                    )));
                }

                let value = ((encoded[index + 1] - b'0') << 6)
                    | ((encoded[index + 2] - b'0') << 3)
                    | (encoded[index + 3] - b'0');

                decoded.push(value);
                index += 4;
            } else {
                decoded.push(encoded[index]);
                index += 1;
            }
        }

        let mountpoint = std::path::PathBuf::from(std::ffi::OsString::from_vec(decoded));

        if mountpoint.starts_with(&rootfs) {
            return Err(BuildError::Workspace(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!(
                    "live rootfs cleanup refused active mountpoint: {}",
                    mountpoint.display()
                ),
            )));
        }
    }

    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn ensure_no_live_rootfs_mounts(_rootfs: &std::path::Path) -> Result<(), BuildError> {
    Err(BuildError::Workspace(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "live rootfs cleanup mountpoint validation requires Linux",
    )))
}

fn clean_live_rootfs(rootfs: &std::path::Path) -> Result<(), BuildError> {
    use std::path::Component;

    let paths = [
        rootfs.join("var/cache/apt/archives"),
        rootfs.join("var/lib/apt/lists"),
    ];

    // Validate every cleanup destination before modifying either directory.
    for path in &paths {
        if !path.is_absolute()
            || path
                .components()
                .any(|component| matches!(component, Component::ParentDir))
        {
            return Err(BuildError::Workspace(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("unsafe live rootfs cleanup path: {}", path.display()),
            )));
        }

        for component_path in path.ancestors() {
            match fs::symlink_metadata(component_path) {
                Ok(metadata) if metadata.file_type().is_symlink() => {
                    return Err(BuildError::Workspace(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        format!(
                            "live rootfs cleanup refused symlink: {}",
                            component_path.display()
                        ),
                    )));
                }
                Ok(metadata) if !metadata.is_dir() => {
                    return Err(BuildError::Workspace(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        format!(
                            "live rootfs cleanup refused non-directory: {}",
                            component_path.display()
                        ),
                    )));
                }
                Ok(_) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(BuildError::Workspace(error)),
            }
        }
    }

    // Refuse mounted filesystems before modifying either APT directory.
    ensure_no_live_rootfs_mounts(rootfs)?;

    // Preflight every existing directory before creating either one.
    for path in &paths {
        if path.exists() {
            let mut entries = fs::read_dir(path).map_err(BuildError::Workspace)?;

            match entries.next() {
                None => {}
                Some(Ok(_)) => {
                    return Err(BuildError::Workspace(std::io::Error::new(
                        std::io::ErrorKind::PermissionDenied,
                        format!(
                            "live rootfs cleanup refused nonempty APT directory `{}`",
                            path.display()
                        ),
                    )));
                }
                Some(Err(error)) => return Err(BuildError::Workspace(error)),
            }
        }
    }

    // Both destinations passed preflight; create missing directories.
    for path in paths {
        clean_directory(&path)?;
    }

    Ok(())
}

fn remove_file(path: &std::path::Path) -> Result<(), BuildError> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
            let status = std::process::Command::new("sudo")
                .arg("rm")
                .arg("-f")
                .arg("--")
                .arg(path)
                .status()
                .map_err(BuildError::Workspace)?;

            if status.success() {
                Ok(())
            } else {
                Err(BuildError::Workspace(std::io::Error::other(format!(
                    "failed to remove privileged build file `{}`",
                    path.display()
                ))))
            }
        }
        Err(error) => Err(BuildError::Workspace(error)),
    }
}

fn create_symlink(target: &std::path::Path, link: &std::path::Path) -> Result<(), BuildError> {
    match std::os::unix::fs::symlink(target, link) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
            let status = std::process::Command::new("sudo")
                .arg("ln")
                .arg("-s")
                .arg("--")
                .arg(target)
                .arg(link)
                .status()
                .map_err(BuildError::Workspace)?;

            if status.success() {
                Ok(())
            } else {
                Err(BuildError::Workspace(std::io::Error::other(format!(
                    "failed to create privileged build symlink `{}`",
                    link.display()
                ))))
            }
        }
        Err(error) => Err(BuildError::Workspace(error)),
    }
}

fn write_file(path: &std::path::Path, contents: &str) -> Result<(), BuildError> {
    match fs::write(path, contents) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
            let mut child = std::process::Command::new("sudo")
                .arg("tee")
                .arg(path)
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::null())
                .spawn()
                .map_err(BuildError::Workspace)?;

            use std::io::Write;

            child
                .stdin
                .as_mut()
                .ok_or_else(|| {
                    BuildError::Workspace(std::io::Error::other(
                        "failed to open privileged file writer stdin",
                    ))
                })?
                .write_all(contents.as_bytes())
                .map_err(BuildError::Workspace)?;

            let status = child.wait().map_err(BuildError::Workspace)?;

            if status.success() {
                Ok(())
            } else {
                Err(BuildError::Workspace(std::io::Error::other(format!(
                    "failed to write privileged build file `{}`",
                    path.display()
                ))))
            }
        }
        Err(error) => Err(BuildError::Workspace(error)),
    }
}

fn install_exporter_binaries(build_context: &BuildContext) -> Result<(), BuildError> {
    // Stage the exporter executables without activating either component.
    // Authorization and worker isolation must be established separately.
    let broker = build_context.daia_model_export_broker_binary();
    let worker = build_context.daia_model_export_worker_binary();

    if broker.is_some() != worker.is_some() {
        return Err(BuildError::Workspace(std::io::Error::other(
            "Model Exporter broker and worker must be staged together",
        )));
    }

    for (binary, name) in [
        (
            build_context.daia_model_export_broker_binary(),
            "daia-model-export-broker",
        ),
        (
            build_context.daia_model_export_worker_binary(),
            "daia-model-export-worker",
        ),
    ] {
        if let Some(binary) = binary {
            let libexec = build_context.rootfs().join("usr/libexec");
            create_directory(&libexec)?;
            let destination = libexec.join(name);
            copy_file(binary, &destination)?;

            use std::os::unix::fs::PermissionsExt;

            enforce_exporter_root_ownership(&destination)?;

            match fs::set_permissions(&destination, fs::Permissions::from_mode(0o755)) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
                    let status = std::process::Command::new("sudo")
                        .arg("chmod")
                        .arg("0755")
                        .arg("--")
                        .arg(&destination)
                        .status()
                        .map_err(BuildError::Workspace)?;

                    if !status.success() {
                        return Err(BuildError::Workspace(std::io::Error::other(format!(
                            "failed to set privileged exporter permissions: {}",
                            destination.display()
                        ))));
                    }
                }
                Err(error) => return Err(BuildError::Workspace(error)),
            }
        }
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn verify_exporter_secure_executable(path: &std::path::Path) -> Result<(), BuildError> {
    use rustix::fs::{Mode, OFlags};
    use std::path::Component;

    if !path.is_absolute() {
        return Err(BuildError::Workspace(std::io::Error::other(
            "exporter executable path must be absolute",
        )));
    }

    let mut components = path.components().peekable();

    if components.next() != Some(Component::RootDir) {
        return Err(BuildError::Workspace(std::io::Error::other(
            "invalid exporter executable root",
        )));
    }

    let mut directory = rustix::fs::openat(
        rustix::fs::CWD,
        "/",
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|error| BuildError::Workspace(error.into()))?;

    let check_directory = |fd: &std::os::fd::OwnedFd| -> Result<(), BuildError> {
        let metadata =
            rustix::fs::fstat(fd).map_err(|error| BuildError::Workspace(error.into()))?;

        if metadata.st_uid != 0 || metadata.st_gid != 0 || metadata.st_mode & 0o022 != 0 {
            return Err(BuildError::Workspace(std::io::Error::other(
                "untrusted exporter executable directory ancestry",
            )));
        }

        Ok(())
    };

    check_directory(&directory)?;

    while let Some(component) = components.next() {
        let Component::Normal(name) = component else {
            return Err(BuildError::Workspace(std::io::Error::other(
                "invalid exporter executable path component",
            )));
        };

        if components.peek().is_some() {
            let next = rustix::fs::openat(
                &directory,
                name,
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(|error| BuildError::Workspace(error.into()))?;

            check_directory(&next)?;
            directory = next;
        } else {
            let executable = rustix::fs::openat(
                &directory,
                name,
                OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(|error| BuildError::Workspace(error.into()))?;

            let metadata = rustix::fs::fstat(&executable)
                .map_err(|error| BuildError::Workspace(error.into()))?;

            if rustix::fs::FileType::from_raw_mode(metadata.st_mode)
                != rustix::fs::FileType::RegularFile
                || metadata.st_uid != 0
                || metadata.st_gid != 0
                || metadata.st_mode & 0o022 != 0
                || metadata.st_mode & 0o111 == 0
            {
                return Err(BuildError::Workspace(std::io::Error::other(
                    "untrusted exporter executable",
                )));
            }

            return Ok(());
        }
    }

    Err(BuildError::Workspace(std::io::Error::other(
        "missing exporter executable filename",
    )))
}

fn verify_exporter_root_ownership(path: &std::path::Path) -> Result<(), BuildError> {
    use std::os::unix::fs::MetadataExt;

    let metadata = fs::symlink_metadata(path).map_err(BuildError::Workspace)?;

    if !metadata.file_type().is_file() || metadata.uid() != 0 || metadata.gid() != 0 {
        return Err(BuildError::Workspace(std::io::Error::other(format!(
            "exporter binary must be a root-owned regular file: {}",
            path.display()
        ))));
    }

    Ok(())
}

fn enforce_exporter_root_ownership(path: &std::path::Path) -> Result<(), BuildError> {
    use std::os::unix::fs::MetadataExt;

    let metadata = fs::symlink_metadata(path).map_err(BuildError::Workspace)?;

    if !metadata.file_type().is_file() {
        return Err(BuildError::Workspace(std::io::Error::other(format!(
            "exporter binary is not a regular file: {}",
            path.display()
        ))));
    }

    if metadata.uid() != 0 || metadata.gid() != 0 {
        let status = std::process::Command::new("sudo")
            .arg("chown")
            .arg("0:0")
            .arg("--")
            .arg(path)
            .status()
            .map_err(BuildError::Workspace)?;

        if !status.success() {
            return Err(BuildError::Workspace(std::io::Error::other(format!(
                "failed to establish exporter root ownership: {}",
                path.display()
            ))));
        }
    }

    verify_exporter_root_ownership(path)
}

fn copy_file(source: &std::path::Path, destination: &std::path::Path) -> Result<(), BuildError> {
    match fs::copy(source, destination) {
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
            let status = std::process::Command::new("sudo")
                .arg("cp")
                .arg("--")
                .arg(source)
                .arg(destination)
                .status()
                .map_err(BuildError::Workspace)?;

            if status.success() {
                Ok(())
            } else {
                Err(BuildError::Workspace(std::io::Error::other(format!(
                    "failed to copy privileged file `{}` to `{}`",
                    source.display(),
                    destination.display()
                ))))
            }
        }
        Err(error) => Err(BuildError::Workspace(std::io::Error::new(
            error.kind(),
            format!(
                "failed to copy `{}` to `{}`: {error}",
                source.display(),
                destination.display()
            ),
        ))),
    }
}

fn copy_directory_contents(
    source: &std::path::Path,
    destination: &std::path::Path,
) -> Result<(), BuildError> {
    create_directory(destination)?;

    for entry in fs::read_dir(source).map_err(|error| {
        BuildError::Workspace(std::io::Error::new(
            error.kind(),
            format!("failed to read directory `{}`: {error}", source.display()),
        ))
    })? {
        let entry = entry.map_err(BuildError::Workspace)?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());

        if source_path.is_dir() {
            copy_directory_contents(&source_path, &destination_path)?;
        } else {
            copy_file(&source_path, &destination_path)?;
        }
    }

    Ok(())
}
impl IsoWorkflow {
    /// Runs the bootable ISO workflow.
    ///
    /// # Errors
    ///
    /// Returns a [`BuildError`] if bootstrapping, root filesystem execution,
    /// or ISO generation fails.
    pub fn run(
        build_context: &BuildContext,
        package_repository: &PackageRepository,
        plan: Plan,
    ) -> Result<Plan, BuildError> {
        let pipeline = Self::production_pipeline(build_context, package_repository);

        Self::run_with(build_context, plan, pipeline)
    }

    fn production_pipeline(
        build_context: &BuildContext,
        package_repository: &PackageRepository,
    ) -> IsoPipeline<
        MmdebstrapBootstrapper,
        RootfsBackend,
        SystemLiveRootfsPreparer,
        SystemRootfsVerifier,
        IsoBackend,
    > {
        IsoPipeline {
            bootstrapper: MmdebstrapBootstrapper::new(),
            rootfs_backend: RootfsBackend::new(
                build_context.rootfs().to_path_buf(),
                build_context.asset_directory().to_path_buf(),
                package_repository.clone(),
            ),
            live_rootfs_preparer: SystemLiveRootfsPreparer,
            rootfs_verifier: SystemRootfsVerifier {
                package_repository: package_repository.clone(),
            },
            iso_backend: IsoBackend::from_context(build_context),
        }
    }
    fn run_with<B, R, L, V, I>(
        build_context: &BuildContext,
        plan: Plan,
        pipeline: IsoPipeline<B, R, L, V, I>,
    ) -> Result<Plan, BuildError>
    where
        B: Bootstrapper<Error = MmdebstrapError>,
        R: BuildBackend<Error = RootfsRunError>,
        L: LiveRootfsPreparer,
        V: RootfsVerifier,
        I: BuildBackend<Error = std::io::Error>,
    {
        pipeline.execute(build_context, &plan)?;

        Ok(plan)
    }
}
#[cfg(test)]
mod tests {

    use executor::{ExecuteError, RootfsRunError};
    use model::{
        Action, Capability, CapabilityId, PackageManifest, Plan, PlanStep, Provider, ProviderId,
    };
    use registry::{PackageRepository, Registry};
    use std::{
        fs, io,
        sync::{Arc, Mutex},
    };

    use super::{
        IsoPipeline, IsoWorkflow, LiveRootfsPreparer, RootfsVerifier, SystemLiveRootfsPreparer,
        SystemRootfsVerifier,
    };
    use crate::{
        BootstrapConfig, Bootstrapper, BuildBackend, BuildContext, BuildError, Engine,
        MmdebstrapError,
    };

    type ExecutionLog = Arc<Mutex<Vec<&'static str>>>;

    struct RecordingBootstrapper {
        log: ExecutionLog,
        error: bool,
    }

    impl Bootstrapper for RecordingBootstrapper {
        type Error = MmdebstrapError;

        fn bootstrap(&self, _context: &BuildContext) -> Result<(), Self::Error> {
            self.log
                .lock()
                .expect("execution log should not be poisoned")
                .push("bootstrap");

            if self.error {
                Err(MmdebstrapError::Process(io::Error::other(
                    "bootstrap failed",
                )))
            } else {
                Ok(())
            }
        }
    }

    struct RecordingRootfsBackend {
        log: ExecutionLog,
        error: bool,
    }

    impl BuildBackend for RecordingRootfsBackend {
        type Error = RootfsRunError;

        fn build(&mut self, _plan: &Plan) -> Result<(), ExecuteError<Self::Error>> {
            self.log
                .lock()
                .expect("execution log should not be poisoned")
                .push("rootfs");

            if self.error {
                Err(ExecuteError::Environment(io::Error::other("rootfs failed")))
            } else {
                Ok(())
            }
        }
    }

    struct RecordingIsoBackend {
        log: ExecutionLog,
        error: bool,
    }

    impl BuildBackend for RecordingIsoBackend {
        type Error = io::Error;

        fn build(&mut self, _plan: &Plan) -> Result<(), ExecuteError<Self::Error>> {
            self.log
                .lock()
                .expect("execution log should not be poisoned")
                .push("iso");

            if self.error {
                Err(ExecuteError::Environment(io::Error::other("iso failed")))
            } else {
                Ok(())
            }
        }
    }

    struct RecordingRootfsVerifier {
        log: ExecutionLog,
        error: bool,
    }

    impl RootfsVerifier for RecordingRootfsVerifier {
        fn verify(&self, _context: &BuildContext, _plan: &Plan) -> Result<(), BuildError> {
            self.log
                .lock()
                .expect("execution log should not be poisoned")
                .push("verify");

            if self.error {
                Err(BuildError::Workspace(io::Error::other(
                    "rootfs verification failed",
                )))
            } else {
                Ok(())
            }
        }
    }

    fn workflow_inputs() -> (Engine, Capability, BuildContext, PackageRepository) {
        let registry = Registry::from_providers(vec![Provider {
            id: ProviderId::new("desktop"),
            capability: CapabilityId::new("desktop"),
            steps: vec![
                PlanStep::new(Action::InstallPackageManifest("desktop".to_owned())),
                PlanStep::new(Action::EnableService("display-manager".to_owned())),
            ],
        }])
        .expect("test registry should be valid");

        let bootstrap = BootstrapConfig::new(
            "bookworm",
            "amd64",
            "https://deb.debian.org/debian",
            vec!["main".to_owned()],
            "minbase",
        );

        (
            Engine::from_registry(registry),
            Capability::new("desktop"),
            BuildContext::new(
                "build/rootfs",
                "images/source.iso",
                "build/work",
                "build/output.iso",
                std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../registry/assets"),
                bootstrap,
            ),
            PackageRepository::new(),
        )
    }

    fn run_workflow(
        bootstrap_error: bool,
        rootfs_error: bool,
        iso_error: bool,
    ) -> (Result<Plan, BuildError>, ExecutionLog) {
        run_workflow_with_verification(bootstrap_error, rootfs_error, false, iso_error)
    }

    fn run_workflow_with_verification(
        bootstrap_error: bool,
        rootfs_error: bool,
        verification_error: bool,
        iso_error: bool,
    ) -> (Result<Plan, BuildError>, ExecutionLog) {
        let (engine, capability, _, _) = workflow_inputs();

        let temp = tempfile::tempdir().expect("temporary workflow directory should be created");

        let registry_directory = temp.path().join("registry");
        let asset_directory = registry_directory.join("assets");
        let package_manifest_directory = registry_directory.join("package-manifests");
        let provider_directory = registry_directory.join("providers");
        let appliance_profile_directory = registry_directory.join("appliance-profiles");
        let content_repository_directory = registry_directory.join("content-repositories");

        std::fs::create_dir_all(&asset_directory).expect("asset directory should be created");

        std::fs::create_dir_all(&package_manifest_directory)
            .expect("package manifest directory should be created");

        std::fs::create_dir_all(&provider_directory).expect("provider directory should be created");

        std::fs::create_dir_all(&appliance_profile_directory)
            .expect("appliance profile directory should be created");

        std::fs::create_dir_all(&content_repository_directory)
            .expect("content repository directory should be created");

        let build_context = BuildContext::new(
            temp.path().join("rootfs"),
            temp.path().join("source.iso"),
            temp.path().join("work"),
            temp.path().join("output.iso"),
            asset_directory,
            BootstrapConfig::default(),
        );

        let plan = engine
            .plan(&capability)
            .expect("test workflow plan should build");

        let log = Arc::new(Mutex::new(Vec::new()));

        let result = IsoWorkflow::run_with(
            &build_context,
            plan,
            IsoPipeline {
                bootstrapper: RecordingBootstrapper {
                    log: Arc::clone(&log),
                    error: bootstrap_error,
                },
                rootfs_backend: RecordingRootfsBackend {
                    log: Arc::clone(&log),
                    error: rootfs_error,
                },
                live_rootfs_preparer: SystemLiveRootfsPreparer,
                rootfs_verifier: RecordingRootfsVerifier {
                    log: Arc::clone(&log),
                    error: verification_error,
                },
                iso_backend: RecordingIsoBackend {
                    log: Arc::clone(&log),
                    error: iso_error,
                },
            },
        );

        (result, log)
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn secure_exporter_verifier_accepts_trusted_system_executable() {
        use super::verify_exporter_secure_executable;
        use std::os::unix::fs::MetadataExt;

        let path = std::path::Path::new("/usr/bin/env");

        let metadata = fs::symlink_metadata(path).expect("inspect system executable");

        assert!(
            metadata.file_type().is_file(),
            "test requires a regular system executable"
        );
        assert_eq!(metadata.uid(), 0, "system executable must be root-owned");
        assert_eq!(
            metadata.gid(),
            0,
            "system executable must be root-group-owned"
        );

        verify_exporter_secure_executable(path)
            .expect("trusted system executable must pass verification");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn secure_exporter_verifier_rejects_system_symlinks() {
        use super::verify_exporter_secure_executable;

        let trusted = std::path::Path::new("/usr/bin/env");
        let executable_link = std::path::Path::new("/usr/bin/sh");
        let ancestor_link = std::path::Path::new("/bin/env");

        assert!(
            fs::symlink_metadata(trusted)
                .expect("inspect trusted executable")
                .file_type()
                .is_file()
        );

        assert!(
            fs::symlink_metadata(executable_link)
                .expect("inspect executable symlink")
                .file_type()
                .is_symlink()
        );

        assert!(
            fs::symlink_metadata("/bin")
                .expect("inspect directory symlink")
                .file_type()
                .is_symlink()
        );

        verify_exporter_secure_executable(trusted).expect("trusted executable must pass");

        assert!(
            verify_exporter_secure_executable(executable_link).is_err(),
            "executable symlink must be rejected"
        );

        assert!(
            verify_exporter_secure_executable(ancestor_link).is_err(),
            "directory ancestor symlink must be rejected"
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn secure_exporter_verifier_rejects_untrusted_paths() {
        use super::verify_exporter_secure_executable;
        use std::os::unix::fs::{PermissionsExt, symlink};

        let temp = tempfile::tempdir().expect("create test directory");
        let executable = temp.path().join("exporter");
        fs::write(&executable, b"test executable").expect("write fixture");
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o755))
            .expect("make fixture executable");

        assert!(
            verify_exporter_secure_executable(&executable).is_err(),
            "unprivileged directory ancestry must be rejected"
        );

        let link = temp.path().join("exporter-link");
        symlink(&executable, &link).expect("create symlink");
        assert!(
            verify_exporter_secure_executable(&link).is_err(),
            "symlinked executable must be rejected"
        );

        let non_executable = temp.path().join("not-executable");
        fs::write(&non_executable, b"fixture").expect("write non-executable");
        assert!(
            verify_exporter_secure_executable(&non_executable).is_err(),
            "non-root-owned executable must be rejected"
        );
    }

    #[test]
    fn exporter_staging_rejects_partial_binary_configuration() {
        let temp = tempfile::tempdir().expect("temporary directory");
        let rootfs = temp.path().join("rootfs");
        let broker = temp.path().join("broker");
        let worker = temp.path().join("worker");

        fs::write(&broker, b"broker").expect("write broker fixture");
        fs::write(&worker, b"worker").expect("write worker fixture");

        for (include_broker, include_worker) in [(true, false), (false, true)] {
            let mut context = BuildContext::new(
                &rootfs,
                temp.path().join("source.iso"),
                temp.path().join("work"),
                temp.path().join("output.iso"),
                temp.path().join("assets"),
                BootstrapConfig::default(),
            );

            if include_broker {
                context = context.with_daia_model_export_broker_binary(&broker);
            }

            if include_worker {
                context = context.with_daia_model_export_worker_binary(&worker);
            }

            let result = super::install_exporter_binaries(&context);
            assert!(
                result.is_err(),
                "partial exporter configuration must be rejected"
            );
            assert!(
                !rootfs.join("usr/libexec/daia-model-export-broker").exists(),
                "broker must not be staged after rejection"
            );
            assert!(
                !rootfs.join("usr/libexec/daia-model-export-worker").exists(),
                "worker must not be staged after rejection"
            );
        }
    }

    #[test]
    fn exporter_staging_allows_both_binaries_absent() {
        let temp = tempfile::tempdir().expect("temporary directory");

        let context = BuildContext::new(
            temp.path().join("rootfs"),
            temp.path().join("source.iso"),
            temp.path().join("work"),
            temp.path().join("output.iso"),
            temp.path().join("assets"),
            BootstrapConfig::default(),
        );

        super::install_exporter_binaries(&context)
            .expect("existing optional staging contract must remain valid");
    }

    #[test]
    fn system_live_rootfs_preparer_can_be_created() {
        let _preparer = SystemLiveRootfsPreparer;
    }

    #[test]
    fn rejects_unprivileged_exporter_binary() {
        let temp = tempfile::tempdir().expect("temporary directory");
        let binary = temp.path().join("exporter");
        fs::write(&binary, b"test").expect("write exporter fixture");

        assert!(
            super::verify_exporter_root_ownership(&binary).is_err(),
            "unprivileged exporter must be rejected"
        );
    }

    #[test]
    fn prepares_live_rootfs_daia_binary() {
        let temp = tempfile::tempdir().expect("temporary directory should be created");
        let rootfs = temp.path().join("rootfs");

        let registry_directory = temp.path().join("registry");
        let asset_directory = registry_directory.join("assets");
        let package_manifest_directory = registry_directory.join("package-manifests");
        let provider_directory = registry_directory.join("providers");
        let appliance_profile_directory = registry_directory.join("appliance-profiles");
        let content_repository_directory = registry_directory.join("content-repositories");

        let daia_binary = temp.path().join("daia");
        let daia_tui_binary = temp.path().join("daia-tui");
        let model_import_binary = temp.path().join("daia-model-import");
        let export_broker = temp.path().join("daia-model-export-broker");
        let export_worker = temp.path().join("daia-model-export-worker");
        let payload = temp.path().join("payload");
        let importer = payload.join("usr/bin/daia-model-importer");
        let manager = payload.join("usr/bin/daia-model-manager");
        let manager_desktop_entry =
            payload.join("usr/share/applications/daia-model-manager.desktop");

        fs::create_dir_all(&rootfs).expect("rootfs should be created");

        fs::create_dir_all(&asset_directory).expect("asset directory should be created");

        fs::create_dir_all(&package_manifest_directory)
            .expect("package manifest directory should be created");

        fs::create_dir_all(&provider_directory).expect("provider directory should be created");

        fs::create_dir_all(&appliance_profile_directory)
            .expect("appliance profile directory should be created");

        fs::create_dir_all(&content_repository_directory)
            .expect("content repository directory should be created");

        fs::write(
            content_repository_directory.join("local-models.yaml"),
            b"content repository",
        )
        .expect("content repository fixture should be written");

        fs::write(&daia_binary, b"daia").expect("DAIA binary fixture should be written");
        fs::write(&daia_tui_binary, b"daia-tui")
            .expect("DAIA TUI binary fixture should be written");
        fs::write(&model_import_binary, b"daia-model-import")
            .expect("model-import binary fixture should be written");
        fs::write(&export_broker, b"export-broker")
            .expect("export broker fixture should be written");
        fs::write(&export_worker, b"export-worker")
            .expect("export worker fixture should be written");

        fs::create_dir_all(importer.parent().unwrap())
            .expect("importer payload directory should be created");
        fs::create_dir_all(manager_desktop_entry.parent().unwrap())
            .expect("desktop payload directory should be created");

        fs::write(&manager, b"#!/usr/bin/env python3\\n")
            .expect("manager fixture should be written");
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&manager, fs::Permissions::from_mode(0o755))
                .expect("manager fixture should be executable");
        }
        fs::write(
            &manager_desktop_entry,
            b"[Desktop Entry]\\nExec=/usr/bin/daia-model-manager\\n",
        )
        .expect("manager desktop entry fixture should be written");

        fs::write(&importer, b"#!/bin/sh\\n").expect("importer fixture should be written");
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&importer, fs::Permissions::from_mode(0o755))
                .expect("importer fixture should be executable");
        }

        let getty_wants = rootfs.join("etc/systemd/system/getty.target.wants");
        fs::create_dir_all(&getty_wants).expect("getty wants directory should be created");

        let tty1_getty_link = getty_wants.join("getty@tty1.service");
        std::os::unix::fs::symlink("/usr/lib/systemd/system/getty@.service", &tty1_getty_link)
            .expect("tty1 getty fixture should be created");

        let build_context = BuildContext::new(
            rootfs.clone(),
            temp.path().join("source.iso"),
            temp.path().join("work"),
            temp.path().join("output.iso"),
            asset_directory,
            BootstrapConfig::default(),
        )
        .with_daia_binary(&daia_binary)
        .with_daia_tui_binary(&daia_tui_binary)
        .with_daia_model_import_binary(&model_import_binary)
        .with_daia_model_export_broker_binary(&export_broker)
        .with_daia_model_export_worker_binary(&export_worker)
        .with_daia_payload_directory(&payload);

        let mut preparer = SystemLiveRootfsPreparer;

        preparer
            .prepare(&build_context)
            .expect("live rootfs preparation should succeed");

        assert_eq!(
            fs::read(rootfs.join("usr/bin/daia")).expect("DAIA binary should be copied"),
            b"daia"
        );

        assert_eq!(
            fs::read(rootfs.join("usr/bin/daia-tui")).expect("DAIA TUI binary should be copied"),
            b"daia-tui"
        );

        assert_eq!(
            fs::read(rootfs.join("usr/bin/daia-model-import"))
                .expect("model-import helper should be copied"),
            b"daia-model-import"
        );

        assert_eq!(
            fs::read(rootfs.join("usr/libexec/daia-model-export-broker"))
                .expect("export broker should be installed"),
            b"export-broker"
        );
        assert_eq!(
            fs::read(rootfs.join("usr/libexec/daia-model-export-worker"))
                .expect("export worker should be installed"),
            b"export-worker"
        );

        {
            use std::os::unix::fs::PermissionsExt;

            for name in ["daia-model-export-broker", "daia-model-export-worker"] {
                let path = rootfs.join("usr/libexec").join(name);
                let mode = fs::metadata(&path)
                    .expect("export executable metadata")
                    .permissions()
                    .mode()
                    & 0o7777;
                assert_eq!(mode, 0o755, "{name} must have mode 0755");
            }
        }

        assert_eq!(
            fs::read(rootfs.join("usr/bin/daia-model-importer"))
                .expect("graphical importer should be installed"),
            b"#!/bin/sh\\n"
        );
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(rootfs.join("usr/bin/daia-model-importer"))
                .expect("installed importer metadata should be readable")
                .permissions()
                .mode()
                & 0o777;

            assert_eq!(mode, 0o755, "graphical importer must be executable");
        }

        assert_eq!(
            fs::read(rootfs.join("usr/bin/daia-model-manager"))
                .expect("graphical manager should be installed"),
            b"#!/usr/bin/env python3\\n"
        );
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(rootfs.join("usr/bin/daia-model-manager"))
                .expect("installed manager metadata should be readable")
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(mode, 0o755, "graphical manager must be executable");
        }
        assert_eq!(
            fs::read(rootfs.join("usr/share/applications/daia-model-manager.desktop"))
                .expect("manager desktop entry should be installed"),
            b"[Desktop Entry]\\nExec=/usr/bin/daia-model-manager\\n"
        );

        let installer_service =
            fs::read_to_string(rootfs.join("etc/systemd/system/daia-installer.service"))
                .expect("DAIA installer service should be readable");

        assert!(
            installer_service.contains("ExecStart=/usr/bin/daia-tui"),
            "live installer service must launch the DAIA terminal installer"
        );
        assert!(
            installer_service.contains("ConditionKernelCommandLine=daia.mode=install"),
            "installer service must run only when Install DAIA is selected"
        );

        let recovery_target =
            fs::read_to_string(rootfs.join("etc/systemd/system/daia-recovery.target"))
                .expect("DAIA recovery target should exist");
        let recovery_service =
            fs::read_to_string(rootfs.join("etc/systemd/system/daia-recovery.service"))
                .expect("DAIA recovery service should exist");

        assert!(
            recovery_target.contains("Requires=sysinit.target daia-recovery.service"),
            "recovery target must require the recovery console"
        );
        assert!(
            recovery_service.contains("ConditionKernelCommandLine=daia.mode=recovery"),
            "recovery console must be restricted to Recovery Mode"
        );
        assert!(
            recovery_service.contains("ExecStart=/usr/bin/bash --login"),
            "recovery console must launch the local shell"
        );
        assert!(
            recovery_service.contains("TTYPath=/dev/tty1"),
            "recovery console must use the physical terminal"
        );

        let live_autologin =
            fs::read_to_string(rootfs.join("etc/sddm.conf.d/90-daia-live-autologin.conf"))
                .expect("Live SDDM autologin configuration should exist");

        assert_eq!(
            live_autologin, "[Autologin]\nUser=daia\nSession=plasmax11.desktop\nRelogin=false\n",
            "Live SDDM must autologin the temporary DAIA user using Plasma X11"
        );
        assert!(
            !installer_service.contains("ExecStart=/usr/bin/daia install"),
            "live installer service must not launch the legacy CLI installer presentation"
        );

        assert_eq!(
            fs::read(rootfs.join("usr/share/daia/content-repositories/local-models.yaml"))
                .expect("content repository should be copied"),
            b"content repository"
        );

        assert!(
            rootfs
                .join("etc/systemd/system/daia-installer.service")
                .is_file()
        );

        assert_eq!(
            fs::read_link(rootfs.join("etc/systemd/system/default.target"))
                .expect("live rootfs default target should be a symlink"),
            std::path::PathBuf::from("/usr/lib/systemd/system/multi-user.target")
        );

        assert!(
            rootfs
                .join("etc/systemd/system/multi-user.target.wants/daia-installer.service")
                .is_symlink()
        );

        assert!(
            !tty1_getty_link.exists(),
            "tty1 getty must be removed before the DAIA installer owns tty1"
        );

        for tty in 2..=6 {
            let getty_link = getty_wants.join(format!("getty@tty{tty}.service"));

            assert!(
                getty_link.is_symlink(),
                "live rootfs must enable a troubleshooting getty on tty{tty}"
            );

            assert_eq!(
                fs::read_link(&getty_link).expect("troubleshooting getty link should be readable"),
                std::path::PathBuf::from("/usr/lib/systemd/system/getty@.service")
            );
        }
    }

    #[test]
    fn prepares_live_rootfs_runtime_directories() {
        let temp = tempfile::tempdir().expect("temporary directory should be created");
        let rootfs = temp.path().join("rootfs");

        let registry_directory = temp.path().join("registry");
        let asset_directory = registry_directory.join("assets");
        let package_manifest_directory = registry_directory.join("package-manifests");
        let provider_directory = registry_directory.join("providers");
        let appliance_profile_directory = registry_directory.join("appliance-profiles");
        let content_repository_directory = registry_directory.join("content-repositories");

        fs::create_dir_all(&rootfs).expect("rootfs should be created");

        fs::create_dir_all(&asset_directory).expect("asset directory should be created");

        fs::create_dir_all(&package_manifest_directory)
            .expect("package manifest directory should be created");

        fs::create_dir_all(&provider_directory).expect("provider directory should be created");

        fs::create_dir_all(&appliance_profile_directory)
            .expect("appliance profile directory should be created");

        fs::create_dir_all(&content_repository_directory)
            .expect("content repository directory should be created");

        let build_context = BuildContext::new(
            rootfs.clone(),
            temp.path().join("source.iso"),
            temp.path().join("work"),
            temp.path().join("output.iso"),
            asset_directory,
            BootstrapConfig::default(),
        );

        let mut preparer = SystemLiveRootfsPreparer;

        preparer
            .prepare(&build_context)
            .expect("live rootfs preparation should succeed");

        assert!(rootfs.join("usr/share/daia/package-manifests").is_dir());

        assert!(rootfs.join("usr/share/daia/providers").is_dir());

        assert!(rootfs.join("usr/share/daia/appliance-profiles").is_dir());

        assert!(rootfs.join("usr/share/daia/assets").is_dir());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn live_rootfs_mountinfo_rejects_rootfs_mountpoint() {
        let rootfs = std::path::Path::new("/tmp/daia-test-rootfs");
        let mountinfo = "42 1 0:42 / /tmp/daia-test-rootfs rw - tmpfs tmpfs rw\n";

        assert!(super::validate_live_rootfs_mountinfo(rootfs, mountinfo).is_err());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn live_rootfs_mountinfo_rejects_nested_mountpoint() {
        let rootfs = std::path::Path::new("/tmp/daia-test-rootfs");
        let mountinfo =
            "42 1 0:42 / /tmp/daia-test-rootfs/var/cache/apt/archives rw - tmpfs tmpfs rw\n";

        assert!(super::validate_live_rootfs_mountinfo(rootfs, mountinfo).is_err());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn live_rootfs_mountinfo_decodes_escaped_spaces() {
        let rootfs = std::path::Path::new("/tmp/daia test rootfs");
        let mountinfo =
            r"42 1 0:42 / /tmp/daia\040test\040rootfs/var/lib/apt/lists rw - tmpfs tmpfs rw";

        assert!(super::validate_live_rootfs_mountinfo(rootfs, mountinfo).is_err());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn live_rootfs_mountinfo_allows_unrelated_mountpoint() {
        let rootfs = std::path::Path::new("/tmp/daia-test-rootfs");
        let mountinfo = "42 1 0:42 / /tmp/unrelated-mount rw - tmpfs tmpfs rw\n";

        assert!(super::validate_live_rootfs_mountinfo(rootfs, mountinfo).is_ok());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn live_rootfs_mountinfo_rejects_malformed_entry() {
        let rootfs = std::path::Path::new("/tmp/daia-test-rootfs");

        assert!(super::validate_live_rootfs_mountinfo(rootfs, "invalid entry\n").is_err());

        let invalid_escape = r"42 1 0:42 / /tmp/daia\04x-rootfs rw - tmpfs tmpfs rw";

        assert!(super::validate_live_rootfs_mountinfo(rootfs, invalid_escape).is_err());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn live_rootfs_cleanup_preserves_empty_apt_directories() {
        use std::os::unix::fs::MetadataExt;

        let temp = tempfile::tempdir().expect("temporary directory");
        let rootfs = temp.path().join("rootfs");

        let archives = rootfs.join("var/cache/apt/archives");
        let lists = rootfs.join("var/lib/apt/lists");

        fs::create_dir_all(&archives).expect("create archives");
        fs::create_dir_all(&lists).expect("create lists");

        let archives_before = fs::metadata(&archives).expect("archives metadata");
        let lists_before = fs::metadata(&lists).expect("lists metadata");

        super::clean_live_rootfs(&rootfs).expect("empty APT cleanup should succeed");

        let archives_after = fs::metadata(&archives).expect("archives metadata");
        let lists_after = fs::metadata(&lists).expect("lists metadata");

        assert_eq!(archives_before.dev(), archives_after.dev());
        assert_eq!(archives_before.ino(), archives_after.ino());

        assert_eq!(lists_before.dev(), lists_after.dev());
        assert_eq!(lists_before.ino(), lists_after.ino());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn live_rootfs_cleanup_refuses_nonempty_apt_directories() {
        use std::os::unix::fs::MetadataExt;

        let temp = tempfile::tempdir().expect("temporary directory");
        let rootfs = temp.path().join("rootfs");

        let archives = rootfs.join("var/cache/apt/archives");
        let lists = rootfs.join("var/lib/apt/lists");

        fs::create_dir_all(&archives).expect("create archives");
        fs::create_dir_all(&lists).expect("create lists");

        fs::write(archives.join("sentinel"), b"preserve").expect("create archives sentinel");
        fs::write(lists.join("sentinel"), b"preserve").expect("create lists sentinel");

        let archives_before = fs::metadata(&archives).expect("archives metadata");
        let lists_before = fs::metadata(&lists).expect("lists metadata");

        let result = super::clean_live_rootfs(&rootfs);

        assert!(result.is_err(), "nonempty APT cleanup must fail closed");

        assert_eq!(
            fs::read(archives.join("sentinel")).expect("archives sentinel"),
            b"preserve"
        );
        assert_eq!(
            fs::read(lists.join("sentinel")).expect("lists sentinel"),
            b"preserve"
        );

        let archives_after = fs::metadata(&archives).expect("archives metadata");
        let lists_after = fs::metadata(&lists).expect("lists metadata");

        assert_eq!(archives_before.dev(), archives_after.dev());
        assert_eq!(archives_before.ino(), archives_after.ino());
        assert_eq!(lists_before.dev(), lists_after.dev());
        assert_eq!(lists_before.ino(), lists_after.ino());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn live_rootfs_cleanup_preflights_both_directories_before_creation() {
        let temp = tempfile::tempdir().expect("temporary directory");
        let rootfs = temp.path().join("rootfs");

        let archives = rootfs.join("var/cache/apt/archives");
        let lists = rootfs.join("var/lib/apt/lists");

        fs::create_dir_all(&lists).expect("create lists");
        fs::write(lists.join("sentinel"), b"preserve").expect("create lists sentinel");

        assert!(!archives.exists(), "archives must initially be absent");

        let result = super::clean_live_rootfs(&rootfs);

        assert!(result.is_err(), "nonempty lists must reject cleanup");
        assert!(
            !archives.exists(),
            "archives must remain absent when lists preflight fails"
        );
        assert_eq!(
            fs::read(lists.join("sentinel")).expect("lists sentinel"),
            b"preserve"
        );
    }

    #[cfg(unix)]
    #[test]
    fn live_rootfs_cleanup_validates_all_paths_before_deletion() {
        let temp = tempfile::tempdir().expect("temporary directory");
        let rootfs = temp.path().join("rootfs");
        let external = temp.path().join("external");

        let archives = rootfs.join("var/cache/apt/archives");
        fs::create_dir_all(&archives).expect("create archives");
        fs::write(archives.join("sentinel"), b"preserve").expect("write archives sentinel");

        fs::create_dir_all(rootfs.join("var/lib/apt")).expect("create lists parent");
        fs::create_dir_all(&external).expect("create external directory");

        std::os::unix::fs::symlink(&external, rootfs.join("var/lib/apt/lists"))
            .expect("create unsafe lists symlink");

        let result = super::clean_live_rootfs(&rootfs);

        assert!(result.is_err(), "unsafe cleanup must be rejected");
        assert_eq!(
            fs::read(archives.join("sentinel")).unwrap(),
            b"preserve",
            "cleanup must validate every path before deleting anything"
        );
    }

    #[cfg(unix)]
    #[test]
    fn live_rootfs_cleanup_rejects_symlinked_parent() {
        let temp = tempfile::tempdir().expect("temporary directory");
        let rootfs = temp.path().join("rootfs");
        let external = temp.path().join("external");

        fs::create_dir_all(rootfs.join("var/cache")).expect("create cache parent");
        fs::create_dir_all(external.join("apt/archives")).expect("create external APT directory");
        fs::write(external.join("apt/archives/sentinel"), b"preserve").expect("write sentinel");

        std::os::unix::fs::symlink(external.join("apt"), rootfs.join("var/cache/apt"))
            .expect("create unsafe parent symlink");

        let result = super::clean_live_rootfs(&rootfs);

        assert!(result.is_err(), "symlinked parent must be rejected");
        assert_eq!(
            fs::read(external.join("apt/archives/sentinel")).unwrap(),
            b"preserve"
        );
    }

    #[cfg(unix)]
    #[test]
    fn live_rootfs_cleanup_rejects_symlinked_apt_directory() {
        let temp = tempfile::tempdir().expect("temporary directory");
        let rootfs = temp.path().join("rootfs");
        let external = temp.path().join("external");

        fs::create_dir_all(rootfs.join("var/cache/apt")).expect("create APT parent");
        fs::create_dir_all(&external).expect("create external directory");
        fs::write(external.join("sentinel"), b"preserve").expect("write external sentinel");

        std::os::unix::fs::symlink(&external, rootfs.join("var/cache/apt/archives"))
            .expect("create unsafe APT symlink");

        let result = super::clean_live_rootfs(&rootfs);

        assert!(result.is_err(), "symlinked cleanup path must be rejected");
        assert_eq!(fs::read(external.join("sentinel")).unwrap(), b"preserve");
    }

    #[test]
    fn fresh_build_refuses_existing_paths_without_deleting_contents() {
        let temp = tempfile::tempdir().expect("temporary directory");

        for existing in ["rootfs", "iso-build", "output.iso"] {
            let rootfs = temp.path().join("rootfs");
            let work = temp.path().join("iso-build");
            let output = temp.path().join("output.iso");

            let selected = temp.path().join(existing);
            if existing == "output.iso" {
                fs::write(&selected, b"preserve").expect("create output fixture");
            } else {
                fs::create_dir_all(&selected).expect("create directory fixture");
                fs::write(selected.join("sentinel"), b"preserve").expect("create sentinel");
            }

            let context = BuildContext::new(
                &rootfs,
                temp.path().join("source.iso"),
                &work,
                &output,
                temp.path().join("assets"),
                BootstrapConfig::default(),
            );

            let result = super::IsoPipeline::<
                super::MmdebstrapBootstrapper,
                super::RootfsBackend,
                super::SystemLiveRootfsPreparer,
                super::SystemRootfsVerifier,
                super::IsoBackend,
            >::prepare(&context);

            assert!(result.is_err(), "{existing} must be rejected");

            if existing == "output.iso" {
                assert_eq!(fs::read(&selected).unwrap(), b"preserve");
            } else {
                assert_eq!(fs::read(selected.join("sentinel")).unwrap(), b"preserve");
            }

            if existing == "output.iso" {
                fs::remove_file(selected).unwrap();
            } else {
                fs::remove_dir_all(selected).unwrap();
            }
        }
    }

    #[test]
    fn fresh_build_rejects_relative_and_overlapping_paths() {
        let temp = tempfile::tempdir().expect("temporary directory");

        for (rootfs, work, output) in [
            (
                std::path::PathBuf::from("relative/rootfs"),
                temp.path().join("work"),
                temp.path().join("output.iso"),
            ),
            (
                temp.path().join("work/rootfs"),
                temp.path().join("work"),
                temp.path().join("output.iso"),
            ),
            (
                temp.path().join("rootfs"),
                temp.path().join("rootfs/iso-build"),
                temp.path().join("output.iso"),
            ),
        ] {
            let context = BuildContext::new(
                rootfs,
                temp.path().join("source.iso"),
                work,
                output,
                temp.path().join("assets"),
                BootstrapConfig::default(),
            );

            let result = super::IsoPipeline::<
                super::MmdebstrapBootstrapper,
                super::RootfsBackend,
                super::SystemLiveRootfsPreparer,
                super::SystemRootfsVerifier,
                super::IsoBackend,
            >::prepare(&context);

            assert!(result.is_err(), "unsafe build paths must be rejected");
        }
    }

    #[cfg(unix)]
    #[test]
    fn fresh_build_rejects_symlinked_parent() {
        let temp = tempfile::tempdir().expect("temporary directory");
        let real = temp.path().join("real");
        let linked = temp.path().join("linked");

        fs::create_dir(&real).expect("create real directory");
        std::os::unix::fs::symlink(&real, &linked).expect("create parent symlink");

        let context = BuildContext::new(
            linked.join("rootfs"),
            temp.path().join("source.iso"),
            temp.path().join("iso-build"),
            temp.path().join("output.iso"),
            temp.path().join("assets"),
            BootstrapConfig::default(),
        );

        let result = super::IsoPipeline::<
            super::MmdebstrapBootstrapper,
            super::RootfsBackend,
            super::SystemLiveRootfsPreparer,
            super::SystemRootfsVerifier,
            super::IsoBackend,
        >::prepare(&context);

        assert!(result.is_err(), "symlinked parent must be rejected");
        assert!(!real.join("rootfs").exists());
    }

    #[test]
    fn executes_workflow_stages_in_order() {
        let (result, log) = run_workflow(false, false, false);

        let plan = result.expect("workflow should succeed");

        assert_eq!(plan.capability, Capability::new("desktop"));
        assert_eq!(plan.provider, ProviderId::new("desktop"));
        assert_eq!(
            *log.lock().expect("execution log should not be poisoned"),
            vec!["bootstrap", "rootfs", "verify", "iso"]
        );
    }

    fn create_desktop_grub_module_fixtures(rootfs: &std::path::Path) {
        for (target, modules) in [
            (
                "i386-pc",
                &[
                    "biosdisk.mod",
                    "ext2.mod",
                    "linux.mod",
                    "normal.mod",
                    "part_gpt.mod",
                ][..],
            ),
            (
                "x86_64-efi",
                &[
                    "efi_gop.mod",
                    "ext2.mod",
                    "linux.mod",
                    "normal.mod",
                    "part_gpt.mod",
                ][..],
            ),
        ] {
            let directory = rootfs.join("usr/lib/grub").join(target);
            fs::create_dir_all(&directory).expect("create GRUB module directory");

            for module in modules {
                fs::write(directory.join(module), b"test module")
                    .expect("write GRUB module fixture");
            }
        }
    }

    #[test]
    fn desktop_verifier_accepts_installed_packages_and_firmware() {
        let temp = tempfile::tempdir().expect("temporary directory");
        let rootfs = temp.path().join("rootfs");
        let dpkg = rootfs.join("var/lib/dpkg");

        create_desktop_grub_module_fixtures(&rootfs);

        let nvidia_directory = rootfs.join("usr/share/nvidia");
        fs::create_dir_all(&nvidia_directory).expect("create NVIDIA database directory");

        for name in [
            "nvidia.ids",
            "nvidia-tesla-535.ids",
            "nvidia-legacy-390xx.ids",
        ] {
            fs::write(nvidia_directory.join(name), b"10DE1340\n")
                .expect("write NVIDIA database fixture");
        }

        let firmware = rootfs.join("usr/lib/firmware");

        fs::create_dir_all(&dpkg).expect("create dpkg directory");
        fs::create_dir_all(&firmware).expect("create firmware directory");

        fs::write(
            dpkg.join("status"),
            "Package: network-manager\nStatus: install ok installed\n\nPackage: firmware-iwlwifi\nStatus: install ok installed\n\nPackage: grub-efi-amd64-bin\nStatus: install ok installed\n\nPackage: grub-pc-bin\nStatus: install ok installed\n\n",
        )
        .expect("write package status");

        fs::write(firmware.join("iwlwifi-8265-36.ucode"), b"test firmware")
            .expect("write firmware fixture");

        let setpriv = rootfs.join("usr/bin/setpriv");
        fs::create_dir_all(setpriv.parent().expect("setpriv parent"))
            .expect("create setpriv directory");
        fs::write(&setpriv, b"test executable").expect("write setpriv fixture");

        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&setpriv, fs::Permissions::from_mode(0o755))
                .expect("make setpriv executable");
        }

        let repository = PackageRepository::from_manifests(vec![PackageManifest::new(
            "desktop",
            vec!["network-manager".into(), "firmware-iwlwifi".into()],
        )])
        .expect("valid package repository");

        let verifier = SystemRootfsVerifier {
            package_repository: repository,
        };

        let (engine, capability, _, _) = workflow_inputs();
        let plan = engine.plan(&capability).expect("desktop plan");

        let context = BuildContext::new(
            rootfs,
            temp.path().join("source.iso"),
            temp.path().join("work"),
            temp.path().join("output.iso"),
            temp.path().join("assets"),
            BootstrapConfig::default(),
        );

        verifier
            .verify(&context, &plan)
            .expect("complete desktop rootfs should pass verification");
    }

    #[test]
    fn desktop_verifier_rejects_missing_graphical_importer() {
        let temp = tempfile::tempdir().expect("temporary directory");
        let rootfs = temp.path().join("rootfs");
        let dpkg = rootfs.join("var/lib/dpkg");

        create_desktop_grub_module_fixtures(&rootfs);

        let nvidia_directory = rootfs.join("usr/share/nvidia");
        fs::create_dir_all(&nvidia_directory).expect("create NVIDIA database directory");

        for name in [
            "nvidia.ids",
            "nvidia-tesla-535.ids",
            "nvidia-legacy-390xx.ids",
        ] {
            fs::write(nvidia_directory.join(name), b"10DE1340\n")
                .expect("write NVIDIA database fixture");
        }

        let firmware = rootfs.join("usr/lib/firmware");
        let setpriv = rootfs.join("usr/bin/setpriv");

        fs::create_dir_all(&dpkg).expect("create dpkg directory");
        fs::create_dir_all(&firmware).expect("create firmware directory");
        fs::create_dir_all(setpriv.parent().expect("setpriv parent"))
            .expect("create binary directory");

        fs::write(
            dpkg.join("status"),
            "Package: network-manager\nStatus: install ok installed\n\nPackage: firmware-iwlwifi\nStatus: install ok installed\n\nPackage: grub-efi-amd64-bin\nStatus: install ok installed\n\nPackage: grub-pc-bin\nStatus: install ok installed\n\n",
        )
        .expect("write package status");

        fs::write(firmware.join("iwlwifi-8265-36.ucode"), b"test firmware")
            .expect("write firmware fixture");
        fs::write(&setpriv, b"test executable").expect("write setpriv fixture");

        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&setpriv, fs::Permissions::from_mode(0o755))
                .expect("make setpriv executable");
        }

        let repository = PackageRepository::from_manifests(vec![PackageManifest::new(
            "desktop",
            vec!["network-manager".into(), "firmware-iwlwifi".into()],
        )])
        .expect("valid package repository");

        let verifier = SystemRootfsVerifier {
            package_repository: repository,
        };

        let (engine, capability, _, _) = workflow_inputs();
        let plan = engine.plan(&capability).expect("desktop plan");

        let context = BuildContext::new(
            rootfs,
            temp.path().join("source.iso"),
            temp.path().join("work"),
            temp.path().join("output.iso"),
            temp.path().join("assets"),
            BootstrapConfig::default(),
        )
        .with_daia_payload_directory(temp.path().join("payload"));

        let error = verifier
            .verify(&context, &plan)
            .expect_err("missing graphical importer must fail verification");

        assert!(
            error.to_string().contains("daia-model-importer"),
            "error must identify missing graphical importer: {error:?}"
        );
    }

    #[test]
    fn desktop_verifier_rejects_missing_nvidia_databases() {
        let temp = tempfile::tempdir().expect("temporary directory");
        let rootfs = temp.path().join("rootfs");
        let dpkg = rootfs.join("var/lib/dpkg");

        create_desktop_grub_module_fixtures(&rootfs);
        let firmware = rootfs.join("usr/lib/firmware");
        let setpriv = rootfs.join("usr/bin/setpriv");

        fs::create_dir_all(&dpkg).expect("create dpkg directory");
        fs::create_dir_all(&firmware).expect("create firmware directory");
        fs::create_dir_all(setpriv.parent().expect("setpriv parent"))
            .expect("create binary directory");

        fs::write(
            dpkg.join("status"),
            "Package: network-manager\nStatus: install ok installed\n\nPackage: firmware-iwlwifi\nStatus: install ok installed\n\nPackage: grub-efi-amd64-bin\nStatus: install ok installed\n\nPackage: grub-pc-bin\nStatus: install ok installed\n\n",
        )
        .expect("write package status");

        fs::write(firmware.join("iwlwifi-8265-36.ucode"), b"test firmware")
            .expect("write firmware fixture");
        fs::write(&setpriv, b"test executable").expect("write setpriv fixture");

        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&setpriv, fs::Permissions::from_mode(0o755))
                .expect("make setpriv executable");
        }

        let repository = PackageRepository::from_manifests(vec![PackageManifest::new(
            "desktop",
            vec!["network-manager".into(), "firmware-iwlwifi".into()],
        )])
        .expect("valid package repository");

        let verifier = SystemRootfsVerifier {
            package_repository: repository,
        };

        let (engine, capability, _, _) = workflow_inputs();
        let plan = engine.plan(&capability).expect("desktop plan");

        let context = BuildContext::new(
            rootfs,
            temp.path().join("source.iso"),
            temp.path().join("work"),
            temp.path().join("output.iso"),
            temp.path().join("assets"),
            BootstrapConfig::default(),
        );

        let error = verifier
            .verify(&context, &plan)
            .expect_err("missing NVIDIA databases must fail verification");

        assert!(
            format!("{error:?}").contains("nvidia.ids"),
            "verification error must identify the missing NVIDIA database: {error:?}"
        );
    }

    #[test]
    fn desktop_verifier_rejects_missing_setpriv() {
        let temp = tempfile::tempdir().expect("temporary directory");
        let rootfs = temp.path().join("rootfs");
        let dpkg = rootfs.join("var/lib/dpkg");

        create_desktop_grub_module_fixtures(&rootfs);

        let nvidia_directory = rootfs.join("usr/share/nvidia");
        fs::create_dir_all(&nvidia_directory).expect("create NVIDIA database directory");

        for name in [
            "nvidia.ids",
            "nvidia-tesla-535.ids",
            "nvidia-legacy-390xx.ids",
        ] {
            fs::write(nvidia_directory.join(name), b"10DE1340\n")
                .expect("write NVIDIA database fixture");
        }

        let firmware = rootfs.join("usr/lib/firmware");

        fs::create_dir_all(&dpkg).expect("create dpkg directory");
        fs::create_dir_all(&firmware).expect("create firmware directory");

        fs::write(
            dpkg.join("status"),
            "Package: network-manager\nStatus: install ok installed\n\nPackage: firmware-iwlwifi\nStatus: install ok installed\n\nPackage: grub-efi-amd64-bin\nStatus: install ok installed\n\nPackage: grub-pc-bin\nStatus: install ok installed\n\n",
        )
        .expect("write package status");

        fs::write(firmware.join("iwlwifi-8265-36.ucode"), b"test firmware")
            .expect("write firmware fixture");

        let repository = PackageRepository::from_manifests(vec![PackageManifest::new(
            "desktop",
            vec!["network-manager".into(), "firmware-iwlwifi".into()],
        )])
        .expect("valid package repository");

        let verifier = SystemRootfsVerifier {
            package_repository: repository,
        };

        let (engine, capability, _, _) = workflow_inputs();
        let plan = engine.plan(&capability).expect("desktop plan");

        let context = BuildContext::new(
            rootfs,
            temp.path().join("source.iso"),
            temp.path().join("work"),
            temp.path().join("output.iso"),
            temp.path().join("assets"),
            BootstrapConfig::default(),
        );

        let error = verifier
            .verify(&context, &plan)
            .expect_err("missing setpriv must fail desktop verification");

        assert!(
            format!("{error:?}").contains("/usr/bin/setpriv"),
            "verification error should identify missing setpriv: {error:?}"
        );
    }

    #[test]
    fn desktop_verifier_rejects_missing_bios_grub_package() {
        let temp = tempfile::tempdir().expect("temporary directory");
        let rootfs = temp.path().join("rootfs");
        let dpkg = rootfs.join("var/lib/dpkg");

        fs::create_dir_all(&dpkg).expect("create dpkg directory");
        fs::write(
            dpkg.join("status"),
            "Package: network-manager\nStatus: install ok installed\n\nPackage: firmware-iwlwifi\nStatus: install ok installed\n\nPackage: grub-efi-amd64-bin\nStatus: install ok installed\n\n",
        )
        .expect("write package status");

        let repository = PackageRepository::from_manifests(vec![PackageManifest::new(
            "desktop",
            vec!["network-manager".into(), "firmware-iwlwifi".into()],
        )])
        .expect("valid package repository");

        let verifier = SystemRootfsVerifier {
            package_repository: repository,
        };

        let (engine, capability, _, _) = workflow_inputs();
        let plan = engine.plan(&capability).expect("desktop plan");

        let context = BuildContext::new(
            rootfs,
            temp.path().join("source.iso"),
            temp.path().join("work"),
            temp.path().join("output.iso"),
            temp.path().join("assets"),
            BootstrapConfig::default(),
        );

        let error = verifier
            .verify(&context, &plan)
            .expect_err("missing BIOS GRUB package must fail verification");

        assert!(
            error.to_string().contains("grub-pc-bin"),
            "error must identify missing BIOS GRUB package: {error:?}"
        );
    }

    #[test]
    fn desktop_verifier_rejects_missing_manifest_package() {
        let temp = tempfile::tempdir().expect("temporary directory");

        let rootfs = temp.path().join("rootfs");
        let dpkg = rootfs.join("var/lib/dpkg");

        fs::create_dir_all(&dpkg).expect("create dpkg directory");
        fs::write(
            dpkg.join("status"),
            "Package: network-manager\nStatus: install ok installed\n\n",
        )
        .expect("write package status");

        let repository = PackageRepository::from_manifests(vec![PackageManifest::new(
            "desktop",
            vec!["network-manager".into(), "firmware-iwlwifi".into()],
        )])
        .expect("valid package repository");

        let verifier = SystemRootfsVerifier {
            package_repository: repository,
        };

        let (engine, capability, _, _) = workflow_inputs();
        let plan = engine.plan(&capability).expect("desktop plan");

        let context = BuildContext::new(
            rootfs,
            temp.path().join("source.iso"),
            temp.path().join("work"),
            temp.path().join("output.iso"),
            temp.path().join("assets"),
            BootstrapConfig::default(),
        );

        let error = verifier
            .verify(&context, &plan)
            .expect_err("missing firmware package must fail verification");

        assert!(
            error.to_string().contains("firmware-iwlwifi")
                || format!("{error:?}").contains("firmware-iwlwifi"),
            "verification error should identify the missing package: {error:?}"
        );
    }

    #[test]
    fn verification_failure_prevents_iso_packaging() {
        let (result, log) = run_workflow_with_verification(false, false, true, false);

        assert!(matches!(result, Err(BuildError::Workspace(_))));
        assert_eq!(
            *log.lock().expect("execution log should not be poisoned"),
            vec!["bootstrap", "rootfs", "verify"]
        );
    }

    #[test]
    fn returns_bootstrap_error_without_running_backends() {
        let (result, log) = run_workflow(true, false, false);

        assert!(matches!(result, Err(BuildError::Bootstrap(_))));
        assert_eq!(
            *log.lock().expect("execution log should not be poisoned"),
            vec!["bootstrap"]
        );
    }

    #[test]
    fn returns_rootfs_error_without_running_iso_backend() {
        let (result, log) = run_workflow(false, true, false);

        assert!(matches!(result, Err(BuildError::Rootfs(_))));
        assert_eq!(
            *log.lock().expect("execution log should not be poisoned"),
            vec!["bootstrap", "rootfs"]
        );
    }

    #[test]
    fn returns_iso_error_after_rootfs_execution() {
        let (result, log) = run_workflow(false, false, true);

        assert!(matches!(result, Err(BuildError::Iso(_))));
        assert_eq!(
            *log.lock().expect("execution log should not be poisoned"),
            vec!["bootstrap", "rootfs", "verify", "iso"]
        );
    }
}
