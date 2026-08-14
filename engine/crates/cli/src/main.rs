//! DAIA command-line interface.

use engine::{BootstrapConfig, BuildContext, Engine};
use inspector::{DebianIsoInspector, IsoInspector, LinuxStorageInspector};
use model::{Capability, Plan};
use registry::PackageRepository;
use std::env;
use std::path::PathBuf;
use std::process::ExitCode;
use wizard::WizardState;
mod appliance_profile_repository;
mod provider_registry;
mod wizard;

struct BuildOptions {
    rootfs: PathBuf,
    source_iso: PathBuf,
    work_directory: PathBuf,
    output_iso: PathBuf,
}

fn main() -> ExitCode {
    let arguments = env::args().skip(1).collect::<Vec<_>>();

    run(&arguments)
}

fn run(arguments: &[String]) -> ExitCode {
    match arguments {
        [command] if command == "wizard" => run_wizard(),
        [capability_name] => run_plan(capability_name),
        [command, capability_name] if command == "plan" => run_plan(capability_name),
        [command, profile_name] if command == "plan-profile" => run_profile_plan(profile_name),
        [
            command,
            capability_name,
            rootfs,
            source_iso,
            work_directory,
            output_iso,
        ] if command == "build-iso" => run_iso_build(
            capability_name,
            &BuildOptions {
                rootfs: PathBuf::from(rootfs),
                source_iso: PathBuf::from(source_iso),
                work_directory: PathBuf::from(work_directory),
                output_iso: PathBuf::from(output_iso),
            },
        ),
        _ => {
            print_usage();
            ExitCode::FAILURE
        }
    }
}

fn run_profile_plan(profile_name: &str) -> ExitCode {
    let Some(engine) = load_engine() else {
        return ExitCode::FAILURE;
    };

    let repository = match appliance_profile_repository::load() {
        Ok(repository) => repository,
        Err(error) => {
            eprintln!("Error loading appliance profile repository: {error}");
            return ExitCode::FAILURE;
        }
    };

    let Some(profile) = repository.profile(profile_name) else {
        eprintln!("Error: appliance profile \"{profile_name}\" not found");
        return ExitCode::FAILURE;
    };

    match engine.plan_profile(profile) {
        Ok(plans) => {
            for (index, plan) in plans.iter().enumerate() {
                if index > 0 {
                    println!();
                }

                print_plan(plan);
            }

            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("Error: {error}");
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

fn run_iso_build(capability_name: &str, options: &BuildOptions) -> ExitCode {
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

    let context = match create_build_context(options) {
        Ok(context) => context,
        Err(error) => {
            eprintln!("Error preparing build context: {error}");
            return ExitCode::FAILURE;
        }
    };
    match engine.build_iso(
        &Capability::new(capability_name),
        &context,
        &package_repository,
    ) {
        Ok(plan) => {
            print_plan(&plan);
            println!();
            println!("ISO image  : {}", options.output_iso.display());
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("Error building ISO: {error:?}");
            ExitCode::FAILURE
        }
    }
}
fn create_build_context(
    options: &BuildOptions,
) -> Result<BuildContext, Box<dyn std::error::Error>> {
    let inspector = DebianIsoInspector::new();

    let metadata = inspector.inspect(&options.source_iso)?;

    let bootstrap = BootstrapConfig::from_iso_metadata(&metadata);

    Ok(BuildContext::new(
        options.rootfs.clone(),
        options.source_iso.clone(),
        options.work_directory.clone(),
        options.output_iso.clone(),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../registry/assets"),
        bootstrap,
    ))
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
    eprintln!("    daia plan-profile <profile>");
    eprintln!("    daia plan-profile desktop");
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
fn run_wizard() -> ExitCode {
    let Some(engine) = load_engine() else {
        return ExitCode::FAILURE;
    };

    let inspector = LinuxStorageInspector::new();

    let storage = match engine.discover_storage(&inspector) {
        Ok(storage) => storage,
        Err(error) => {
            eprintln!("Error discovering storage: {error}");
            return ExitCode::FAILURE;
        }
    };

    let mut state = WizardState::new();
    state.set_discovered_storage(storage);

    println!("DAIA Wizard");
    println!();
    println!("Storage devices:");

    for storage in state.discovered_storage() {
        println!(
            "  {}  {}  {}",
            storage.kind(),
            storage.id(),
            storage.device_path().display()
        );
    }

    ExitCode::SUCCESS
}
#[cfg(test)]
mod tests {
    use super::{BuildOptions, run};
    use std::path::PathBuf;
    use std::process::ExitCode;

    #[test]
    fn plans_repository_appliance_profile() {
        let arguments = vec!["plan-profile".to_owned(), "desktop".to_owned()];

        let result = run(&arguments);

        assert_eq!(result, ExitCode::SUCCESS);
    }
    #[test]
    fn rejects_unknown_appliance_profile() {
        let arguments = vec!["plan-profile".to_owned(), "does-not-exist".to_owned()];

        let result = run(&arguments);

        assert_eq!(result, ExitCode::FAILURE);
    }
    #[test]
    fn rejects_unknown_command() {
        let arguments = vec!["unknown".to_owned()];

        let result = run(&arguments);

        assert_eq!(result, ExitCode::FAILURE);
    }
    #[test]
    fn rejects_incomplete_iso_build_arguments() {
        let arguments = vec!["build-iso".to_owned(), "desktop".to_owned()];

        let result = run(&arguments);

        assert_eq!(result, ExitCode::FAILURE);
    }
    #[test]
    fn build_options_store_paths() {
        let options = BuildOptions {
            rootfs: PathBuf::from("rootfs"),
            source_iso: PathBuf::from("source.iso"),
            work_directory: PathBuf::from("work"),
            output_iso: PathBuf::from("output.iso"),
        };

        assert_eq!(options.rootfs, PathBuf::from("rootfs"));
        assert_eq!(options.source_iso, PathBuf::from("source.iso"));
        assert_eq!(options.work_directory, PathBuf::from("work"));
        assert_eq!(options.output_iso, PathBuf::from("output.iso"));
    }
}
