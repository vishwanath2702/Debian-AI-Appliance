//! DAIA command-line interface.

use engine::{
    BootstrapConfig, BuildContext, DryRunContentImportOperationExecutor,
    DryRunInstallationExecutor, Engine, InstallationOperation, SystemInstallationOperationExecutor,
};
use inspector::{
    ContentInspector, DebianIsoInspector, IsoInspector, LinuxStorageInspector,
    LocalFilesystemContentInspector, StorageInspector,
};
use model::{Capability, Plan};
use registry::{ContentRepositoryRepository, PackageRepository};
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
fn content_repository_directory() -> PathBuf {
    daia_data_directory().join("content-repositories")
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

    let daia_binary = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/release/daia");
    if !daia_binary.is_file() {
        return Err(format!(
        "DAIA release binary not found at {}. Build it first with: cargo build --release --manifest-path crates/cli/Cargo.toml",
        daia_binary.display()
    )
    .into());
    }

    Ok(BuildContext::new(
        options.rootfs.clone(),
        options.source_iso.clone(),
        options.work_directory.clone(),
        options.output_iso.clone(),
        asset_directory(),
        bootstrap,
    )
    .with_daia_binary(daia_binary))
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

fn parse_appliance_profile_selection(input: &str, item_count: usize) -> Result<usize, String> {
    input
        .trim()
        .parse::<usize>()
        .ok()
        .filter(|selection| (1..=item_count).contains(selection))
        .ok_or_else(|| "Error: invalid appliance profile selection".to_owned())
}

fn load_wizard_appliance_profiles() -> Result<registry::ApplianceProfileRepository, String> {
    appliance_profile_repository::load()
        .map_err(|error| format!("Error loading appliance profiles: {error}"))
}

fn select_appliance_profile(
    state: &mut WizardState,
    repository: &registry::ApplianceProfileRepository,
) -> Result<(), String> {
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

    let selection = parse_appliance_profile_selection(&input, repository.profiles().len())?;
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

fn configure_wizard_appliance_profile(state: &mut WizardState) -> Result<(), String> {
    let repository = load_wizard_appliance_profiles()?;

    select_appliance_profile(state, &repository)
}

fn parse_content_repository_selection(input: &str, item_count: usize) -> Result<usize, String> {
    input
        .trim()
        .parse::<usize>()
        .ok()
        .filter(|selection| (1..=item_count).contains(selection))
        .ok_or_else(|| "Error: invalid content repository selection".to_owned())
}

fn select_content_repository(state: &mut WizardState) -> Result<(), String> {
    let repositories = state.content_repositories();

    if repositories.is_empty() {
        return Err("No content repositories found.".to_owned());
    }

    println!();
    println!("Content repositories:");

    for (index, repository) in repositories.iter().enumerate() {
        println!(
            "  {}. {}  {}",
            index + 1,
            repository.id(),
            repository.description()
        );
    }

    print!("Select content repository [1-{}]: ", repositories.len());

    io::stdout()
        .flush()
        .map_err(|error| format!("Error writing prompt: {error}"))?;

    let mut input = String::new();

    io::stdin()
        .read_line(&mut input)
        .map_err(|error| format!("Error reading selection: {error}"))?;

    let selection = parse_content_repository_selection(&input, repositories.len())?;
    let selected_id = repositories[selection - 1].id().clone();

    state.select_content_repository(selected_id);

    println!(
        "Selected content repository: {}",
        state
            .selected_content_repository()
            .expect("validated content repository selection should exist")
    );

    Ok(())
}

fn configure_wizard_content_repository(state: &mut WizardState) -> Result<(), String> {
    load_wizard_content_repositories(state)?;

    select_content_repository(state)
}

fn parse_external_content_selection(input: &str, item_count: usize) -> Result<Vec<usize>, String> {
    let selections: Vec<usize> = input
        .split_whitespace()
        .map(|value| {
            value
                .parse::<usize>()
                .ok()
                .filter(|selection| (1..=item_count).contains(selection))
                .ok_or_else(|| "Error: invalid external content selection".to_owned())
        })
        .collect::<Result<_, _>>()?;

    if selections.is_empty() {
        return Err("Error: invalid external content selection".to_owned());
    }

    let mut unique_selections = selections.clone();
    unique_selections.sort_unstable();
    unique_selections.dedup();

    if unique_selections.len() != selections.len() {
        return Err("Error: duplicate external content selection".to_owned());
    }

    Ok(selections)
}

fn select_external_content(state: &mut WizardState) -> Result<(), String> {
    let items = state.external_content_items();

    if items.is_empty() {
        return Ok(());
    }

    println!();
    println!("External content:");

    for (index, item) in items.iter().enumerate() {
        println!("  {}. {}  {}", index + 1, item.id(), item.path().display());
    }

    print!(
        "Select external content [1-{}, space-separated]: ",
        items.len()
    );
    io::stdout()
        .flush()
        .map_err(|error| format!("Error writing prompt: {error}"))?;

    let mut input = String::new();

    io::stdin()
        .read_line(&mut input)
        .map_err(|error| format!("Error reading selection: {error}"))?;

    let selections = parse_external_content_selection(&input, items.len())?;

    let selected_ids = selections
        .into_iter()
        .map(|selection| items[selection - 1].id().clone())
        .collect();

    state.select_external_content(selected_ids);
    println!(
        "Selected external content: {}",
        state
            .selected_external_content()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ")
    );
    Ok(())
}

fn discover_external_content<I>(
    engine: &Engine,
    state: &mut WizardState,
    inspector: &I,
) -> Result<(), String>
where
    I: ContentInspector,
{
    let selected_repository_id = state
        .selected_content_repository()
        .expect("content repository should be selected")
        .clone();

    let sources = state
        .content_repositories()
        .iter()
        .find(|repository| repository.id() == &selected_repository_id)
        .expect("selected content repository should exist")
        .sources()
        .to_vec();

    let mut discovered_content = Vec::new();

    for source in &sources {
        let mut discovered = engine
            .discover_content(source, inspector)
            .map_err(|error| {
                format!(
                    "Error discovering content from source \"{}\": {error}",
                    source.id()
                )
            })?;

        discovered_content.append(&mut discovered);
    }

    state.set_discovered_content(discovered_content);

    let mut external_content_items = Vec::new();

    for content in state.discovered_content() {
        let mut items = engine
            .external_content_items(content, inspector)
            .map_err(|error| {
                format!(
                    "Error enumerating external content from \"{}\": {error}",
                    content.path().display()
                )
            })?;

        external_content_items.append(&mut items);
    }

    state.set_external_content_items(external_content_items);

    Ok(())
}
fn configure_wizard_external_content<I>(
    engine: &Engine,
    state: &mut WizardState,
    inspector: &I,
) -> Result<(), String>
where
    I: ContentInspector,
{
    discover_external_content(engine, state, inspector)?;

    select_external_content(state)
}
fn parse_storage_selection(input: &str, item_count: usize) -> Result<usize, String> {
    input
        .trim()
        .parse::<usize>()
        .ok()
        .filter(|selection| (1..=item_count).contains(selection))
        .ok_or_else(|| "Error: invalid storage selection".to_owned())
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

    let selection = parse_storage_selection(&input, selectable.len())?;

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

fn configure_wizard_storage<I>(
    engine: &Engine,
    state: &mut WizardState,
    inspector: &I,
) -> Result<(), String>
where
    I: StorageInspector,
{
    discover_wizard_storage_with(engine, state, inspector)?;

    select_storage(state)
}

fn configure_wizard_state(engine: &Engine, state: &mut WizardState) -> Result<(), String> {
    configure_wizard_appliance_profile(state)?;
    configure_wizard_content_repository(state)?;

    let content_inspector = LocalFilesystemContentInspector::new();

    configure_wizard_external_content(engine, state, &content_inspector)?;

    let storage_inspector = LinuxStorageInspector::new();

    configure_wizard_storage(engine, state, &storage_inspector)
}

fn review_wizard_state(state: &WizardState) {
    println!();
    println!("Review:");
    println!(
        "  Profile            : {}",
        state
            .profile_name()
            .expect("profile should be selected before review")
    );
    println!(
        "  Content repository : {}",
        state
            .selected_content_repository()
            .expect("content repository should be selected before review")
    );

    if state.selected_external_content().is_empty() {
        println!("  External content   : none");
    } else {
        println!("  External content   :");
        for item_id in state.selected_external_content() {
            println!("    {item_id}");
        }
    }

    println!(
        "  Storage            : {}",
        state
            .selected_storage()
            .expect("storage should be selected before review")
    );
}
fn parse_wizard_confirmation(input: &str) -> bool {
    matches!(input.trim().to_ascii_lowercase().as_str(), "y" | "yes")
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

    Ok(parse_wizard_confirmation(&input))
}

fn prepare_wizard_content_import_with<I>(
    engine: &Engine,
    config: &model::ApplianceConfiguration,
    repository: &model::ContentRepository,
    inspector: &I,
) -> Result<engine::PreparedContentImport, String>
where
    I: ContentInspector,
{
    let items = engine
        .repository_content_items(repository, inspector)
        .map_err(|error| format!("Error discovering content: {error}"))?;

    engine
        .prepare_content_import(
            config.content_import().clone(),
            items,
            model::ContentImportDestination::new("/var/lib/daia/content"),
        )
        .map_err(|error| format!("Error preparing content import: {error:?}"))
}

fn prepare_wizard_content_import(
    engine: &Engine,
    config: &model::ApplianceConfiguration,
) -> Result<engine::PreparedContentImport, String> {
    let repositories = ContentRepositoryRepository::load_directory(&content_repository_directory())
        .map_err(|error| format!("Error loading content repositories: {error}"))?;

    let repository = repositories
        .repository(config.content_repository_id())
        .ok_or_else(|| {
            format!(
                "Error: selected content repository \"{}\" no longer exists",
                config.content_repository_id()
            )
        })?;

    let inspector = LocalFilesystemContentInspector::new();

    prepare_wizard_content_import_with(engine, config, repository, &inspector)
}
fn prepare_wizard_installation(
    engine: &Engine,
    config: &model::ApplianceConfiguration,
) -> Result<engine::PreparedInstallation, String> {
    let repository = appliance_profile_repository::load()
        .map_err(|error| format!("Error loading appliance profiles: {error}"))?;

    let profile = repository.profile(config.profile_name()).ok_or_else(|| {
        format!(
            "Error: selected appliance profile \"{}\" no longer exists",
            config.profile_name()
        )
    })?;

    let intent = config.installation().clone();

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
        InstallationOperation::ImportContent { .. } => "Import content".to_string(),
    }
}

fn print_content_import_operations(executor: &DryRunContentImportOperationExecutor) {
    if executor.executed_operations().is_empty() {
        return;
    }

    println!();
    println!("Planned content import operations:");

    for (index, operation) in executor.executed_operations().iter().enumerate() {
        match operation {
            engine::ContentImportOperation::ImportItem { item, destination } => {
                println!(
                    "  {}. Import {} to {}",
                    index + 1,
                    item.path().display(),
                    destination.path()
                );
            }
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

fn load_wizard_content_repositories(state: &mut WizardState) -> Result<(), String> {
    let repository = ContentRepositoryRepository::load_directory(&content_repository_directory())
        .map_err(|error| format!("Error loading content repositories: {error}"))?;

    state.set_content_repositories(repository.repositories().to_vec());

    Ok(())
}

fn discover_wizard_storage_with<I>(
    engine: &Engine,
    state: &mut WizardState,
    inspector: &I,
) -> Result<(), String>
where
    I: StorageInspector,
{
    let storage = engine
        .discover_storage(inspector)
        .map_err(|error| format!("Error discovering storage: {error}"))?;

    state.set_discovered_storage(storage);

    Ok(())
}

fn run_install() -> ExitCode {
    let Some(engine) = load_engine() else {
        return ExitCode::FAILURE;
    };

    let mut state = WizardState::new();

    println!("DAIA Installer");
    println!();

    if let Err(error) = configure_wizard_state(&engine, &mut state) {
        eprintln!("{error}");
        return ExitCode::FAILURE;
    }
    let Some(config) = state.into_config() else {
        eprintln!("Error: installer configuration is incomplete");
        return ExitCode::FAILURE;
    };

    let appliance_configuration = config.appliance_configuration();

    let prepared_content = match prepare_wizard_content_import(&engine, &appliance_configuration) {
        Ok(prepared) => prepared,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };

    let prepared_installation = match prepare_wizard_installation(&engine, &appliance_configuration)
    {
        Ok(prepared) => prepared,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };

    let prepared =
        engine::PreparedApplianceInstallation::new(prepared_installation, prepared_content);
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
    let package_repository = match PackageRepository::from_directory(package_manifest_directory()) {
        Ok(repository) => repository,
        Err(error) => {
            eprintln!("Error loading package repository: {error}");
            return ExitCode::FAILURE;
        }
    };

    let mut executor =
        SystemInstallationOperationExecutor::new(asset_directory(), package_repository);

    println!();
    println!("Starting installation...");

    match engine.execute_appliance_installation(&prepared, &mut executor) {
        Ok(()) => {
            println!("Installation complete.");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("Installation failed: {error}");
            ExitCode::FAILURE
        }
    }
}
fn run_wizard() -> ExitCode {
    let Some(engine) = load_engine() else {
        return ExitCode::FAILURE;
    };

    let mut state = WizardState::new();

    println!("DAIA Wizard");
    println!();

    if let Err(error) = configure_wizard_state(&engine, &mut state) {
        eprintln!("{error}");
        return ExitCode::FAILURE;
    }

    review_wizard_state(&state);

    match confirm_wizard_state() {
        Ok(true) => {
            let Some(config) = state.into_config() else {
                eprintln!("Error: wizard configuration is incomplete");
                return ExitCode::FAILURE;
            };

            let appliance_configuration = config.appliance_configuration();

            let prepared_content_import =
                match prepare_wizard_content_import(&engine, &appliance_configuration) {
                    Ok(prepared) => prepared,
                    Err(error) => {
                        eprintln!("{error}");
                        return ExitCode::FAILURE;
                    }
                };

            let mut content_import_executor = DryRunContentImportOperationExecutor::default();

            if let Err(error) = prepared_content_import.execute(&mut content_import_executor) {
                match error {}
            }

            let prepared = match prepare_wizard_installation(&engine, &appliance_configuration) {
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

            print_content_import_operations(&content_import_executor);
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
    fn prepares_content_import_from_appliance_configuration() {
        use model::{
            ApplianceConfiguration, ContentImportIntent, ContentRepository, ContentRepositoryId,
            ContentSource, ContentSourceId, DiscoveredStorageId, ExternalContentItem,
            InstallationIntent,
        };

        let directory =
            std::env::temp_dir().join(format!("daia-content-import-test-{}", std::process::id()));

        if directory.exists() {
            std::fs::remove_dir_all(&directory).expect("existing test directory should be removed");
        }

        std::fs::create_dir(&directory).expect("temporary directory should be created");

        let model_path = directory.join("model.gguf");

        std::fs::write(&model_path, "model").expect("model content should be written");

        let repository = ContentRepository::with_sources(
            "local-models",
            "Models available on local storage",
            vec![ContentSource::new(
                "local-models-directory",
                ContentRepositoryId::new("local-models"),
                directory.to_string_lossy(),
            )],
        );

        let item =
            ExternalContentItem::new(ContentSourceId::new("local-models-directory"), &model_path);

        let config = ApplianceConfiguration::new(
            "desktop",
            ContentRepositoryId::new("local-models"),
            ContentImportIntent::new(vec![item.id().clone()]),
            InstallationIntent::new("desktop", DiscoveredStorageId::new("serial:usb-disk")),
        );

        let engine = super::load_engine().expect("engine should load");
        let inspector = inspector::LocalFilesystemContentInspector::new();

        let prepared =
            super::prepare_wizard_content_import_with(&engine, &config, &repository, &inspector)
                .expect("appliance content import should prepare");

        assert_eq!(prepared.intent(), config.content_import());
        assert_eq!(prepared.items(), &[item]);
        assert_eq!(prepared.destination().path(), "/var/lib/daia/content");

        std::fs::remove_dir_all(&directory).expect("test directory should be removed");
    }
    #[test]
    fn loads_wizard_appliance_profiles() {
        let repository =
            super::load_wizard_appliance_profiles().expect("wizard appliance profiles should load");

        assert!(!repository.profiles().is_empty());
    }

    #[test]
    fn discovers_wizard_storage_into_state() {
        struct TestStorageInspector;

        impl inspector::StorageInspector for TestStorageInspector {
            fn inspect(
                &self,
            ) -> Result<Vec<model::DiscoveredStorage>, inspector::StorageInspectError> {
                Ok(vec![model::DiscoveredStorage::new(
                    "serial:test-disk",
                    model::StorageKind::Removable,
                    "/dev/test-disk",
                )])
            }
        }

        let engine = engine::Engine::from_registry(registry::Registry::new());
        let mut state = super::WizardState::new();

        super::discover_wizard_storage_with(&engine, &mut state, &TestStorageInspector)
            .expect("wizard storage discovery should succeed");

        assert_eq!(state.selectable_storage().count(), 1);
    }
    #[test]
    fn loads_wizard_content_repositories() {
        let mut state = super::WizardState::new();

        super::load_wizard_content_repositories(&mut state)
            .expect("wizard content repositories should load");

        assert!(!state.content_repositories().is_empty());
    }
    #[test]
    fn parses_wizard_confirmation() {
        assert!(super::parse_wizard_confirmation("yes"));
    }
    #[test]
    fn rejects_wizard_confirmation() {
        assert!(!super::parse_wizard_confirmation("no"));
    }
    #[test]
    fn rejects_invalid_content_repository_selection() {
        let result = super::parse_content_repository_selection("4", 3);

        assert_eq!(
            result,
            Err("Error: invalid content repository selection".to_owned())
        );
    }
    #[test]
    fn parses_content_repository_selection() {
        let selection = super::parse_content_repository_selection("2", 3)
            .expect("valid content repository selection should parse");

        assert_eq!(selection, 2);
    }
    #[test]
    fn parses_appliance_profile_selection() {
        let selection = super::parse_appliance_profile_selection("2", 3)
            .expect("valid appliance profile selection should parse");

        assert_eq!(selection, 2);
    }
    #[test]
    fn rejects_invalid_appliance_profile_selection() {
        let result = super::parse_appliance_profile_selection("4", 3);

        assert_eq!(
            result,
            Err("Error: invalid appliance profile selection".to_owned())
        );
    }
    #[test]
    fn parses_storage_selection() {
        let selection =
            super::parse_storage_selection("2", 3).expect("valid storage selection should parse");

        assert_eq!(selection, 2);
    }
    #[test]
    fn rejects_invalid_storage_selection() {
        let result = super::parse_storage_selection("4", 3);

        assert_eq!(result, Err("Error: invalid storage selection".to_owned()));
    }
    #[test]
    fn discovers_external_content_into_wizard_state() {
        use model::{ContentRepository, ContentRepositoryId, ContentSource};

        let directory = std::env::temp_dir().join(format!(
            "daia-external-content-discovery-test-{}",
            std::process::id()
        ));

        if directory.exists() {
            std::fs::remove_dir_all(&directory).expect("existing test directory should be removed");
        }

        std::fs::create_dir(&directory).expect("temporary directory should be created");

        let model_path = directory.join("model.gguf");

        std::fs::write(&model_path, "model").expect("model content should be written");

        let repository = ContentRepository::with_sources(
            "local-models",
            "Models available on local storage",
            vec![ContentSource::new(
                "local-models-directory",
                ContentRepositoryId::new("local-models"),
                directory.to_string_lossy(),
            )],
        );

        let mut state = super::WizardState::new();

        state.set_content_repositories(vec![repository]);
        state.select_content_repository(ContentRepositoryId::new("local-models"));

        let engine = super::load_engine().expect("engine should load");
        let inspector = inspector::LocalFilesystemContentInspector::new();

        super::discover_external_content(&engine, &mut state, &inspector)
            .expect("external content discovery should succeed");

        assert_eq!(state.discovered_content().len(), 1);
        assert_eq!(state.discovered_content()[0].path(), directory);

        assert_eq!(state.external_content_items().len(), 1);
        assert_eq!(state.external_content_items()[0].path(), model_path);

        std::fs::remove_dir_all(&directory).expect("test directory should be removed");
    }

    #[test]
    fn parses_multiple_external_content_selections() {
        let selections = super::parse_external_content_selection("1 3", 3)
            .expect("valid external content selections should parse");

        assert_eq!(selections, vec![1, 3]);
    }

    #[test]
    fn rejects_invalid_external_content_selection() {
        let result = super::parse_external_content_selection("1 4", 3);

        assert_eq!(
            result,
            Err("Error: invalid external content selection".to_owned())
        );
    }
    #[test]
    fn rejects_empty_external_content_selection() {
        let result = super::parse_external_content_selection("", 3);

        assert_eq!(
            result,
            Err("Error: invalid external content selection".to_owned())
        );
    }
    #[test]
    fn rejects_duplicate_external_content_selection() {
        let result = super::parse_external_content_selection("1 1", 3);

        assert_eq!(
            result,
            Err("Error: duplicate external content selection".to_owned())
        );
    }
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
