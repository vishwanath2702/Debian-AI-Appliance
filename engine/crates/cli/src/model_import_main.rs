//! Restricted DAIA model-import helper.
//!
//! Privileged imports require root authorization and restricted validation.

use std::env;
use std::process::ExitCode;

fn main() -> ExitCode {
    let arguments: Vec<_> = env::args_os().skip(1).collect();

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
    command.env("OLLAMA_HOST", "127.0.0.1:11434");

    command
}

#[derive(Default)]
struct RestrictedOllamaRunner;

impl engine::OllamaCommandRunner for RestrictedOllamaRunner {
    fn status(&mut self, command: &mut std::process::Command) -> std::io::Result<()> {
        validate_ollama_create_command(command)
            .map_err(|message| std::io::Error::new(std::io::ErrorKind::InvalidInput, message))?;

        let mut restricted = restricted_ollama_command(command);
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
    let mut executor = engine::OllamaModelRealizationExecutor::new(RestrictedOllamaRunner);

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

        assert_eq!(environment.len(), 2);
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

        let mut runner = super::RestrictedOllamaRunner;
        let mut command = Command::new("/bin/sh");
        command.args(["-c", "echo should-not-run"]);

        let error = runner
            .status(&mut command)
            .expect_err("unexpected executable must be rejected");

        assert_eq!(error.kind(), ErrorKind::InvalidInput);
    }

    #[test]
    fn requires_exactly_three_arguments() {
        for count in [0, 1, 2, 4, 5] {
            assert!(validate_argument_count(count).is_err());
        }

        assert!(validate_argument_count(3).is_ok());
    }
}
