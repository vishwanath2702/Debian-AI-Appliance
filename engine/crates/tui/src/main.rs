//! DAIA terminal user interface.

use application::{
    WizardState, load_appliance_profiles, load_content_repositories, load_package_repository,
    load_provider_registry,
};
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use engine::Engine;
use inspector::{
    ContentInspector, LinuxStorageInspector, LocalFilesystemContentInspector, StorageInspector,
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    widgets::{Block, Borders, Paragraph},
};
use std::io::{self, stdout};

/// Screens presented by the DAIA configuration wizard.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WizardScreen {
    Welcome,
    Profile,
    ContentRepository,
    ExternalContent,
    Storage,
    Administrator,
    Review,
    ConfirmInstallation,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum StorageFocus {
    #[default]
    Devices,
    Back,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum ScreenAction {
    #[default]
    Continue,
    Back,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum InstallationAction {
    Install,
    #[default]
    Back,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum WelcomeAction {
    #[default]
    Start,
    Quit,
}

/// Credentials retained only for the active installer session.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum AdministratorField {
    #[default]
    RootPassword,
    RootPasswordConfirmation,
    Username,
    DisplayName,
    AdministratorPassword,
    AdministratorPasswordConfirmation,
    Back,
}

#[derive(Debug, Default)]
struct CredentialState {
    root_password: String,
    root_password_confirmation: String,
    administrator_password: String,
    administrator_password_confirmation: String,
}

/// Presentation state owned by the terminal interface.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum ExternalContentFocus {
    Items,
    SaveAndContinue,
    ContinueWithoutLocalContent,
    Back,
}

struct TuiState {
    wizard: WizardState,
    appliance_profiles: Vec<model::ApplianceProfile>,
    selected_profile_index: usize,
    selected_content_repository_index: usize,
    selected_external_content_index: usize,
    pending_external_content: Vec<model::ExternalContentItemId>,
    pending_model_names: Vec<(model::ExternalContentItemId, String)>,
    pending_model_name_index: Option<usize>,
    external_content_error: Option<String>,
    external_content_focus: ExternalContentFocus,
    selected_storage_index: usize,
    storage_error: Option<String>,
    storage_focus: StorageFocus,
    credentials: CredentialState,
    administrator_username: String,
    administrator_display_name: String,
    administrator_field: AdministratorField,
    administrator_error: Option<String>,
    review_error: Option<String>,
    installation_action: InstallationAction,
    welcome_action: WelcomeAction,
    screen_action: ScreenAction,
    screen: WizardScreen,
}

impl TuiState {
    fn new() -> Self {
        let appliance_profiles = load_appliance_profiles()
            .map(|repository| repository.profiles().to_vec())
            .unwrap_or_default();

        let content_repositories = load_content_repositories().unwrap_or_default();

        let mut wizard = WizardState::new();
        wizard.set_content_repositories(content_repositories);

        Self {
            wizard,
            appliance_profiles,
            selected_profile_index: 0,
            selected_content_repository_index: 0,
            selected_external_content_index: 0,
            pending_external_content: Vec::new(),
            pending_model_names: Vec::new(),
            pending_model_name_index: None,
            external_content_error: None,
            external_content_focus: ExternalContentFocus::ContinueWithoutLocalContent,
            selected_storage_index: 0,
            storage_error: None,
            storage_focus: StorageFocus::Devices,
            credentials: CredentialState::default(),
            administrator_username: String::new(),
            administrator_display_name: String::new(),
            administrator_field: AdministratorField::default(),
            administrator_error: None,
            review_error: None,
            installation_action: InstallationAction::Back,
            welcome_action: WelcomeAction::default(),
            screen_action: ScreenAction::default(),
            screen: WizardScreen::Welcome,
        }
    }

    fn next_profile(&mut self) {
        if self.selected_profile_index + 1 < self.appliance_profiles.len() {
            self.selected_profile_index += 1;
        }
    }

    fn previous_profile(&mut self) {
        self.selected_profile_index = self.selected_profile_index.saturating_sub(1);
    }

    fn confirm_profile(&mut self) {
        let Some(profile) = self.appliance_profiles.get(self.selected_profile_index) else {
            return;
        };

        self.wizard.set_profile_name(profile.name());
        self.next_screen();
    }

    fn next_content_repository(&mut self) {
        if self.selected_content_repository_index + 1 < self.wizard.content_repositories().len() {
            self.selected_content_repository_index += 1;
        }
    }

    fn previous_content_repository(&mut self) {
        self.selected_content_repository_index =
            self.selected_content_repository_index.saturating_sub(1);
    }

    fn confirm_content_repository(&mut self) {
        let Some(repository) = self
            .wizard
            .content_repositories()
            .get(self.selected_content_repository_index)
            .cloned()
        else {
            return;
        };

        let repository_id = repository.id().clone();
        let engine = Engine::from_registry(registry::Registry::default());
        let inspector = LocalFilesystemContentInspector::new();

        if discover_external_content(&engine, &mut self.wizard, &repository, &inspector).is_err() {
            return;
        }

        debug_assert_eq!(
            self.wizard.selected_content_repository(),
            Some(&repository_id)
        );

        self.selected_external_content_index = 0;
        self.pending_external_content = self.wizard.selected_external_content().to_vec();
        self.external_content_focus = if self.wizard.external_content_items().is_empty() {
            ExternalContentFocus::ContinueWithoutLocalContent
        } else {
            ExternalContentFocus::Items
        };

        self.next_screen();
    }

    fn next_screen_action(&mut self) {
        self.screen_action = ScreenAction::Back;
    }

    fn previous_screen_action(&mut self) {
        self.screen_action = ScreenAction::Continue;
    }

    fn next_installation_action(&mut self) {
        self.installation_action = InstallationAction::Back;
    }

    fn previous_installation_action(&mut self) {
        self.installation_action = InstallationAction::Install;
    }

    fn confirm_installation_action(&mut self) {
        match self.installation_action {
            InstallationAction::Install => {}
            InstallationAction::Back => self.previous_screen(),
        }

        self.installation_action = InstallationAction::Back;
    }

    fn confirm_screen_action(&mut self) {
        match self.screen_action {
            ScreenAction::Continue if self.screen == WizardScreen::Review => {
                self.prepare_review_installation();
            }
            ScreenAction::Continue => self.next_screen(),
            ScreenAction::Back => self.previous_screen(),
        }

        self.screen_action = ScreenAction::Continue;
    }

    fn prepare_review_installation(&mut self) {
        self.review_error = None;

        let registry = match load_provider_registry() {
            Ok(registry) => registry,
            Err(error) => {
                self.review_error = Some(format!("Error loading provider registry: {error}"));
                return;
            }
        };

        let engine = Engine::from_registry(registry);
        let content_inspector = LocalFilesystemContentInspector::new();
        let storage_inspector = LinuxStorageInspector::new();

        match prepare_appliance_installation(
            &engine,
            &self.wizard,
            &self.appliance_profiles,
            &content_inspector,
            &storage_inspector,
        ) {
            Ok(_) => {}
            Err(error) => {
                self.review_error = Some(error);
                return;
            }
        }

        if let Err(error) = load_package_repository() {
            self.review_error = Some(format!("Error loading package repository: {error}"));
            return;
        }

        if let Err(error) = engine::validate_installation_commands() {
            self.review_error = Some(format!("Error validating installation commands: {error}"));
            return;
        }

        self.installation_action = InstallationAction::Back;
        self.screen = WizardScreen::ConfirmInstallation;
    }

    fn next_welcome_action(&mut self) {
        self.welcome_action = WelcomeAction::Quit;
    }

    fn previous_welcome_action(&mut self) {
        self.welcome_action = WelcomeAction::Start;
    }

    fn next_storage(&mut self) {
        let selectable_count = self.wizard.selectable_storage().count();

        match self.storage_focus {
            StorageFocus::Devices => {
                if selectable_count == 0 || self.selected_storage_index + 1 >= selectable_count {
                    self.storage_focus = StorageFocus::Back;
                } else {
                    self.selected_storage_index += 1;
                }
            }
            StorageFocus::Back => {}
        }
    }

    fn previous_storage(&mut self) {
        let selectable_count = self.wizard.selectable_storage().count();

        match self.storage_focus {
            StorageFocus::Devices => {
                self.selected_storage_index = self.selected_storage_index.saturating_sub(1);
            }
            StorageFocus::Back => {
                if selectable_count > 0 {
                    self.storage_focus = StorageFocus::Devices;
                    self.selected_storage_index = selectable_count - 1;
                }
            }
        }
    }

    fn confirm_storage(&mut self) {
        match self.storage_focus {
            StorageFocus::Devices => {
                let selected_id = self
                    .wizard
                    .selectable_storage()
                    .nth(self.selected_storage_index)
                    .map(|storage| storage.id().clone());

                if let Some(selected_id) = selected_id {
                    self.wizard.select_storage(selected_id);
                    self.next_screen();
                }
            }
            StorageFocus::Back => {
                self.previous_screen();
            }
        }
    }

    fn enter_storage(&mut self) {
        let engine = Engine::from_registry(registry::Registry::default());
        let inspector = LinuxStorageInspector::new();

        self.storage_error = None;
        self.selected_storage_index = 0;
        self.storage_focus = StorageFocus::Devices;

        match discover_storage(&engine, &mut self.wizard, &inspector) {
            Ok(()) => {
                if self.wizard.selectable_storage().next().is_none() {
                    self.storage_focus = StorageFocus::Back;
                }
                self.screen = WizardScreen::Storage;
            }
            Err(error) => {
                self.storage_error = Some(format!("Error discovering storage: {error}"));
                self.screen = WizardScreen::Storage;
            }
        }
    }

    fn next_screen(&mut self) {
        self.screen = match self.screen {
            WizardScreen::Welcome => WizardScreen::Profile,
            WizardScreen::Profile => WizardScreen::ContentRepository,
            WizardScreen::ContentRepository => WizardScreen::ExternalContent,
            WizardScreen::ExternalContent => WizardScreen::Storage,
            WizardScreen::Storage => WizardScreen::Administrator,
            WizardScreen::Administrator => WizardScreen::Review,
            WizardScreen::Review => WizardScreen::Review,
            WizardScreen::ConfirmInstallation => WizardScreen::ConfirmInstallation,
        };
    }

    fn push_administrator_character(&mut self, character: char) {
        match self.administrator_field {
            AdministratorField::RootPassword => {
                self.credentials.root_password.push(character);
            }
            AdministratorField::RootPasswordConfirmation => {
                self.credentials.root_password_confirmation.push(character);
            }
            AdministratorField::Username => {
                self.administrator_username.push(character);
            }
            AdministratorField::DisplayName => {
                self.administrator_display_name.push(character);
            }
            AdministratorField::AdministratorPassword => {
                self.credentials.administrator_password.push(character);
            }
            AdministratorField::AdministratorPasswordConfirmation => {
                self.credentials
                    .administrator_password_confirmation
                    .push(character);
            }
            AdministratorField::Back => {}
        }
    }

    fn pop_administrator_character(&mut self) {
        match self.administrator_field {
            AdministratorField::RootPassword => {
                self.credentials.root_password.pop();
            }
            AdministratorField::RootPasswordConfirmation => {
                self.credentials.root_password_confirmation.pop();
            }
            AdministratorField::Username => {
                self.administrator_username.pop();
            }
            AdministratorField::DisplayName => {
                self.administrator_display_name.pop();
            }
            AdministratorField::AdministratorPassword => {
                self.credentials.administrator_password.pop();
            }
            AdministratorField::AdministratorPasswordConfirmation => {
                self.credentials.administrator_password_confirmation.pop();
            }
            AdministratorField::Back => {}
        }
    }

    fn next_administrator_field(&mut self) {
        self.administrator_field = match self.administrator_field {
            AdministratorField::RootPassword => AdministratorField::RootPasswordConfirmation,
            AdministratorField::RootPasswordConfirmation => AdministratorField::Username,
            AdministratorField::Username => AdministratorField::DisplayName,
            AdministratorField::DisplayName => AdministratorField::AdministratorPassword,
            AdministratorField::AdministratorPassword => {
                AdministratorField::AdministratorPasswordConfirmation
            }
            AdministratorField::AdministratorPasswordConfirmation => AdministratorField::Back,
            AdministratorField::Back => AdministratorField::Back,
        };
    }

    fn validate_administrator(&mut self) -> bool {
        let error = if self.credentials.root_password.is_empty() {
            Some((
                AdministratorField::RootPassword,
                "Root password cannot be empty",
            ))
        } else if self.credentials.root_password != self.credentials.root_password_confirmation {
            Some((
                AdministratorField::RootPasswordConfirmation,
                "Root passwords do not match",
            ))
        } else if self.administrator_username.trim().is_empty() {
            Some((
                AdministratorField::Username,
                "Administrator username cannot be empty",
            ))
        } else if self.administrator_display_name.trim().is_empty() {
            Some((
                AdministratorField::DisplayName,
                "Administrator display name cannot be empty",
            ))
        } else if self.credentials.administrator_password.is_empty() {
            Some((
                AdministratorField::AdministratorPassword,
                "Administrator password cannot be empty",
            ))
        } else if self.credentials.administrator_password
            != self.credentials.administrator_password_confirmation
        {
            Some((
                AdministratorField::AdministratorPasswordConfirmation,
                "Administrator passwords do not match",
            ))
        } else {
            None
        };

        if let Some((field, error)) = error {
            self.administrator_field = field;
            self.administrator_error = Some(error.to_owned());
            return false;
        }

        self.wizard.set_user_identity(
            self.administrator_username.trim(),
            self.administrator_display_name.trim(),
        );
        self.administrator_error = None;
        true
    }

    fn confirm_administrator_field(&mut self) {
        match self.administrator_field {
            AdministratorField::AdministratorPasswordConfirmation => {
                if self.validate_administrator() {
                    self.next_screen();
                }
            }
            AdministratorField::Back => {
                self.previous_screen();
                self.administrator_field = AdministratorField::RootPassword;
            }
            _ => {
                self.next_administrator_field();
            }
        }
    }

    fn previous_administrator_field(&mut self) {
        self.administrator_field = match self.administrator_field {
            AdministratorField::RootPassword => AdministratorField::RootPassword,
            AdministratorField::Back => AdministratorField::AdministratorPasswordConfirmation,
            AdministratorField::RootPasswordConfirmation => AdministratorField::RootPassword,
            AdministratorField::Username => AdministratorField::RootPasswordConfirmation,
            AdministratorField::DisplayName => AdministratorField::Username,
            AdministratorField::AdministratorPassword => AdministratorField::DisplayName,
            AdministratorField::AdministratorPasswordConfirmation => {
                AdministratorField::AdministratorPassword
            }
        };
    }

    fn next_external_content(&mut self) {
        if self.wizard.external_content_items().is_empty() {
            self.external_content_focus = match self.external_content_focus {
                ExternalContentFocus::ContinueWithoutLocalContent => ExternalContentFocus::Back,
                ExternalContentFocus::Back => ExternalContentFocus::Back,
                _ => ExternalContentFocus::ContinueWithoutLocalContent,
            };
            return;
        }

        match self.external_content_focus {
            ExternalContentFocus::Items => {
                if self.selected_external_content_index + 1
                    < self.wizard.external_content_items().len()
                {
                    self.selected_external_content_index += 1;
                } else {
                    self.external_content_focus = ExternalContentFocus::SaveAndContinue;
                }
            }
            ExternalContentFocus::SaveAndContinue => {
                self.external_content_focus = ExternalContentFocus::ContinueWithoutLocalContent;
            }
            ExternalContentFocus::ContinueWithoutLocalContent => {
                self.external_content_focus = ExternalContentFocus::Back;
            }
            ExternalContentFocus::Back => {}
        }
    }

    fn previous_external_content(&mut self) {
        if self.wizard.external_content_items().is_empty() {
            self.external_content_focus = match self.external_content_focus {
                ExternalContentFocus::Back => ExternalContentFocus::ContinueWithoutLocalContent,
                _ => ExternalContentFocus::ContinueWithoutLocalContent,
            };
            return;
        }

        match self.external_content_focus {
            ExternalContentFocus::Items => {
                self.selected_external_content_index =
                    self.selected_external_content_index.saturating_sub(1);
            }
            ExternalContentFocus::SaveAndContinue => {
                self.external_content_focus = ExternalContentFocus::Items;
                self.selected_external_content_index =
                    self.wizard.external_content_items().len() - 1;
            }
            ExternalContentFocus::ContinueWithoutLocalContent => {
                self.external_content_focus = ExternalContentFocus::SaveAndContinue;
            }
            ExternalContentFocus::Back => {
                self.external_content_focus = ExternalContentFocus::ContinueWithoutLocalContent;
            }
        }
    }

    fn toggle_external_content(&mut self) {
        if self.external_content_focus != ExternalContentFocus::Items {
            return;
        }

        let Some(item) = self
            .wizard
            .external_content_items()
            .get(self.selected_external_content_index)
        else {
            return;
        };

        let item_id = item.id().clone();

        if let Some(index) = self
            .pending_external_content
            .iter()
            .position(|selected| selected == &item_id)
        {
            self.pending_external_content.remove(index);
        } else {
            self.pending_external_content.push(item_id);
        }
    }

    fn confirm_external_content(&mut self) {
        match self.external_content_focus {
            ExternalContentFocus::Items => {}
            ExternalContentFocus::SaveAndContinue => {
                self.external_content_error = None;
                self.pending_model_names.clear();
                self.pending_model_name_index = None;

                if self.pending_external_content.is_empty() {
                    self.wizard.select_external_content(Vec::new());
                    self.wizard.set_model_realization_intents(Vec::new());
                    self.enter_storage();
                    return;
                }

                let engine = Engine::from_registry(registry::Registry::default());

                for item_id in &self.pending_external_content {
                    let Some(item) = self
                        .wizard
                        .external_content_items()
                        .iter()
                        .find(|item| item.id() == item_id)
                    else {
                        self.external_content_error =
                            Some("Selected external content is no longer available.".to_owned());
                        return;
                    };

                    match engine.inspect_external_model(item) {
                        Ok(Some(_)) => {
                            self.pending_model_names
                                .push((item.id().clone(), String::new()));
                        }
                        Ok(None) => {}
                        Err(error) => {
                            self.external_content_error = Some(format!(
                                "Error inspecting external content {}: {error}",
                                item.path().display()
                            ));
                            self.pending_model_names.clear();
                            return;
                        }
                    }
                }

                if self.pending_model_names.is_empty() {
                    self.wizard
                        .select_external_content(self.pending_external_content.clone());
                    self.wizard.set_model_realization_intents(Vec::new());
                    self.enter_storage();
                } else {
                    self.pending_model_name_index = Some(0);
                }
            }
            ExternalContentFocus::ContinueWithoutLocalContent => {
                self.pending_external_content.clear();
                self.wizard.select_external_content(Vec::new());
                self.enter_storage();
            }
            ExternalContentFocus::Back => {
                self.previous_screen();
            }
        }
    }

    fn push_external_model_name_character(&mut self, character: char) {
        let Some(index) = self.pending_model_name_index else {
            return;
        };

        if let Some((_, name)) = self.pending_model_names.get_mut(index) {
            name.push(character);
            self.external_content_error = None;
        }
    }

    fn pop_external_model_name_character(&mut self) {
        let Some(index) = self.pending_model_name_index else {
            return;
        };

        if let Some((_, name)) = self.pending_model_names.get_mut(index) {
            name.pop();
            self.external_content_error = None;
        }
    }

    fn cancel_external_model_naming(&mut self) {
        self.pending_model_names.clear();
        self.pending_model_name_index = None;
        self.external_content_error = None;
        self.external_content_focus = ExternalContentFocus::SaveAndContinue;
    }

    fn confirm_external_model_name(&mut self) {
        let Some(index) = self.pending_model_name_index else {
            return;
        };

        let Some((_, name)) = self.pending_model_names.get(index) else {
            return;
        };

        if name.trim().is_empty() {
            self.external_content_error = Some("DAIA model name cannot be empty.".to_owned());
            return;
        }

        if index + 1 < self.pending_model_names.len() {
            self.pending_model_name_index = Some(index + 1);
            self.external_content_error = None;
            return;
        }

        let realization_intents = self
            .pending_model_names
            .iter()
            .map(|(item_id, name)| {
                model::ModelRealizationIntent::new(
                    model::ModelRealizationId::new(name.trim()),
                    model::InferenceEngineId::ollama(),
                    item_id.clone(),
                )
            })
            .collect::<Vec<_>>();

        self.wizard
            .select_external_content(self.pending_external_content.clone());
        self.wizard
            .set_model_realization_intents(realization_intents);

        self.pending_model_name_index = None;
        self.external_content_error = None;
        self.enter_storage();
    }

    fn previous_screen(&mut self) {
        self.screen = match self.screen {
            WizardScreen::Welcome => WizardScreen::Welcome,
            WizardScreen::Profile => WizardScreen::Welcome,
            WizardScreen::ContentRepository => WizardScreen::Profile,
            WizardScreen::ExternalContent => WizardScreen::ContentRepository,
            WizardScreen::Storage => WizardScreen::ExternalContent,
            WizardScreen::Administrator => WizardScreen::Storage,
            WizardScreen::Review => WizardScreen::Administrator,
            WizardScreen::ConfirmInstallation => WizardScreen::Review,
        };
    }
}

fn discover_storage<I>(
    engine: &Engine,
    state: &mut WizardState,
    inspector: &I,
) -> Result<(), inspector::StorageInspectError>
where
    I: StorageInspector,
{
    let storage = engine.discover_storage(inspector)?;
    state.set_discovered_storage(storage);

    Ok(())
}

fn prepare_appliance_installation<I, S>(
    engine: &Engine,
    state: &WizardState,
    profiles: &[model::ApplianceProfile],
    content_inspector: &I,
    storage_inspector: &S,
) -> Result<engine::PreparedApplianceInstallation, String>
where
    I: ContentInspector,
    S: StorageInspector,
{
    let profile_name = state
        .profile_name()
        .ok_or_else(|| "installer configuration is missing an appliance profile".to_owned())?;

    let profile = profiles
        .iter()
        .find(|profile| profile.name() == profile_name)
        .ok_or_else(|| {
            format!("selected appliance profile \"{profile_name}\" is no longer available")
        })?;

    let repository_id = state
        .selected_content_repository()
        .ok_or_else(|| "installer configuration is missing a content repository".to_owned())?;

    let repository = state
        .content_repositories()
        .iter()
        .find(|repository| repository.id() == repository_id)
        .ok_or_else(|| {
            format!(
                "selected content repository \"{}\" is no longer available",
                repository_id.as_str()
            )
        })?;

    let storage = engine
        .discover_storage(storage_inspector)
        .map_err(|error| format!("Error discovering storage: {error}"))?;

    let config = state
        .clone()
        .into_config()
        .ok_or_else(|| "installer configuration is incomplete".to_owned())?;

    engine.prepare_appliance_configuration(
        &config.appliance_configuration(),
        profile,
        repository,
        content_inspector,
        &storage,
        model::ContentImportDestination::new("/var/lib/daia/content"),
    )
}

fn discover_external_content<I>(
    engine: &Engine,
    state: &mut WizardState,
    repository: &model::ContentRepository,
    inspector: &I,
) -> Result<(), inspector::ContentInspectError>
where
    I: ContentInspector,
{
    let items = engine.repository_content_items(repository, inspector)?;

    state.select_content_repository(repository.id().clone());
    state.set_external_content_items(items);

    Ok(())
}

fn screen_title(screen: WizardScreen) -> &'static str {
    match screen {
        WizardScreen::Welcome => "Welcome",
        WizardScreen::Profile => "Appliance Profile",
        WizardScreen::ContentRepository => "Content Repository",
        WizardScreen::ExternalContent => "External Content",
        WizardScreen::Storage => "Installation Storage",
        WizardScreen::Administrator => "Administrator",
        WizardScreen::Review => "Review",
        WizardScreen::ConfirmInstallation => "Confirm Installation",
    }
}

fn render(frame: &mut ratatui::Frame<'_>, state: &TuiState) {
    let areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(3),
        ])
        .split(frame.area());

    let title = Paragraph::new("Debian AI Appliance")
        .block(Block::default().borders(Borders::ALL).title("DAIA"));

    let profile = state.wizard.profile_name().unwrap_or("Not selected");

    let screens = [
        WizardScreen::Welcome,
        WizardScreen::Profile,
        WizardScreen::ContentRepository,
        WizardScreen::ExternalContent,
        WizardScreen::Storage,
        WizardScreen::Administrator,
        WizardScreen::Review,
    ];

    let navigation = screens
        .iter()
        .map(|screen| {
            let marker = if *screen == state.screen { ">" } else { " " };
            format!("{marker} {}", screen_title(*screen))
        })
        .collect::<Vec<_>>()
        .join("\n");

    let detail = match state.screen {
        WizardScreen::Welcome => {
            let start_marker = if state.welcome_action == WelcomeAction::Start {
                ">"
            } else {
                " "
            };
            let quit_marker = if state.welcome_action == WelcomeAction::Quit {
                ">"
            } else {
                " "
            };

            format!(
                "Welcome to Debian AI Appliance\n\n                 {start_marker} Start\n                 {quit_marker} Quit"
            )
        }
        WizardScreen::Profile => {
            if state.appliance_profiles.is_empty() {
                "No appliance profiles are available.".to_owned()
            } else {
                let profiles = state
                    .appliance_profiles
                    .iter()
                    .enumerate()
                    .map(|(index, profile)| {
                        let marker = if index == state.selected_profile_index {
                            ">"
                        } else {
                            " "
                        };

                        let capabilities = profile
                            .capabilities()
                            .iter()
                            .map(model::Capability::as_str)
                            .collect::<Vec<_>>()
                            .join(", ");

                        format!(
                            "{marker} {}\n    {}\n    Capabilities: {capabilities}",
                            profile.name(),
                            profile.description(),
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n\n");

                format!("Choose the appliance profile that DAIA will configure.\n\n{profiles}")
            }
        }
        WizardScreen::ContentRepository => {
            if state.wizard.content_repositories().is_empty() {
                "No content repositories are available.".to_owned()
            } else {
                let repositories = state
                    .wizard
                    .content_repositories()
                    .iter()
                    .enumerate()
                    .map(|(index, repository)| {
                        let marker = if index == state.selected_content_repository_index {
                            ">"
                        } else {
                            " "
                        };

                        format!(
                            "{marker} {}\n    {}",
                            repository.id(),
                            repository.description(),
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n\n");

                format!(
                    "Choose the content repository DAIA will use for external content.\n\n{repositories}"
                )
            }
        }
        WizardScreen::ExternalContent => {
            if let Some(model_index) = state.pending_model_name_index {
                let (item_id, model_name) = &state.pending_model_names[model_index];

                let path = state
                    .wizard
                    .external_content_items()
                    .iter()
                    .find(|item| item.id() == item_id)
                    .map(|item| item.path().display().to_string())
                    .unwrap_or_else(|| item_id.to_string());

                let error = state
                    .external_content_error
                    .as_deref()
                    .map_or(String::new(), |error| {
                        format!("\n\n                     Error: {error}")
                    });

                format!(
                    "External Content\n\n                     Configure selected model {} of {}\n\n                     Model: {path}\n                     DAIA model name: {model_name}\n\n                     Enter: Save model name\n                     Esc: Return to content selection{error}",
                    model_index + 1,
                    state.pending_model_names.len(),
                )
            } else {
                let continue_marker = if state.external_content_focus
                    == ExternalContentFocus::ContinueWithoutLocalContent
                {
                    ">"
                } else {
                    " "
                };
                let back_marker = if state.external_content_focus == ExternalContentFocus::Back {
                    ">"
                } else {
                    " "
                };

                let error = state
                    .external_content_error
                    .as_deref()
                    .map_or(String::new(), |error| {
                        format!("\n\n                     Error: {error}")
                    });

                if state.wizard.external_content_items().is_empty() {
                    format!(
                        "External Content\n\n                     No local content was discovered.\n\n                     {continue_marker} Continue without local content\n                     {back_marker} Back\n\n                     Models and other content can be added after installation.{error}"
                    )
                } else {
                    let items = state
                        .wizard
                        .external_content_items()
                        .iter()
                        .enumerate()
                        .map(|(index, item)| {
                            let cursor = if state.external_content_focus
                                == ExternalContentFocus::Items
                                && index == state.selected_external_content_index
                            {
                                ">"
                            } else {
                                " "
                            };
                            let selected = if state
                                .pending_external_content
                                .iter()
                                .any(|selected| selected == item.id())
                            {
                                "x"
                            } else {
                                " "
                            };

                            format!("{cursor} [{selected}] {}", item.path().display())
                        })
                        .collect::<Vec<_>>()
                        .join("\n");

                    let save_marker =
                        if state.external_content_focus == ExternalContentFocus::SaveAndContinue {
                            ">"
                        } else {
                            " "
                        };

                    format!(
                        "External Content\n\n                     Content discovered in the selected repository:\n\n                     {items}\n\n                     {save_marker} Save and Continue\n                     {continue_marker} Continue without local content\n                     {back_marker} Back\n\n                     Space toggles the highlighted content. Models and other content can also be added after installation.{error}"
                    )
                }
            }
        }
        WizardScreen::Storage => {
            let storage = state
                .wizard
                .selectable_storage()
                .enumerate()
                .map(|(index, storage)| {
                    let marker = if state.storage_focus == StorageFocus::Devices
                        && index == state.selected_storage_index
                    {
                        ">"
                    } else {
                        " "
                    };

                    let size = match storage.size_bytes() {
                        Some(size_bytes) => {
                            let gib = size_bytes as f64 / 1024.0 / 1024.0 / 1024.0;
                            format!("{gib:.1} GiB")
                        }
                        None => "unknown size".to_owned(),
                    };

                    format!(
                        "{marker} {}  {size}  {}  {}",
                        storage.kind(),
                        storage.id(),
                        storage.device_path().display(),
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");

            let back_marker = if state.storage_focus == StorageFocus::Back {
                ">"
            } else {
                " "
            };

            let error = state
                .storage_error
                .as_deref()
                .map_or(String::new(), |error| format!("\n\nError: {error}"));

            if storage.is_empty() {
                format!(
                    "Installation Storage\n\n\
                     No selectable storage devices were found.\n\n\
                     {back_marker} Back{error}"
                )
            } else {
                format!(
                    "Installation Storage\n\n\
                     Select the device DAIA will install onto.\n\n\
                     {storage}\n\n\
                     {back_marker} Back{error}"
                )
            }
        }
        WizardScreen::Administrator => {
            let username = if state.administrator_username.is_empty() {
                "Not configured"
            } else {
                &state.administrator_username
            };

            let display_name = if state.administrator_display_name.is_empty() {
                "Not configured"
            } else {
                &state.administrator_display_name
            };

            let root_password = "*".repeat(state.credentials.root_password.chars().count());
            let root_confirmation =
                "*".repeat(state.credentials.root_password_confirmation.chars().count());
            let administrator_password =
                "*".repeat(state.credentials.administrator_password.chars().count());
            let administrator_confirmation = "*".repeat(
                state
                    .credentials
                    .administrator_password_confirmation
                    .chars()
                    .count(),
            );

            let marker = |field| {
                if state.administrator_field == field {
                    ">"
                } else {
                    " "
                }
            };

            let error = state
                .administrator_error
                .as_deref()
                .map_or(String::new(), |error| {
                    format!("\n\n                 Error: {error}")
                });

            format!(
                "Administrator\n\n                 Root account\n                 {} Root password: {root_password}\n                 {} Confirm password: {root_confirmation}\n\n                 Administrator account\n                 {} Username: {username}\n                 {} Display name: {display_name}\n                 {} Password: {administrator_password}\n                 {} Confirm password: {administrator_confirmation}\n\n                 {} Back{error}",
                marker(AdministratorField::RootPassword),
                marker(AdministratorField::RootPasswordConfirmation),
                marker(AdministratorField::Username),
                marker(AdministratorField::DisplayName),
                marker(AdministratorField::AdministratorPassword),
                marker(AdministratorField::AdministratorPasswordConfirmation),
                marker(AdministratorField::Back),
            )
        }
        WizardScreen::Review => {
            let continue_marker = if state.screen_action == ScreenAction::Continue {
                ">"
            } else {
                " "
            };
            let back_marker = if state.screen_action == ScreenAction::Back {
                ">"
            } else {
                " "
            };

            let repository = state
                .wizard
                .selected_content_repository()
                .map_or("Not selected", model::ContentRepositoryId::as_str);

            let storage = state
                .wizard
                .selected_storage_device()
                .map(|storage| {
                    format!(
                        "{} ({}, {})",
                        storage.device_path().display(),
                        storage.kind(),
                        storage.id(),
                    )
                })
                .unwrap_or_else(|| "Not selected".to_owned());

            let local_content = state.wizard.selected_external_content().len();

            let username = if state.administrator_username.is_empty() {
                "Not configured"
            } else {
                &state.administrator_username
            };

            let display_name = if state.administrator_display_name.is_empty() {
                "Not configured"
            } else {
                &state.administrator_display_name
            };

            let error = state
                .review_error
                .as_deref()
                .map_or(String::new(), |error| format!("\n\nError: {error}"));

            format!(
                "Review Configuration\n\n\
                 Appliance profile: {profile}\n\
                 Content repository: {repository}\n\
                 Local content selected: {local_content}\n\
                 Installation storage: {storage}\n\
                 Administrator: {username}\n\
                 Display name: {display_name}\n\n\
                 Root and administrator passwords are configured but are not displayed.\n\n\
                 {continue_marker} Continue\n\
                 {back_marker} Back{error}"
            )
        }
        WizardScreen::ConfirmInstallation => {
            let storage = state
                .wizard
                .selected_storage_device()
                .map(|storage| storage.device_path().display().to_string())
                .unwrap_or_else(|| "Not selected".to_owned());

            let install_marker = if state.installation_action == InstallationAction::Install {
                ">"
            } else {
                " "
            };
            let back_marker = if state.installation_action == InstallationAction::Back {
                ">"
            } else {
                " "
            };

            format!(
                "Confirm Installation\n\n\
                 WARNING: Installation will erase the selected target disk.\n\n\
                 Target disk: {storage}\n\n\
                 Installation has not started.\n\n\
                 {install_marker} Install\n\
                 {back_marker} Back"
            )
        }
    };

    let body = Paragraph::new(format!("{navigation}\n\n{detail}")).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Configuration"),
    );

    let controls = match state.screen {
        WizardScreen::Welcome => "↑ / ↓: Navigate    Enter: Select",
        WizardScreen::ExternalContent if state.pending_model_name_index.is_some() => {
            "Type: Model name    Enter: Save    Backspace: Edit    Esc: Cancel"
        }
        WizardScreen::ExternalContent if !state.wizard.external_content_items().is_empty() => {
            "↑ / ↓: Navigate    Space: Toggle    Enter: Select    Esc: Back"
        }
        WizardScreen::ExternalContent => "↑ / ↓: Navigate    Enter: Select    Esc: Back",
        WizardScreen::Administrator => {
            "↑ / ↓: Navigate    Enter: Select    Esc: Back    Backspace: Edit"
        }
        WizardScreen::ConfirmInstallation => "↑ / ↓: Navigate    Enter: Select    Esc: Back",
        _ => "↑ / ↓: Navigate    Enter: Select    Esc: Back",
    };

    let controls = Paragraph::new(controls).block(Block::default().borders(Borders::ALL));

    frame.render_widget(title, areas[0]);
    frame.render_widget(body, areas[1]);
    frame.render_widget(controls, areas[2]);
}

fn run() -> io::Result<()> {
    enable_raw_mode()?;

    let mut output = stdout();
    execute!(output, EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(output);
    let mut terminal = Terminal::new(backend)?;
    let mut state = TuiState::new();

    let result = loop {
        terminal.draw(|frame| render(frame, &state))?;

        let Event::Key(key) = event::read()? else {
            continue;
        };

        match key.code {
            KeyCode::Esc
                if state.screen == WizardScreen::ExternalContent
                    && state.pending_model_name_index.is_some() =>
            {
                state.cancel_external_model_naming();
            }
            KeyCode::Esc if state.screen != WizardScreen::Welcome => {
                state.previous_screen();
                state.screen_action = ScreenAction::Continue;

                if state.screen == WizardScreen::Administrator {
                    state.administrator_field = AdministratorField::RootPassword;
                }
            }
            KeyCode::Down if state.screen == WizardScreen::Welcome => {
                state.next_welcome_action();
            }
            KeyCode::Up if state.screen == WizardScreen::Welcome => {
                state.previous_welcome_action();
            }
            KeyCode::Enter if state.screen == WizardScreen::Welcome => match state.welcome_action {
                WelcomeAction::Start => state.next_screen(),
                WelcomeAction::Quit => break Ok(()),
            },
            KeyCode::Down if state.screen == WizardScreen::Profile => {
                state.next_profile();
            }
            KeyCode::Up if state.screen == WizardScreen::Profile => {
                state.previous_profile();
            }
            KeyCode::Enter if state.screen == WizardScreen::Profile => {
                state.confirm_profile();
            }
            KeyCode::Down if state.screen == WizardScreen::ContentRepository => {
                state.next_content_repository();
            }
            KeyCode::Up if state.screen == WizardScreen::ContentRepository => {
                state.previous_content_repository();
            }
            KeyCode::Enter if state.screen == WizardScreen::ContentRepository => {
                state.confirm_content_repository();
            }
            KeyCode::Down
                if state.screen == WizardScreen::ExternalContent
                    && state.pending_model_name_index.is_none() =>
            {
                state.next_external_content();
            }
            KeyCode::Up
                if state.screen == WizardScreen::ExternalContent
                    && state.pending_model_name_index.is_none() =>
            {
                state.previous_external_content();
            }
            KeyCode::Char(' ')
                if state.screen == WizardScreen::ExternalContent
                    && state.pending_model_name_index.is_none() =>
            {
                state.toggle_external_content();
            }
            KeyCode::Down if state.screen == WizardScreen::Storage => {
                state.next_storage();
            }
            KeyCode::Down if state.screen == WizardScreen::Review => {
                state.next_screen_action();
            }
            KeyCode::Down if state.screen == WizardScreen::ConfirmInstallation => {
                state.next_installation_action();
            }
            KeyCode::Up if state.screen == WizardScreen::Storage => {
                state.previous_storage();
            }
            KeyCode::Up if state.screen == WizardScreen::Review => {
                state.previous_screen_action();
            }
            KeyCode::Up if state.screen == WizardScreen::ConfirmInstallation => {
                state.previous_installation_action();
            }
            KeyCode::Backspace
                if state.screen == WizardScreen::ExternalContent
                    && state.pending_model_name_index.is_some() =>
            {
                state.pop_external_model_name_character();
            }
            KeyCode::Char(character)
                if state.screen == WizardScreen::ExternalContent
                    && state.pending_model_name_index.is_some() =>
            {
                state.push_external_model_name_character(character);
            }
            KeyCode::Enter
                if state.screen == WizardScreen::ExternalContent
                    && state.pending_model_name_index.is_some() =>
            {
                state.confirm_external_model_name();
            }
            KeyCode::Enter if state.screen == WizardScreen::ExternalContent => {
                state.confirm_external_content();
            }
            KeyCode::Enter if state.screen == WizardScreen::Storage => {
                state.confirm_storage();
            }
            KeyCode::Enter if state.screen == WizardScreen::Review => {
                state.confirm_screen_action();
            }
            KeyCode::Enter if state.screen == WizardScreen::ConfirmInstallation => {
                state.confirm_installation_action();
            }
            KeyCode::Down if state.screen == WizardScreen::Administrator => {
                state.next_administrator_field();
            }
            KeyCode::Up if state.screen == WizardScreen::Administrator => {
                state.previous_administrator_field();
            }
            KeyCode::Backspace if state.screen == WizardScreen::Administrator => {
                state.pop_administrator_character();
            }
            KeyCode::Char(character) if state.screen == WizardScreen::Administrator => {
                state.push_administrator_character(character);
            }
            KeyCode::Enter if state.screen == WizardScreen::Administrator => {
                state.confirm_administrator_field();
            }
            KeyCode::Enter => state.next_screen(),
            _ => {}
        }
    };

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

fn main() -> io::Result<()> {
    run()
}

#[cfg(test)]
mod tests {
    use super::{
        AdministratorField, ExternalContentFocus, InstallationAction, ScreenAction, TuiState,
        WelcomeAction, WizardScreen, render,
    };
    use ratatui::{Terminal, backend::TestBackend};

    #[test]
    fn installation_confirmation_defaults_to_back() {
        let mut state = TuiState::new();

        state.installation_action = InstallationAction::Install;
        state.screen = WizardScreen::Review;

        state.installation_action = InstallationAction::Back;
        state.screen = WizardScreen::ConfirmInstallation;

        assert_eq!(state.screen, WizardScreen::ConfirmInstallation);
        assert_eq!(state.installation_action, InstallationAction::Back);
    }

    #[test]
    fn selecting_install_is_inert_until_execution_is_wired() {
        let mut state = TuiState::new();

        state.screen = WizardScreen::ConfirmInstallation;
        state.installation_action = InstallationAction::Install;

        state.confirm_installation_action();

        assert_eq!(state.screen, WizardScreen::ConfirmInstallation);
        assert_eq!(state.installation_action, InstallationAction::Back);
    }

    #[test]
    fn confirming_installation_back_returns_to_review() {
        let mut state = TuiState::new();

        state.screen = WizardScreen::ConfirmInstallation;
        state.installation_action = InstallationAction::Back;

        state.confirm_installation_action();

        assert_eq!(state.screen, WizardScreen::Review);
        assert_eq!(state.installation_action, InstallationAction::Back);
    }

    #[test]
    fn renders_review_configuration_summary() {
        let mut state = TuiState::new();

        let storage = model::DiscoveredStorage::new(
            "review-disk",
            model::StorageKind::Secondary,
            "/dev/review",
        )
        .with_size_bytes(128 * 1024 * 1024 * 1024);
        let storage_id = storage.id().clone();

        state.wizard.set_discovered_storage(vec![storage]);
        state.wizard.select_storage(storage_id);
        state
            .wizard
            .set_user_identity("review-admin", "Review Administrator");

        state.administrator_username = "review-admin".to_owned();
        state.administrator_display_name = "Review Administrator".to_owned();
        state.screen = WizardScreen::Review;

        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).expect("test terminal should be created");

        terminal
            .draw(|frame| render(frame, &state))
            .expect("Review should render");

        let rendered = terminal.backend().to_string();

        assert!(rendered.contains("Review Configuration"));
        assert!(rendered.contains("/dev/review"));
        assert!(rendered.contains("review-disk"));
        assert!(rendered.contains("review-admin"));
        assert!(rendered.contains("Review Administrator"));
        assert!(rendered.contains("Root and administrator passwords are configured"));
    }

    #[test]
    fn starts_at_welcome_screen_with_empty_wizard_state() {
        let state = TuiState::new();

        assert_eq!(state.screen, WizardScreen::Welcome);
        assert_eq!(state.wizard.profile_name(), None);
        assert_eq!(state.wizard.selected_content_repository(), None);
        assert_eq!(state.wizard.selected_storage(), None);
        assert_eq!(state.wizard.user_configuration(), None);
    }

    #[test]
    fn selects_welcome_start_and_quit_actions() {
        let mut state = TuiState::new();

        assert_eq!(state.welcome_action, WelcomeAction::Start);

        state.next_welcome_action();
        assert_eq!(state.welcome_action, WelcomeAction::Quit);

        state.previous_welcome_action();
        assert_eq!(state.welcome_action, WelcomeAction::Start);
    }

    #[test]
    fn selects_appliance_profile_and_advances() {
        let mut state = TuiState::new();

        assert!(
            !state.appliance_profiles.is_empty(),
            "repository should provide appliance profiles"
        );

        state.screen = WizardScreen::Profile;

        let selected_name = state.appliance_profiles[0].name().to_owned();

        state.confirm_profile();

        assert_eq!(state.wizard.profile_name(), Some(selected_name.as_str()));
        assert_eq!(state.screen, WizardScreen::ContentRepository);
    }

    #[test]
    fn continues_without_external_content_and_advances() {
        let mut state = TuiState::new();

        let source_id = model::ContentSourceId::new("test-source");
        let item = model::ExternalContentItem::new(source_id, "/media/models/model.gguf");
        let item_id = item.id().clone();

        state.wizard.set_external_content_items(vec![item]);
        state.wizard.select_external_content(vec![item_id.clone()]);
        state.pending_external_content = vec![item_id];
        state.screen = WizardScreen::ExternalContent;
        state.external_content_focus = ExternalContentFocus::ContinueWithoutLocalContent;

        state.confirm_external_content();

        assert!(state.pending_external_content.is_empty());
        assert!(state.wizard.selected_external_content().is_empty());
        assert_eq!(state.screen, WizardScreen::Storage);
    }

    #[test]
    fn external_content_back_returns_to_content_repository() {
        let mut state = TuiState::new();

        state.screen = WizardScreen::ExternalContent;
        state.external_content_focus = ExternalContentFocus::Back;

        state.confirm_external_content();

        assert_eq!(state.screen, WizardScreen::ContentRepository);
    }

    #[test]
    fn rejects_empty_external_model_name() {
        let mut state = TuiState::new();

        let item_id = model::ExternalContentItemId::new("test-model");
        state.pending_model_names = vec![(item_id, String::new())];
        state.pending_model_name_index = Some(0);
        state.screen = WizardScreen::ExternalContent;

        state.confirm_external_model_name();

        assert_eq!(state.pending_model_name_index, Some(0));
        assert_eq!(
            state.external_content_error.as_deref(),
            Some("DAIA model name cannot be empty.")
        );
        assert_eq!(state.screen, WizardScreen::ExternalContent);
    }

    #[test]
    fn advances_through_external_model_names_and_commits_final_selection() {
        let mut state = TuiState::new();

        let source_id = model::ContentSourceId::new("test-source");
        let first = model::ExternalContentItem::new(source_id.clone(), "/media/models/first.gguf");
        let second = model::ExternalContentItem::new(source_id, "/media/models/second.gguf");

        let first_id = first.id().clone();
        let second_id = second.id().clone();

        state.wizard.set_external_content_items(vec![first, second]);
        state.pending_external_content = vec![first_id.clone(), second_id.clone()];
        state.pending_model_names = vec![
            (first_id.clone(), "first-model".to_owned()),
            (second_id.clone(), "second-model".to_owned()),
        ];
        state.pending_model_name_index = Some(0);
        state.screen = WizardScreen::ExternalContent;

        state.confirm_external_model_name();

        assert_eq!(state.pending_model_name_index, Some(1));
        assert!(state.wizard.selected_external_content().is_empty());
        assert_eq!(state.screen, WizardScreen::ExternalContent);

        state.confirm_external_model_name();

        assert_eq!(state.pending_model_name_index, None);
        assert_eq!(
            state.wizard.selected_external_content(),
            &[first_id, second_id]
        );
        assert_eq!(state.wizard.model_realization_intents().len(), 2);
        assert_eq!(state.screen, WizardScreen::Storage);
    }

    #[test]
    fn cancels_external_model_naming_without_committing() {
        let mut state = TuiState::new();

        let item_id = model::ExternalContentItemId::new("test-model");
        state.pending_external_content = vec![item_id.clone()];
        state.pending_model_names = vec![(item_id, "pending-name".to_owned())];
        state.pending_model_name_index = Some(0);
        state.external_content_error = Some("test error".to_owned());
        state.external_content_focus = ExternalContentFocus::SaveAndContinue;
        state.screen = WizardScreen::ExternalContent;

        state.cancel_external_model_naming();

        assert!(state.pending_model_names.is_empty());
        assert_eq!(state.pending_model_name_index, None);
        assert_eq!(state.external_content_error, None);
        assert!(state.wizard.selected_external_content().is_empty());
        assert_eq!(
            state.external_content_focus,
            ExternalContentFocus::SaveAndContinue
        );
        assert_eq!(state.screen, WizardScreen::ExternalContent);
    }

    #[test]
    fn toggles_external_content_as_pending_without_committing() {
        let mut state = TuiState::new();

        let source_id = model::ContentSourceId::new("test-source");
        let first = model::ExternalContentItem::new(source_id.clone(), "/media/models/first.gguf");
        let second = model::ExternalContentItem::new(source_id, "/media/models/second.gguf");
        let second_id = second.id().clone();

        state.wizard.set_external_content_items(vec![first, second]);
        state.screen = WizardScreen::ExternalContent;
        state.external_content_focus = ExternalContentFocus::Items;

        assert_eq!(state.selected_external_content_index, 0);
        assert!(state.pending_external_content.is_empty());
        assert!(state.wizard.selected_external_content().is_empty());

        state.next_external_content();
        assert_eq!(state.selected_external_content_index, 1);

        state.toggle_external_content();

        assert_eq!(state.pending_external_content, vec![second_id]);
        assert!(state.wizard.selected_external_content().is_empty());

        state.toggle_external_content();

        assert!(state.pending_external_content.is_empty());
        assert!(state.wizard.selected_external_content().is_empty());
    }

    #[test]
    fn selects_storage_device_and_advances() {
        let mut state = TuiState::new();

        state.wizard.set_discovered_storage(vec![
            model::DiscoveredStorage::new("system-disk", model::StorageKind::System, "/dev/system"),
            model::DiscoveredStorage::new(
                "first-disk",
                model::StorageKind::Secondary,
                "/dev/first",
            ),
            model::DiscoveredStorage::new(
                "second-disk",
                model::StorageKind::Removable,
                "/dev/second",
            ),
        ]);

        state.screen = WizardScreen::Storage;
        state.selected_storage_index = 0;
        state.storage_focus = super::StorageFocus::Devices;

        state.next_storage();

        assert_eq!(state.selected_storage_index, 1);
        assert_eq!(state.storage_focus, super::StorageFocus::Devices);

        state.confirm_storage();

        assert_eq!(state.screen, WizardScreen::Administrator);
        assert_eq!(
            state
                .wizard
                .selected_storage()
                .expect("Storage should be selected")
                .as_str(),
            "second-disk"
        );
    }

    #[test]
    fn prepares_appliance_installation_from_completed_wizard_state() {
        struct TestStorageInspector;

        impl inspector::StorageInspector for TestStorageInspector {
            fn inspect(
                &self,
            ) -> Result<Vec<model::DiscoveredStorage>, inspector::StorageInspectError> {
                Ok(vec![
                    model::DiscoveredStorage::new(
                        "install-disk",
                        model::StorageKind::Secondary,
                        "/dev/install",
                    )
                    .with_size_bytes(512 * 1024 * 1024 * 1024),
                ])
            }
        }

        struct TestContentInspector;

        impl inspector::ContentInspector for TestContentInspector {
            fn inspect(
                &self,
                _source: &model::ContentSource,
            ) -> Result<Vec<model::DiscoveredContent>, inspector::ContentInspectError> {
                Ok(Vec::new())
            }

            fn items(
                &self,
                _content: &model::DiscoveredContent,
            ) -> Result<Vec<model::ExternalContentItem>, inspector::ContentInspectError>
            {
                Ok(Vec::new())
            }
        }

        let mut state = TuiState::new();

        let profile = state
            .appliance_profiles
            .first()
            .expect("test appliance profile should exist")
            .clone();
        state.wizard.set_profile_name(profile.name());

        let repository = state
            .wizard
            .content_repositories()
            .first()
            .expect("test content repository should exist")
            .clone();
        state
            .wizard
            .select_content_repository(repository.id().clone());

        state.wizard.set_discovered_storage(vec![
            model::DiscoveredStorage::new(
                "install-disk",
                model::StorageKind::Secondary,
                "/dev/install",
            )
            .with_size_bytes(512 * 1024 * 1024 * 1024),
        ]);
        state
            .wizard
            .select_storage(model::DiscoveredStorageId::new("install-disk"));
        state
            .wizard
            .set_user_identity("daia-admin", "DAIA Administrator");

        let registry = super::load_provider_registry()
            .expect("provider registry should load for preparation test");
        let engine = engine::Engine::from_registry(registry);

        let prepared = super::prepare_appliance_installation(
            &engine,
            &state.wizard,
            &state.appliance_profiles,
            &TestContentInspector,
            &TestStorageInspector,
        )
        .expect("completed wizard configuration should prepare installation");

        assert_eq!(
            prepared.installation().storage().id().as_str(),
            "install-disk"
        );
        assert_eq!(
            prepared.installation().storage().device_path(),
            std::path::Path::new("/dev/install")
        );
    }

    #[test]
    fn discovers_storage_and_excludes_system_device_from_selection() {
        struct TestStorageInspector;

        impl inspector::StorageInspector for TestStorageInspector {
            fn inspect(
                &self,
            ) -> Result<Vec<model::DiscoveredStorage>, inspector::StorageInspectError> {
                Ok(vec![
                    model::DiscoveredStorage::new(
                        "system-disk",
                        model::StorageKind::System,
                        "/dev/system",
                    )
                    .with_size_bytes(128 * 1024 * 1024 * 1024),
                    model::DiscoveredStorage::new(
                        "install-disk",
                        model::StorageKind::Secondary,
                        "/dev/install",
                    )
                    .with_size_bytes(512 * 1024 * 1024 * 1024),
                ])
            }
        }

        let engine = engine::Engine::from_registry(registry::Registry::new());
        let mut state = super::WizardState::new();

        super::discover_storage(&engine, &mut state, &TestStorageInspector)
            .expect("storage discovery should succeed");

        let selectable = state.selectable_storage().collect::<Vec<_>>();

        assert_eq!(selectable.len(), 1);
        assert_eq!(selectable[0].id().as_str(), "install-disk");
        assert_eq!(
            selectable[0].device_path(),
            std::path::Path::new("/dev/install")
        );
    }

    #[test]
    fn discovers_external_content_into_wizard_state() {
        struct TestContentInspector;

        impl inspector::ContentInspector for TestContentInspector {
            fn inspect(
                &self,
                source: &model::ContentSource,
            ) -> Result<Vec<model::DiscoveredContent>, inspector::ContentInspectError> {
                Ok(vec![model::DiscoveredContent::new(
                    source.id().clone(),
                    "/media/models",
                )])
            }

            fn items(
                &self,
                content: &model::DiscoveredContent,
            ) -> Result<Vec<model::ExternalContentItem>, inspector::ContentInspectError>
            {
                Ok(vec![model::ExternalContentItem::new(
                    content.source_id().clone(),
                    "/media/models/model.gguf",
                )])
            }
        }

        let repository_id = model::ContentRepositoryId::new("local-models");
        let repository = model::ContentRepository::with_sources(
            "local-models",
            "Local model files",
            vec![model::ContentSource::new(
                "local-models-directory",
                repository_id,
                "/media/models",
            )],
        );

        let engine = engine::Engine::from_registry(registry::Registry::new());
        let mut state = super::WizardState::new();

        super::discover_external_content(&engine, &mut state, &repository, &TestContentInspector)
            .expect("external content discovery should succeed");

        assert_eq!(state.external_content_items().len(), 1);
        assert_eq!(
            state.external_content_items()[0].path(),
            std::path::Path::new("/media/models/model.gguf")
        );
    }

    #[test]
    fn selects_content_repository_and_advances() {
        let mut state = TuiState::new();

        assert!(
            !state.wizard.content_repositories().is_empty(),
            "repository should provide content repositories"
        );

        state.screen = WizardScreen::ContentRepository;

        let selected_id = state.wizard.content_repositories()[0].id().clone();

        state.confirm_content_repository();

        assert_eq!(
            state.wizard.selected_content_repository(),
            Some(&selected_id)
        );
        assert_eq!(state.screen, WizardScreen::ExternalContent);
    }

    #[test]
    fn selects_continue_and_back_actions() {
        let mut state = TuiState::new();

        assert_eq!(state.screen_action, ScreenAction::Continue);

        state.next_screen_action();
        assert_eq!(state.screen_action, ScreenAction::Back);

        state.previous_screen_action();
        assert_eq!(state.screen_action, ScreenAction::Continue);
    }

    #[test]
    fn administrator_fields_advance_through_back() {
        let mut state = TuiState::new();
        state.screen = WizardScreen::Administrator;

        let fields = [
            AdministratorField::RootPasswordConfirmation,
            AdministratorField::Username,
            AdministratorField::DisplayName,
            AdministratorField::AdministratorPassword,
            AdministratorField::AdministratorPasswordConfirmation,
            AdministratorField::Back,
        ];

        for field in fields {
            state.next_administrator_field();
            assert_eq!(state.administrator_field, field);
        }

        state.next_administrator_field();
        assert_eq!(state.administrator_field, AdministratorField::Back);

        state.previous_administrator_field();
        assert_eq!(
            state.administrator_field,
            AdministratorField::AdministratorPasswordConfirmation
        );
    }

    #[test]
    fn administrator_back_returns_to_storage() {
        let mut state = TuiState::new();
        state.screen = WizardScreen::Administrator;
        state.administrator_field = AdministratorField::Back;

        state.confirm_administrator_field();

        assert_eq!(state.screen, WizardScreen::Storage);
        assert_eq!(state.administrator_field, AdministratorField::RootPassword);
    }

    #[test]
    fn final_administrator_confirmation_advances_to_review() {
        let mut state = TuiState::new();
        state.screen = WizardScreen::Administrator;
        state.administrator_field = AdministratorField::AdministratorPasswordConfirmation;
        state.credentials.root_password = "root-secret".to_owned();
        state.credentials.root_password_confirmation = "root-secret".to_owned();
        state.administrator_username = "daia-admin".to_owned();
        state.administrator_display_name = "DAIA Administrator".to_owned();
        state.credentials.administrator_password = "admin-secret".to_owned();
        state.credentials.administrator_password_confirmation = "admin-secret".to_owned();

        state.confirm_administrator_field();

        assert_eq!(state.screen, WizardScreen::Review);
        assert_eq!(state.administrator_error, None);

        let user = state
            .wizard
            .user_configuration()
            .expect("administrator identity should be committed");
        assert_eq!(user.username(), "daia-admin");
        assert_eq!(user.display_name(), "DAIA Administrator");
    }

    #[test]
    fn invalid_administrator_configuration_does_not_advance() {
        let mut state = TuiState::new();
        state.screen = WizardScreen::Administrator;
        state.administrator_field = AdministratorField::AdministratorPasswordConfirmation;

        state.confirm_administrator_field();

        assert_eq!(state.screen, WizardScreen::Administrator);
        assert_eq!(
            state.administrator_error.as_deref(),
            Some("Root password cannot be empty")
        );
        assert_eq!(state.administrator_field, AdministratorField::RootPassword);
        assert_eq!(state.wizard.user_configuration(), None);
    }

    #[test]
    fn mismatched_administrator_passwords_do_not_advance() {
        let mut state = TuiState::new();
        state.screen = WizardScreen::Administrator;
        state.administrator_field = AdministratorField::AdministratorPasswordConfirmation;
        state.credentials.root_password = "root-secret".to_owned();
        state.credentials.root_password_confirmation = "root-secret".to_owned();
        state.administrator_username = "daia-admin".to_owned();
        state.administrator_display_name = "DAIA Administrator".to_owned();
        state.credentials.administrator_password = "first".to_owned();
        state.credentials.administrator_password_confirmation = "second".to_owned();

        state.confirm_administrator_field();

        assert_eq!(state.screen, WizardScreen::Administrator);
        assert_eq!(
            state.administrator_error.as_deref(),
            Some("Administrator passwords do not match")
        );
        assert_eq!(
            state.administrator_field,
            AdministratorField::AdministratorPasswordConfirmation
        );
        assert_eq!(state.wizard.user_configuration(), None);
    }

    #[test]
    fn escape_navigation_moves_back_one_screen() {
        let mut state = TuiState::new();

        state.screen = WizardScreen::Profile;
        state.previous_screen();
        assert_eq!(state.screen, WizardScreen::Welcome);

        state.screen = WizardScreen::ContentRepository;
        state.previous_screen();
        assert_eq!(state.screen, WizardScreen::Profile);

        state.screen = WizardScreen::ExternalContent;
        state.previous_screen();
        assert_eq!(state.screen, WizardScreen::ContentRepository);

        state.screen = WizardScreen::Storage;
        state.previous_screen();
        assert_eq!(state.screen, WizardScreen::ExternalContent);

        state.screen = WizardScreen::Administrator;
        state.previous_screen();
        assert_eq!(state.screen, WizardScreen::Storage);

        state.screen = WizardScreen::Review;
        state.previous_screen();
        assert_eq!(state.screen, WizardScreen::Administrator);

        state.screen = WizardScreen::Welcome;
        state.previous_screen();
        assert_eq!(state.screen, WizardScreen::Welcome);
    }

    #[test]
    fn navigates_forward_and_backward_through_wizard_screens() {
        let mut state = TuiState::new();

        let screens = [
            WizardScreen::Profile,
            WizardScreen::ContentRepository,
            WizardScreen::ExternalContent,
            WizardScreen::Storage,
            WizardScreen::Administrator,
            WizardScreen::Review,
        ];

        for screen in screens {
            state.next_screen();
            assert_eq!(state.screen, screen);
        }

        state.next_screen();
        assert_eq!(state.screen, WizardScreen::Review);

        for screen in screens[..5].iter().rev() {
            state.previous_screen();
            assert_eq!(state.screen, *screen);
        }

        state.previous_screen();
        assert_eq!(state.screen, WizardScreen::Welcome);

        state.previous_screen();
        assert_eq!(state.screen, WizardScreen::Welcome);
    }
}
