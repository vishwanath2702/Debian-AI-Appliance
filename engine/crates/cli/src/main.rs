//! DAIA command-line interface.

use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

use engine::{BootstrapConfig, BuildContext, Engine};
use model::{Capability, Plan};
use registry::PackageRepository;

mod provider_registry;

fn main() -> ExitCode {
    let arguments = env::args().skip(1).collect::<Vec<_>>();

    run(arguments)
}

fn run(arguments: Vec<String>) -> ExitCode {
    match arguments.as_slice() {
        [capability_name] => run_plan(capability_name),
        [command, capability_name] if command == "plan" => run_plan(capability_name),
        [
            command,
            capability_name,
            rootfs,
            source_iso,
            work_directory,
            output_iso,
        ] if command == "build-iso" => run_iso_build(
            capability_name,
            PathBuf::from(rootfs),
            PathBuf::from(source_iso),
            PathBuf::from(work_directory),
            std::path::Path::new(output_iso),
        ),
        _ => {
            print_usage();
            ExitCode::FAILURE
        }
    }
}
fn run_plan(capability_name: &str) -> ExitCode {
    let Some(engine) = load_engine() else {
        return ExitCode::FAILURE;
    };

    match engine.plan(&Capability::new(capability_name)) {
        Ok(plan) => {
            print_plan(&plan);
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("Error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_iso_build(
    capability_name: &str,
    rootfs: PathBuf,
    source_iso: PathBuf,
    work_directory: PathBuf,
    output_iso: &std::path::Path,
) -> ExitCode {
    let Some(engine) = load_engine() else {
        return ExitCode::FAILURE;
    };

    let package_manifest_directory =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../registry/package-manifests");

    let package_repository = match PackageRepository::from_directory(package_manifest_directory) {
        Ok(repository) => repository,
        Err(error) => {
            eprintln!("Error loading package repository: {error}");
            return ExitCode::FAILURE;
        }
    };

    let context = create_build_context(rootfs, source_iso, work_directory, output_iso);
    match engine.build_iso(
        &Capability::new(capability_name),
        &context,
        &package_repository,
    ) {
        Ok(plan) => {
            print_plan(&plan);
            println!();
            println!("ISO image  : {}", output_iso.display());
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("Error building ISO: {error:?}");
            ExitCode::FAILURE
        }
    }
}
fn create_build_context(
    rootfs: PathBuf,
    source_iso: PathBuf,
    work_directory: PathBuf,
    output_iso: &std::path::Path,
) -> BuildContext {
    let bootstrap = BootstrapConfig::new(
        "bookworm",
        "amd64",
        "https://deb.debian.org/debian",
        vec!["main".to_owned()],
        "minbase",
    );

    BuildContext::new(
        rootfs,
        source_iso,
        work_directory,
        output_iso.to_path_buf(),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../registry/assets"),
        bootstrap,
    )
}
fn load_engine() -> Option<Engine> {
    match provider_registry::load() {
        Ok(registry) => Some(Engine::from_registry(registry)),
        Err(error) => {
            eprintln!("Error loading provider registry: {error}");
            None
        }
    }
}

fn print_plan(plan: &Plan) {
    println!("Capability : {}", plan.capability);
    println!("Provider   : {}", plan.provider);
    println!();
    println!("Plan:");

    for (index, step) in plan.steps.iter().enumerate() {
        println!("  {}. {}", index + 1, step);
    }
}

fn print_usage() {
    eprintln!("Usage:");
    eprintln!("    daia <capability>");
    eprintln!("    daia plan <capability>");
    eprintln!(
        "    daia build-iso <capability> <rootfs> <source-iso> <work-directory> <output-iso>"
    );
    eprintln!();
    eprintln!("Examples:");
    eprintln!("    daia desktop");
    eprintln!("    daia plan desktop");
    eprintln!("    daia build-iso desktop /rootfs source.iso /tmp/daia-work output.iso");
}
#[cfg(test)]
mod tests {
    use super::{create_build_context, run};
    use std::path::PathBuf;
    use std::process::ExitCode;

    #[test]
    fn rejects_unknown_command() {
        let result = run(vec!["unknown".to_owned()]);

        assert_eq!(result, ExitCode::FAILURE);
    }

    #[test]
    fn rejects_incomplete_iso_build_arguments() {
        let result = run(vec!["build-iso".to_owned(), "desktop".to_owned()]);

        assert_eq!(result, ExitCode::FAILURE);
    }
    #[test]
    fn creates_iso_build_context() {
        let context = create_build_context(
            PathBuf::from("rootfs"),
            PathBuf::from("source.iso"),
            PathBuf::from("work"),
            std::path::Path::new("output.iso"),
        );

        assert_eq!(context.rootfs(), PathBuf::from("rootfs"));
        assert_eq!(context.source_iso(), PathBuf::from("source.iso"));
        assert_eq!(context.work_directory(), PathBuf::from("work"));
        assert_eq!(context.output_iso(), PathBuf::from("output.iso"));
    }
}
