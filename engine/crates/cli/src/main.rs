//! DAIA command-line interface.

use engine::{
    BootstrapConfig, BuildContext, DryRunInstallationExecutor, Engine, InstallationOperation,
};
use inspector::{DebianIsoInspector, IsoInspector, LinuxStorageInspector};
use model::{Capability, Plan};
use registry::PackageRepository;
use std::env;
use std::io::{self, Write};
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

fn daia_data_directory() -> PathBuf {
    let installed = PathBuf::from("/usr/share/daia");

    if installed.is_dir() {
        installed
    } else {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../registry")
    }
}

fn package_manifest_directory() -> PathBuf {
    daia_data_directory().join("package-manifests")
}

fn asset_directory() -> PathBuf {
    daia_data_directory().join("assets")
}

fn main() -> ExitCode {
    let arguments = env::args().skip(1).collect::<Vec<_>>();

    run(&arguments)
}

fn run(arguments: &[String]) -> ExitCode {
    match arguments {
        [command] if command == "wizard" => run_wizard(),
        [command] if command == "install" => run_install(),
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

    let package_repository = match PackageRepository::from_directory(package_manifest_directory()) {
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
        asset_directory(),
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
    eprintln!("    daia install");
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

fn select_appliance_profile(state: &mut WizardState) -> Result<(), String> {
    let repository = appliance_profile_repository::load()
        .map_err(|error| format!("Error loading appliance profiles: {error}"))?;

    println!("Appliance profiles:");

    for (index, profile) in repository.profiles().iter().enumerate() {
        println!(
            "  {}. {} - {}",
            index + 1,
            profile.name(),
            profile.description()
        );
    }

    print!(
        "Select appliance profile [1-{}]: ",
        repository.profiles().len()
    );

    io::stdout()
        .flush()
        .map_err(|error| format!("Error writing prompt: {error}"))?;

    let mut input = String::new();

    io::stdin()
        .read_line(&mut input)
        .map_err(|error| format!("Error reading selection: {error}"))?;

    let selection = input
        .trim()
        .parse::<usize>()
        .ok()
        .filter(|selection| (1..=repository.profiles().len()).contains(selection))
        .ok_or_else(|| "Error: invalid appliance profile selection".to_owned())?;

    let selected_profile = &repository.profiles()[selection - 1];

    state.set_profile_name(selected_profile.name());

    println!(
        "Selected profile: {}",
        state
            .profile_name()
            .expect("validated profile selection should exist")
    );
    println!();

    Ok(())
}

fn select_storage(state: &mut WizardState) -> Result<(), String> {
    let selectable = state.selectable_storage().collect::<Vec<_>>();

    println!("Storage devices:");

    if selectable.is_empty() {
        return Err("No selectable storage devices found.".to_owned());
    }

    for (index, storage) in selectable.iter().enumerate() {
        println!(
            "  {}. {}  {}  {}",
            index + 1,
            storage.kind(),
            storage.id(),
            storage.device_path().display()
        );
    }

    print!("Select storage target [1-{}]: ", selectable.len());

    io::stdout()
        .flush()
        .map_err(|error| format!("Error writing prompt: {error}"))?;

    let mut input = String::new();

    io::stdin()
        .read_line(&mut input)
        .map_err(|error| format!("Error reading selection: {error}"))?;

    let selection = input
        .trim()
        .parse::<usize>()
        .ok()
        .filter(|selection| (1..=selectable.len()).contains(selection))
        .ok_or_else(|| "Error: invalid storage selection".to_owned())?;

    let selected_id = selectable[selection - 1].id().clone();

    state.select_storage(selected_id);

    println!(
        "Selected storage: {}",
        state
            .selected_storage()
            .expect("validated storage selection should exist")
    );

    Ok(())
}
fn review_wizard_state(state: &WizardState) {
    println!();
    println!("Review:");
    println!(
        "  Profile : {}",
        state
            .profile_name()
            .expect("profile should be selected before review")
    );
    println!(
        "  Storage : {}",
        state
            .selected_storage()
            .expect("storage should be selected before review")
    );
}

fn confirm_wizard_state() -> Result<bool, String> {
    print!("Continue with this configuration? [y/N]: ");

    io::stdout()
        .flush()
        .map_err(|error| format!("Error writing prompt: {error}"))?;

    let mut input = String::new();

    io::stdin()
        .read_line(&mut input)
        .map_err(|error| format!("Error reading confirmation: {error}"))?;

    Ok(matches!(
        input.trim().to_ascii_lowercase().as_str(),
        "y" | "yes"
    ))
}

fn prepare_wizard_installation(
    engine: &Engine,
    config: &wizard::WizardConfig,
) -> Result<engine::PreparedInstallation, String> {
    let repository = appliance_profile_repository::load()
        .map_err(|error| format!("Error loading appliance profiles: {error}"))?;

    let profile = config.profile(&repository).ok_or_else(|| {
        format!(
            "Error: selected appliance profile \"{}\" no longer exists",
            config.profile_name()
        )
    })?;

    let intent = config.installation_intent();

    let storage = engine
        .discover_storage(&LinuxStorageInspector::new())
        .map_err(|error| format!("Error discovering storage: {error}"))?;

    engine.prepare_installation(intent, profile, &storage)
}

fn installation_operation_name(operation: &InstallationOperation) -> String {
    match operation {
        InstallationOperation::PrepareDisk { storage_id, .. } => {
            format!("Prepare disk {storage_id}")
        }

        InstallationOperation::PartitionDisk { device_path, .. } => {
            format!("Partition disk {}", device_path.display())
        }

        InstallationOperation::CreateFilesystems { partitions, .. } => {
            format!("Create filesystems for {} partitions", partitions.len())
        }

        InstallationOperation::MountFilesystems { mounts, .. } => {
            format!("Mount {} filesystems", mounts.len())
        }

        InstallationOperation::BootstrapSystem { root, .. } => {
            format!("Bootstrap system at {}", root.display())
        }

        InstallationOperation::ApplyPlans { plans } => {
            if plans.len() == 1 {
                "Apply 1 appliance plan".to_owned()
            } else {
                format!("Apply {} appliance plans", plans.len())
            }
        }

        InstallationOperation::ConfigureFstab { .. } => "Configure filesystem table".to_owned(),

        InstallationOperation::PrepareTargetRuntime { root } => {
            format!("Prepare target runtime at {}", root.display())
        }

        InstallationOperation::InstallBootloader { root, .. } => {
            format!("Install bootloader in {}", root.display())
        }

        InstallationOperation::CleanupTargetRuntime { root } => {
            format!("Clean up target runtime at {}", root.display())
        }

        InstallationOperation::UnmountFilesystems { mounts } => {
            format!("Unmount {} installation filesystems", mounts.len())
        }
    }
}
fn print_installation_operations(executor: &DryRunInstallationExecutor) {
    let Some(plan) = executor.plan() else {
        return;
    };

    println!();
    println!("Planned installation operations:");

    for (index, operation) in plan.operations().iter().enumerate() {
        println!(
            "  {}. {}",
            index + 1,
            installation_operation_name(operation)
        );
    }
}

fn run_install() -> ExitCode {
    let Some(engine) = load_engine() else {
        return ExitCode::FAILURE;
    };

    let mut state = WizardState::new();

    println!("DAIA Installer");
    println!();

    if let Err(error) = select_appliance_profile(&mut state) {
        eprintln!("{error}");
        return ExitCode::FAILURE;
    }

    let inspector = LinuxStorageInspector::new();

    let storage = match engine.discover_storage(&inspector) {
        Ok(storage) => storage,
        Err(error) => {
            eprintln!("Error discovering storage: {error}");
            return ExitCode::FAILURE;
        }
    };

    state.set_discovered_storage(storage);

    if let Err(error) = select_storage(&mut state) {
        eprintln!("{error}");
        return ExitCode::FAILURE;
    }
    let Some(config) = state.into_config() else {
        eprintln!("Error: installer configuration is incomplete");
        return ExitCode::FAILURE;
    };

    let prepared = match prepare_wizard_installation(&engine, &config) {
        Ok(prepared) => prepared,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };
    println!();
    println!("WARNING: The selected target disk will be erased.");

    match confirm_wizard_state() {
        Ok(true) => {}
        Ok(false) => {
            println!("Installation cancelled.");
            return ExitCode::SUCCESS;
        }
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    }

    println!();
    println!("Installer preparation complete.");
    println!("Prepared {} appliance plan(s).", prepared.plans().len());
    println!("No disk changes have been made.");

    ExitCode::SUCCESS
}
fn run_wizard() -> ExitCode {
    let Some(engine) = load_engine() else {
        return ExitCode::FAILURE;
    };

    let mut state = WizardState::new();

    println!("DAIA Wizard");
    println!();

    if let Err(error) = select_appliance_profile(&mut state) {
        eprintln!("{error}");
        return ExitCode::FAILURE;
    }

    let inspector = LinuxStorageInspector::new();

    let storage = match engine.discover_storage(&inspector) {
        Ok(storage) => storage,
        Err(error) => {
            eprintln!("Error discovering storage: {error}");
            return ExitCode::FAILURE;
        }
    };

    state.set_discovered_storage(storage);

    let selectable = state.selectable_storage().collect::<Vec<_>>();

    println!("Storage devices:");

    if selectable.is_empty() {
        println!("  No selectable storage devices found.");
        return ExitCode::SUCCESS;
    }

    for (index, storage) in selectable.iter().enumerate() {
        println!(
            "  {}. {}  {}  {}",
            index + 1,
            storage.kind(),
            storage.id(),
            storage.device_path().display()
        );
    }

    print!("Select storage target [1-{}]: ", selectable.len());

    if let Err(error) = io::stdout().flush() {
        eprintln!("Error writing prompt: {error}");
        return ExitCode::FAILURE;
    }

    let mut input = String::new();

    if let Err(error) = io::stdin().read_line(&mut input) {
        eprintln!("Error reading selection: {error}");
        return ExitCode::FAILURE;
    }

    let selection = match input.trim().parse::<usize>() {
        Ok(selection) if (1..=selectable.len()).contains(&selection) => selection,
        _ => {
            eprintln!("Error: invalid storage selection");
            return ExitCode::FAILURE;
        }
    };

    let selected_id = selectable[selection - 1].id().clone();
    state.select_storage(selected_id);

    println!(
        "Selected storage: {}",
        state
            .selected_storage()
            .expect("validated storage selection should exist")
    );

    review_wizard_state(&state);

    match confirm_wizard_state() {
        Ok(true) => {
            let Some(config) = state.into_config() else {
                eprintln!("Error: wizard configuration is incomplete");
                return ExitCode::FAILURE;
            };
            let prepared = match prepare_wizard_installation(&engine, &config) {
                Ok(prepared) => prepared,
                Err(error) => {
                    eprintln!("{error}");
                    return ExitCode::FAILURE;
                }
            };

            let mut executor = DryRunInstallationExecutor::default();

            if let Err(error) = engine.execute_installation(&prepared, &mut executor) {
                match error {}
            }

            println!("Configuration confirmed.");
            println!();

            if let Some(summary) = executor.summary() {
                println!("{summary}");
            }
            print_installation_operations(&executor);
            ExitCode::SUCCESS
        }

        Ok(false) => {
            println!("Configuration cancelled.");
            ExitCode::SUCCESS
        }

        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
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
