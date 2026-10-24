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
        engine::OllamaModelRealizationExecutor::new(engine::RestrictedOllamaRunner::default());

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
        engine::OllamaModelRealizationExecutor::new(engine::RestrictedOllamaRunner::default());

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
