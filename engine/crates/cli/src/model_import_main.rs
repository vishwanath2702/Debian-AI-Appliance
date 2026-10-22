//! Restricted DAIA model-import helper.
//!
//! Privileged imports require root authorization and restricted validation.

use std::env;
use std::process::ExitCode;

fn main() -> ExitCode {
    let arguments: Vec<_> = env::args_os().skip(1).collect();

    if is_model_list_request(&arguments) {
        let Some(device) = arguments[1].to_str() else {
            eprintln!("Invalid device path encoding");
            return ExitCode::FAILURE;
        };

        if let Err(message) = validate_device_path(device) {
            eprintln!("{message}");
            return ExitCode::FAILURE;
        }

        if !is_root() {
            eprintln!("DAIA model discovery requires administrator authorization");
            return ExitCode::FAILURE;
        }

        return run_restricted_model_list(device);
    }

    if is_model_resume_request(&arguments) {
        let Some(model_name) = arguments[1].to_str() else {
            eprintln!("Invalid model name encoding");
            return ExitCode::FAILURE;
        };

        if let Err(message) = validate_model_name(model_name) {
            eprintln!("{message}");
            return ExitCode::FAILURE;
        }

        if !is_root() {
            eprintln!("DAIA model recovery requires administrator authorization");
            return ExitCode::FAILURE;
        }

        return run_restricted_model_resume(model_name);
    }

    if let Err(message) = validate_argument_count(arguments.len()) {
        eprintln!("{message}");
        return ExitCode::FAILURE;
    }

    let Some(device) = arguments[0].to_str() else {
        eprintln!("Invalid device path encoding");
        return ExitCode::FAILURE;
    };

    if let Err(message) = validate_device_path(device) {
        eprintln!("{message}");
        return ExitCode::FAILURE;
    }

    let Some(filename) = arguments[1].to_str() else {
        eprintln!("Invalid GGUF filename encoding");
        return ExitCode::FAILURE;
    };

    if let Err(message) = validate_gguf_filename(filename) {
        eprintln!("{message}");
        return ExitCode::FAILURE;
    }

    let Some(model_name) = arguments[2].to_str() else {
        eprintln!("Invalid model name encoding");
        return ExitCode::FAILURE;
    };

    if let Err(message) = validate_model_name(model_name) {
        eprintln!("{message}");
        return ExitCode::FAILURE;
    }

    if !is_root() {
        eprintln!("DAIA model import requires administrator authorization");
        return ExitCode::FAILURE;
    }

    run_restricted_model_import(device, filename, model_name)
}

fn is_model_resume_request(arguments: &[std::ffi::OsString]) -> bool {
    arguments.len() == 2 && arguments[0] == "resume-model"
}

fn run_restricted_model_resume(model_name: &str) -> ExitCode {
    use std::path::Path;

    if let Err(error) = validate_privileged_model_destination() {
        eprintln!("Model recovery destination security check failed: {error}");
        return ExitCode::FAILURE;
    }

    let mut executor =
        engine::OllamaModelRealizationExecutor::new(RestrictedOllamaRunner::default());

    match application::resume_model_realization_with(model_name, Path::new("/"), &mut executor) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("Model recovery failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn is_model_list_request(arguments: &[std::ffi::OsString]) -> bool {
    arguments.len() == 2 && arguments[0] == "list-models"
}

fn run_restricted_model_list(device: &str) -> ExitCode {
    use std::path::Path;

    let inspector = inspector::LinuxContentVolumeInspector::new()
        .with_command("/usr/bin/lsblk")
        .with_findmnt_command("/usr/bin/findmnt")
        .with_post_install_discovery()
        .with_restricted_environment();

    let mounter = inspector::LinuxContentVolumeMounter::new()
        .with_command("/usr/bin/mount")
        .with_umount_command("/usr/bin/umount")
        .with_lsblk_command("/usr/bin/lsblk")
        .with_restricted_environment();

    match application::discover_models_on_fresh_volume_with(&inspector, &mounter, Path::new(device))
    {
        Ok(report) => {
            for discovered in report.models {
                if let Some(filename) = discovered.item.path().file_name() {
                    println!("{}", filename.to_string_lossy());
                }
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("Error discovering GGUF models: {error}");
            ExitCode::FAILURE
        }
    }
}

#[allow(dead_code)]
fn validate_ollama_create_command(command: &std::process::Command) -> Result<(), &'static str> {
    use std::ffi::OsStr;

    if command.get_program() != OsStr::new("ollama") {
        return Err("Unexpected Ollama executable");
    }

    let arguments: Vec<_> = command.get_args().collect();

    if arguments.len() != 4
        || arguments[0] != OsStr::new("create")
        || arguments[1] != OsStr::new("-f")
        || arguments[2].is_empty()
        || arguments[3].is_empty()
        || arguments[3].to_string_lossy().starts_with('-')
    {
        return Err("Unexpected Ollama command arguments");
    }

    Ok(())
}

#[allow(dead_code)]
fn restricted_ollama_command(original: &std::process::Command) -> std::process::Command {
    let mut command = std::process::Command::new("/usr/local/bin/ollama");

    command.args(original.get_args());
    command.env_clear();
    command.env("PATH", "/usr/local/bin:/usr/bin:/bin");
    command.env("HOME", "/var/lib/ollama");
    command.env("OLLAMA_MODELS", "/var/lib/ollama/models");
    command.env("OLLAMA_HOST", "127.0.0.1:11434");

    command
}

// Resolve the Ollama service account rather than assuming a fixed UID/GID.
fn ollama_service_identity() -> std::io::Result<(u32, u32)> {
    let passwd = std::fs::read_to_string("/etc/passwd")?;
    parse_ollama_service_identity(&passwd)
}

fn parse_ollama_service_identity(passwd: &str) -> std::io::Result<(u32, u32)> {
    use std::io::{Error, ErrorKind};

    let identities: Vec<_> = passwd
        .lines()
        .filter_map(|line| {
            let fields: Vec<_> = line.split(':').collect();

            if fields.len() != 7 || fields[0] != "ollama" {
                return None;
            }

            Some((fields[2], fields[3]))
        })
        .collect();

    let [(uid, gid)] = identities.as_slice() else {
        return Err(Error::new(
            ErrorKind::NotFound,
            "Expected exactly one Ollama service account",
        ));
    };

    let uid = uid
        .parse::<u32>()
        .map_err(|_| Error::new(ErrorKind::InvalidData, "Invalid Ollama UID"))?;

    let gid = gid
        .parse::<u32>()
        .map_err(|_| Error::new(ErrorKind::InvalidData, "Invalid Ollama GID"))?;

    if uid == 0 || gid == 0 {
        return Err(Error::new(
            ErrorKind::PermissionDenied,
            "Ollama service account must not use root UID or GID",
        ));
    }

    Ok((uid, gid))
}

fn restricted_ollama_service_command(
    command: &std::process::Command,
    uid: u32,
    gid: u32,
) -> std::process::Command {
    let original = restricted_ollama_command(command);
    let mut restricted = std::process::Command::new("/usr/bin/setpriv");

    restricted
        .arg("--reuid")
        .arg(uid.to_string())
        .arg("--regid")
        .arg(gid.to_string())
        .arg("--clear-groups")
        .arg("--no-new-privs")
        .arg(original.get_program())
        .args(original.get_args());

    restricted.env_clear();
    restricted.env("PATH", "/usr/local/bin:/usr/bin:/bin");
    restricted.env("HOME", "/var/lib/ollama");
    restricted.env("OLLAMA_MODELS", "/var/lib/ollama/models");
    restricted.env("OLLAMA_HOST", "127.0.0.1:11434");

    restricted
}

fn open_private_ollama_modelfile(
    directory: &std::path::Path,
    modelfile: &std::path::Path,
) -> std::io::Result<(std::os::fd::OwnedFd, std::os::fd::OwnedFd)> {
    use rustix::fs::{Mode, OFlags, fstat, openat};
    use std::io::{Error, ErrorKind};

    if modelfile.parent() != Some(directory)
        || modelfile.file_name() != Some(std::ffi::OsStr::new("Modelfile"))
    {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            "Unexpected temporary Ollama Modelfile path",
        ));
    }

    let directory_fd = openat(
        rustix::fs::CWD,
        directory,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )?;

    let directory_stat = fstat(&directory_fd)?;

    if directory_stat.st_uid != 0 || directory_stat.st_mode & 0o777 != 0o700 {
        return Err(Error::new(
            ErrorKind::PermissionDenied,
            "Temporary Ollama directory must be root-owned and private",
        ));
    }

    let file_fd = openat(
        &directory_fd,
        "Modelfile",
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )?;

    let file_stat = fstat(&file_fd)?;

    if file_stat.st_uid != 0
        || file_stat.st_mode & 0o170000 != 0o100000
        || file_stat.st_nlink != 1
        || file_stat.st_mode & 0o022 != 0
    {
        return Err(Error::new(
            ErrorKind::PermissionDenied,
            "Temporary Ollama Modelfile has unsafe ownership or permissions",
        ));
    }

    Ok((directory_fd, file_fd))
}

#[derive(Default)]
struct RestrictedOllamaRunner {
    prepared_modelfile: Option<std::path::PathBuf>,
}

impl engine::OllamaCommandRunner for RestrictedOllamaRunner {
    fn prepare_modelfile(
        &mut self,
        directory: &tempfile::TempDir,
        modelfile: &std::path::Path,
    ) -> std::io::Result<()> {
        use rustix::fs::{Mode, fchmod, fchown};
        use rustix::process::{Gid, Uid};

        let (_, gid) = ollama_service_identity()?;
        let (directory_fd, file_fd) = open_private_ollama_modelfile(directory.path(), modelfile)?;

        let group = Some(Gid::from_raw(gid));

        // Keep both objects root-owned. Ollama may traverse the directory
        // and read the Modelfile, but may modify neither.
        fchown(&file_fd, Some(Uid::from_raw(0)), group)?;
        fchmod(&file_fd, Mode::from_raw_mode(0o640))?;
        fchown(&directory_fd, Some(Uid::from_raw(0)), group)?;
        fchmod(&directory_fd, Mode::from_raw_mode(0o710))?;

        self.prepared_modelfile = Some(modelfile.to_path_buf());

        Ok(())
    }

    fn status(&mut self, command: &mut std::process::Command) -> std::io::Result<()> {
        validate_ollama_create_command(command)
            .map_err(|message| std::io::Error::new(std::io::ErrorKind::InvalidInput, message))?;

        let prepared = self.prepared_modelfile.take().ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "Ollama Modelfile was not prepared",
            )
        })?;

        let arguments: Vec<_> = command.get_args().collect();

        if std::path::Path::new(arguments[2]) != prepared {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "Ollama command does not reference the prepared Modelfile",
            ));
        }

        let (uid, gid) = ollama_service_identity()?;

        let mut restricted = restricted_ollama_service_command(command, uid, gid);

        engine::OllamaCommandRunner::status(
            &mut engine::ProcessOllamaCommandRunner,
            &mut restricted,
        )
    }
}

#[allow(dead_code)]
fn run_restricted_model_import(
    requested_device: &str,
    filename: &str,
    model_name: &str,
) -> ExitCode {
    use std::path::Path;

    use inspector::ContentVolumeInspector;

    if let Err(error) = validate_privileged_model_destination() {
        eprintln!("Model import destination security check failed: {error}");
        return ExitCode::FAILURE;
    }

    let volume_inspector = inspector::LinuxContentVolumeInspector::new()
        .with_command("/usr/bin/lsblk")
        .with_findmnt_command("/usr/bin/findmnt")
        .with_post_install_discovery()
        .with_restricted_environment();

    let volumes = match volume_inspector.inspect() {
        Ok(volumes) => volumes,
        Err(error) => {
            eprintln!("Error discovering model source volumes: {error}");
            return ExitCode::FAILURE;
        }
    };

    let volume = match application::select_post_install_model_volume(
        &volumes,
        Path::new(requested_device),
    ) {
        Ok(volume) => volume,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };

    let mounter = inspector::LinuxContentVolumeMounter::new()
        .with_command("/usr/bin/mount")
        .with_umount_command("/usr/bin/umount")
        .with_lsblk_command("/usr/bin/lsblk")
        .with_restricted_environment();
    let mut executor =
        engine::OllamaModelRealizationExecutor::new(RestrictedOllamaRunner::default());

    match application::import_and_realize_selected_model_on_volume_with(
        &mounter,
        &volume,
        Path::new(filename),
        model_name,
        Path::new("/"),
        &mut executor,
    ) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

#[allow(dead_code)]
fn validate_privileged_model_destination() -> Result<(), String> {
    validate_privileged_directory(std::path::Path::new("/var/lib/daia/content"))
}

fn validate_privileged_directory(path: &std::path::Path) -> Result<(), String> {
    use rustix::fs::{Mode, OFlags};
    use std::path::Component;

    if !path.is_absolute() {
        return Err("Model destination must be absolute".to_owned());
    }

    let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC;

    let mut directory = rustix::fs::open("/", flags, Mode::empty())
        .map_err(|error| format!("Cannot open model destination root: {error}"))?;

    let root_metadata = rustix::fs::fstat(&directory)
        .map_err(|error| format!("Cannot inspect model destination root: {error}"))?;

    if root_metadata.st_uid != 0 || root_metadata.st_mode & 0o022 != 0 {
        return Err("Unsafe model destination directory: /".to_owned());
    }

    let mut current = String::new();

    for component in path.components() {
        let Component::Normal(name) = component else {
            if matches!(component, Component::RootDir) {
                continue;
            }
            return Err("Invalid model destination path component".to_owned());
        };

        current.push('/');
        current.push_str(&name.to_string_lossy());

        directory = rustix::fs::openat(&directory, component, flags, Mode::empty())
            .map_err(|error| format!("Cannot open model destination {current}: {error}"))?;

        let metadata = rustix::fs::fstat(&directory)
            .map_err(|error| format!("Cannot inspect model destination {current}: {error}"))?;

        if metadata.st_uid != 0 || metadata.st_mode & 0o022 != 0 {
            return Err(format!("Unsafe model destination directory: {current}"));
        }
    }

    Ok(())
}

fn is_root() -> bool {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|status| {
            status
                .lines()
                .find(|line| line.starts_with("Uid:"))
                .and_then(|line| line.split_whitespace().nth(2))
                .and_then(|value| value.parse::<u32>().ok())
        })
        == Some(0)
}

fn validate_argument_count(count: usize) -> Result<(), &'static str> {
    if count != 3 {
        return Err("Usage: daia-model-import <device> <filename> <model-name>");
    }

    Ok(())
}

fn validate_model_name(name: &str) -> Result<(), &'static str> {
    let mut characters = name.chars();

    if !characters
        .next()
        .is_some_and(|character| character.is_ascii_alphanumeric())
        || !characters.all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-')
        })
    {
        return Err("Invalid model name");
    }

    Ok(())
}

fn validate_gguf_filename(filename: &str) -> Result<(), &'static str> {
    if filename.is_empty()
        || filename == "."
        || filename == ".."
        || filename.contains('/')
        || filename.contains('\\')
        || filename.chars().any(char::is_control)
        || !filename.to_ascii_lowercase().ends_with(".gguf")
    {
        return Err("Invalid GGUF filename");
    }

    Ok(())
}

fn validate_device_path(device: &str) -> Result<(), &'static str> {
    let Some(name) = device.strip_prefix("/dev/") else {
        return Err("Invalid device path");
    };

    if name.is_empty()
        || !name.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
        })
        || name == "."
        || name == ".."
    {
        return Err("Invalid device path");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        validate_argument_count, validate_device_path, validate_gguf_filename, validate_model_name,
    };

    #[test]
    fn privileged_directory_rejects_world_writable_destination() {
        use std::os::unix::fs::PermissionsExt;

        let temporary = tempfile::tempdir().expect("create temporary directory");
        let destination = temporary.path().join("unsafe");

        std::fs::create_dir(&destination).expect("create destination");
        std::fs::set_permissions(&destination, std::fs::Permissions::from_mode(0o777))
            .expect("set unsafe permissions");

        let result = super::validate_privileged_directory(&destination);

        assert!(
            result.is_err(),
            "privileged import must reject an unsafe destination"
        );
    }

    #[test]
    fn rejects_unsafe_model_names() {
        for name in [
            "",
            "../model",
            "model name",
            "model;rm",
            "-model",
            "model/tag",
        ] {
            assert!(validate_model_name(name).is_err(), "{name:?}");
        }

        for name in ["qwen2.5-local", "llama3", "model_v2"] {
            assert!(validate_model_name(name).is_ok(), "{name:?}");
        }
    }

    #[test]
    fn rejects_unsafe_gguf_filenames() {
        for filename in [
            "",
            ".",
            "..",
            "../model.gguf",
            "/tmp/model.gguf",
            "folder/model.gguf",
            "folder\\\\model.gguf",
            "model.txt",
            "model.gguf\\n",
        ] {
            assert!(validate_gguf_filename(filename).is_err(), "{filename:?}");
        }

        for filename in ["model.gguf", "qwen2.5-q4_0.gguf", "MODEL.GGUF"] {
            assert!(validate_gguf_filename(filename).is_ok(), "{filename:?}");
        }
    }

    #[test]
    fn validates_device_paths() {
        for device in [
            "",
            "sdb1",
            "/tmp/sdb1",
            "/dev/",
            "/dev/../sda",
            "/dev/disk/by-id/example",
            "/dev/.",
            "/dev/..",
        ] {
            assert!(validate_device_path(device).is_err(), "{device:?}");
        }

        for device in ["/dev/sdb1", "/dev/nvme0n1p2", "/dev/mmcblk0p1"] {
            assert!(validate_device_path(device).is_ok(), "{device:?}");
        }
    }

    #[test]
    fn ollama_identity_resolves_only_valid_named_service_account() {
        let parse = super::parse_ollama_service_identity;

        let valid = "root:x:0:0:root:/root:/bin/bash\\n\\
ollama:x:989:989::/var/lib/ollama:/usr/sbin/nologin\\n";

        assert_eq!(parse(valid).unwrap(), (989, 989));

        for invalid in [
            "root:x:0:0:root:/root:/bin/bash\\n",
            "ollama:x:0:989::/var/lib/ollama:/usr/sbin/nologin\\n",
            "ollama:x:989:0::/var/lib/ollama:/usr/sbin/nologin\\n",
            "ollama:x:invalid:989::/var/lib/ollama:/usr/sbin/nologin\\n",
            "ollama:x:989:989::/var/lib/ollama:/usr/sbin/nologin\\n\\
ollama:x:990:990::/var/lib/ollama:/usr/sbin/nologin\\n",
        ] {
            assert!(parse(invalid).is_err(), "{invalid:?}");
        }
    }

    #[test]
    fn private_ollama_modelfile_rejects_unexpected_path() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let unexpected = directory.path().join("unexpected");

        let error = super::open_private_ollama_modelfile(directory.path(), &unexpected)
            .expect_err("unexpected Modelfile path must be rejected");

        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
    }

    #[cfg(unix)]
    #[test]
    fn private_ollama_modelfile_rejects_symlink() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().expect("temporary directory");
        let target = directory.path().join("target");
        let modelfile = directory.path().join("Modelfile");

        std::fs::write(&target, "FROM /tmp/example.gguf\\n").expect("write target");
        symlink(&target, &modelfile).expect("create symlink");

        assert!(
            super::open_private_ollama_modelfile(directory.path(), &modelfile,).is_err(),
            "symbolic-link Modelfile must be rejected"
        );
    }

    #[cfg(unix)]
    #[test]
    fn private_ollama_modelfile_rejects_permissive_directory() {
        use std::os::unix::fs::PermissionsExt;

        let directory = tempfile::tempdir().expect("temporary directory");
        let modelfile = directory.path().join("Modelfile");

        std::fs::write(&modelfile, "FROM /tmp/example.gguf\\n").expect("write Modelfile");

        std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o777))
            .expect("set permissive directory permissions");

        assert!(
            super::open_private_ollama_modelfile(directory.path(), &modelfile,).is_err(),
            "permissive temporary directory must be rejected"
        );
    }

    #[cfg(unix)]
    #[test]
    fn private_ollama_modelfile_rejects_hard_link() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let modelfile = directory.path().join("Modelfile");
        let second_link = directory.path().join("second-link");

        std::fs::write(&modelfile, "FROM /tmp/example.gguf\\n").expect("write Modelfile");

        std::fs::hard_link(&modelfile, &second_link).expect("create hard link");

        assert!(
            super::open_private_ollama_modelfile(directory.path(), &modelfile,).is_err(),
            "hard-linked Modelfile must be rejected"
        );
    }

    #[test]
    #[ignore = "requires root and the local Ollama service account"]
    fn restricted_runner_prepares_readonly_modelfile_for_ollama() {
        use engine::OllamaCommandRunner;
        use std::os::unix::fs::{MetadataExt, PermissionsExt};

        assert_eq!(rustix::process::geteuid().as_raw(), 0);

        let (uid, gid) = super::ollama_service_identity().expect("resolve Ollama identity");

        let directory = tempfile::Builder::new()
            .permissions(std::fs::Permissions::from_mode(0o700))
            .tempdir_in("/tmp")
            .expect("create private temporary directory");

        let modelfile = directory.path().join("Modelfile");
        let content = "FROM \"/tmp/example.gguf\"\n";

        std::fs::write(&modelfile, content).expect("write Modelfile");

        let mut runner = super::RestrictedOllamaRunner::default();

        runner
            .prepare_modelfile(&directory, &modelfile)
            .expect("prepare Modelfile");

        let directory_metadata = std::fs::metadata(directory.path()).expect("directory metadata");
        let file_metadata = std::fs::metadata(&modelfile).expect("Modelfile metadata");

        assert_eq!(directory_metadata.uid(), 0);
        assert_eq!(directory_metadata.gid(), gid);
        assert_eq!(directory_metadata.permissions().mode() & 0o777, 0o710);

        assert_eq!(file_metadata.uid(), 0);
        assert_eq!(file_metadata.gid(), gid);
        assert_eq!(file_metadata.permissions().mode() & 0o777, 0o640);

        assert_eq!(
            runner.prepared_modelfile.as_deref(),
            Some(modelfile.as_path())
        );

        let output = std::process::Command::new("/usr/bin/setpriv")
            .args([
                "--reuid",
                &uid.to_string(),
                "--regid",
                &gid.to_string(),
                "--clear-groups",
                "--no-new-privs",
                "/usr/bin/cat",
            ])
            .arg(&modelfile)
            .output()
            .expect("execute service-account read");

        assert!(
            output.status.success(),
            "Ollama read failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        assert_eq!(output.stdout, content.as_bytes());

        let temporary_path = directory.path().to_path_buf();
        drop(directory);

        assert!(
            !temporary_path.exists(),
            "temporary Modelfile directory must be removed"
        );
    }

    #[test]
    fn restricted_runner_rejects_different_prepared_modelfile() {
        use engine::OllamaCommandRunner;

        let mut runner = super::RestrictedOllamaRunner {
            prepared_modelfile: Some(std::path::PathBuf::from("/tmp/prepared/Modelfile")),
        };

        let mut command = std::process::Command::new("ollama");
        command.args(["create", "-f", "/tmp/different/Modelfile", "qwen2.5-local"]);

        let error = runner
            .status(&mut command)
            .expect_err("different Modelfile must not execute");

        assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
        assert!(
            error
                .to_string()
                .contains("does not reference the prepared Modelfile"),
            "{error}"
        );
        assert!(
            runner.prepared_modelfile.is_none(),
            "prepared state must be consumed even on mismatch"
        );
    }

    #[test]
    fn restricted_runner_rejects_unprepared_modelfile() {
        use engine::OllamaCommandRunner;

        let mut runner = super::RestrictedOllamaRunner::default();
        let mut command = std::process::Command::new("ollama");

        command.args(["create", "-f", "/tmp/unprepared/Modelfile", "qwen2.5-local"]);

        let error = runner
            .status(&mut command)
            .expect_err("unprepared Modelfile must not execute");

        assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
    }

    #[test]
    fn restricted_ollama_service_command_drops_privileges() {
        let mut original = std::process::Command::new("ollama");
        original.args(["create", "-f", "/tmp/example/Modelfile", "qwen2.5-local"]);

        let restricted = super::restricted_ollama_service_command(&original, 997, 986);

        assert_eq!(
            restricted.get_program(),
            std::ffi::OsStr::new("/usr/bin/setpriv")
        );

        let arguments: Vec<_> = restricted
            .get_args()
            .map(|argument| argument.to_string_lossy().into_owned())
            .collect();

        assert_eq!(
            arguments,
            vec![
                "--reuid",
                "997",
                "--regid",
                "986",
                "--clear-groups",
                "--no-new-privs",
                "/usr/local/bin/ollama",
                "create",
                "-f",
                "/tmp/example/Modelfile",
                "qwen2.5-local",
            ]
        );
    }

    #[test]
    fn restricted_ollama_command_uses_fixed_executable() {
        use std::ffi::OsStr;
        use std::process::Command;

        let mut original = Command::new("ollama");
        original.args(["create", "-f", "/tmp/Modelfile", "test-model"]);

        let restricted = super::restricted_ollama_command(&original);

        assert_eq!(
            restricted.get_program(),
            OsStr::new("/usr/local/bin/ollama")
        );
        assert_eq!(
            restricted.get_args().collect::<Vec<_>>(),
            original.get_args().collect::<Vec<_>>()
        );

        let environment = restricted.get_envs().collect::<Vec<_>>();

        assert_eq!(environment.len(), 4);
        assert!(environment.iter().any(|(key, value)| {
            *key == "HOME" && *value == Some(OsStr::new("/var/lib/ollama"))
        }));
        assert!(environment.iter().any(|(key, value)| {
            *key == "OLLAMA_MODELS" && *value == Some(OsStr::new("/var/lib/ollama/models"))
        }));
        assert!(environment.iter().any(|(key, value)| {
            *key == "PATH" && *value == Some(OsStr::new("/usr/local/bin:/usr/bin:/bin"))
        }));
        assert!(environment.iter().any(|(key, value)| {
            *key == "OLLAMA_HOST" && *value == Some(OsStr::new("127.0.0.1:11434"))
        }));
    }

    #[test]
    fn restricted_runner_rejects_unexpected_commands() {
        use std::process::Command;

        let mut valid = Command::new("ollama");
        valid.args(["create", "-f", "/tmp/Modelfile", "test-model"]);
        assert!(super::validate_ollama_create_command(&valid).is_ok());

        for arguments in [
            vec!["rm", "test-model"],
            vec!["pull", "test-model"],
            vec!["create", "-f", "/tmp/Modelfile", "-unsafe"],
            vec!["create", "-f", "/tmp/Modelfile"],
        ] {
            let mut command = Command::new("ollama");
            command.args(arguments);
            assert!(super::validate_ollama_create_command(&command).is_err());
        }

        let mut unexpected = Command::new("/bin/sh");
        unexpected.args(["-c", "true"]);
        assert!(super::validate_ollama_create_command(&unexpected).is_err());
    }

    #[test]
    fn restricted_runner_refuses_unexpected_command_without_execution() {
        use engine::OllamaCommandRunner;
        use std::io::ErrorKind;
        use std::process::Command;

        let mut runner = super::RestrictedOllamaRunner::default();
        let mut command = Command::new("/bin/sh");
        command.args(["-c", "echo should-not-run"]);

        let error = runner
            .status(&mut command)
            .expect_err("unexpected executable must be rejected");

        assert_eq!(error.kind(), ErrorKind::InvalidInput);
    }

    #[test]
    fn recognizes_only_explicit_model_listing_requests() {
        use std::ffi::OsString;

        let args =
            |values: &[&str]| -> Vec<OsString> { values.iter().map(OsString::from).collect() };

        assert!(super::is_model_list_request(&args(&[
            "list-models",
            "/dev/sda1",
        ])));

        assert!(!super::is_model_list_request(&args(&[])));
        assert!(!super::is_model_list_request(&args(&["list-models"])));
        assert!(!super::is_model_list_request(&args(&[
            "list-models",
            "/dev/sda1",
            "extra",
        ])));
        assert!(!super::is_model_list_request(&args(&[
            "/dev/sda1",
            "model.gguf",
            "model-name",
        ])));
    }

    #[test]
    fn recognizes_only_explicit_model_recovery_requests() {
        use std::ffi::OsString;

        let args =
            |values: &[&str]| -> Vec<OsString> { values.iter().map(OsString::from).collect() };

        assert!(super::is_model_resume_request(&args(&[
            "resume-model",
            "recovery-model",
        ])));

        assert!(!super::is_model_resume_request(&args(&[])));
        assert!(!super::is_model_resume_request(&args(&["resume-model"])));
        assert!(!super::is_model_resume_request(&args(&[
            "resume-model",
            "recovery-model",
            "extra",
        ])));
        assert!(!super::is_model_resume_request(&args(&[
            "list-models",
            "/dev/sda1",
        ])));
        assert!(!super::is_model_resume_request(&args(&[
            "/dev/sda1",
            "model.gguf",
            "recovery-model",
        ])));
    }

    #[test]
    fn requires_exactly_three_arguments() {
        for count in [0, 1, 2, 4, 5] {
            assert!(validate_argument_count(count).is_err());
        }

        assert!(validate_argument_count(3).is_ok());
    }
}
