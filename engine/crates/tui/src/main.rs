//! DAIA terminal user interface.

use application::{
    KEYBOARD_LAYOUTS, WizardState, load_appliance_profiles, load_content_repositories,
    load_package_repository, load_provider_registry,
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
use model::LocalizationConfiguration;
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Paragraph, Wrap},
};
use std::io::{self, Stdout, stdout};

/// Screens presented by the DAIA configuration wizard.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WizardScreen {
    Welcome,
    Localization,
    HardwareCheck,
    Profile,
    ContentRepository,
    ExternalContent,
    ApplianceIdentity,
    RootCredentials,
    Storage,
    Administrator,
    Review,
    ConfirmInstallation,
    Installing,
    InstallationComplete,
    InstallationFailed,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum ApplianceIdentityField {
    #[default]
    Hostname,
    DomainName,
    Continue,
    Back,
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
enum RootCredentialsField {
    #[default]
    RootPassword,
    RootPasswordConfirmation,
    Back,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum AdministratorField {
    #[default]
    Username,
    DisplayName,
    AdministratorPassword,
    AdministratorPasswordConfirmation,
    Back,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct HardwareSummary {
    architecture: String,
    logical_processor_count: usize,
    total_memory_bytes: u64,
    gpus: Vec<String>,
    accelerators: Vec<String>,
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
    localization_field: usize,
    localization_indices: [usize; 4],
    hardware: Option<HardwareSummary>,
    hardware_error: Option<String>,
    appliance_profiles: Vec<model::ApplianceProfile>,
    selected_profile_index: usize,
    selected_content_repository_index: usize,
    selected_external_content_index: usize,
    pending_external_content: Vec<model::ExternalContentItemId>,
    pending_model_names: Vec<(model::ExternalContentItemId, String)>,
    external_content_error: Option<String>,
    external_content_focus: ExternalContentFocus,
    appliance_hostname: String,
    appliance_domain_name: String,
    appliance_identity_field: ApplianceIdentityField,
    appliance_identity_error: Option<String>,
    selected_storage_index: usize,
    storage_error: Option<String>,
    storage_focus: StorageFocus,
    credentials: CredentialState,
    root_credentials_field: RootCredentialsField,
    root_credentials_error: Option<String>,
    administrator_username: String,
    administrator_display_name: String,
    administrator_field: AdministratorField,
    administrator_error: Option<String>,
    review_error: Option<String>,
    installation_action: InstallationAction,
    installation_requested: bool,
    welcome_action: WelcomeAction,
    screen_action: ScreenAction,
    screen: WizardScreen,
}

impl TuiState {
    fn new() -> Self {
        let appliance_profiles = load_appliance_profiles()
            .map(|repository| repository.profiles().to_vec())
            .unwrap_or_default();

        // Fresh installation offers bundled content only.
        // Recovery-specific repositories remain available to a separate workflow.
        let content_repositories = load_content_repositories()
            .unwrap_or_default()
            .into_iter()
            .filter(|repository| repository.id().as_str() == "bundled-models")
            .collect();

        let mut wizard = WizardState::new();
        wizard.set_content_repositories(content_repositories);

        Self {
            wizard,
            localization_field: 0,
            localization_indices: [0; 4],
            hardware: None,
            hardware_error: None,
            appliance_profiles,
            selected_profile_index: 0,
            selected_content_repository_index: 0,
            selected_external_content_index: 0,
            pending_external_content: Vec::new(),
            pending_model_names: Vec::new(),
            external_content_error: None,
            external_content_focus: ExternalContentFocus::ContinueWithoutLocalContent,
            appliance_hostname: String::new(),
            appliance_domain_name: String::new(),
            appliance_identity_field: ApplianceIdentityField::default(),
            appliance_identity_error: None,
            selected_storage_index: 0,
            storage_error: None,
            storage_focus: StorageFocus::Devices,
            credentials: CredentialState::default(),
            root_credentials_field: RootCredentialsField::default(),
            root_credentials_error: None,
            administrator_username: String::new(),
            administrator_display_name: String::new(),
            administrator_field: AdministratorField::default(),
            administrator_error: None,
            review_error: None,
            installation_action: InstallationAction::Back,
            installation_requested: false,
            welcome_action: WelcomeAction::default(),
            screen_action: ScreenAction::default(),
            screen: WizardScreen::Welcome,
        }
    }

    fn localization_options(&self, field: usize) -> &'static [&'static str] {
        const LANGUAGES: &[&str] = &["en", "hi", "fr", "de", "ja"];
        const COUNTRIES: &[&str] = &["IN", "US", "GB", "FR", "DE", "JP"];
        const LOCALES: &[&str] = &[
            "en_IN.UTF-8",
            "hi_IN.UTF-8",
            "en_US.UTF-8",
            "en_GB.UTF-8",
            "fr_FR.UTF-8",
            "de_DE.UTF-8",
            "ja_JP.UTF-8",
        ];

        match field {
            0 => LANGUAGES,
            1 => COUNTRIES,
            2 => LOCALES,
            _ => &[],
        }
    }

    fn next_localization_field(&mut self) {
        self.localization_field = (self.localization_field + 1).min(5);
    }

    fn previous_localization_field(&mut self) {
        self.localization_field = self.localization_field.saturating_sub(1);
    }

    fn cycle_localization_option(&mut self, forward: bool) {
        let field = self.localization_field;
        if field >= 4 {
            return;
        }

        let count = if field == 3 {
            KEYBOARD_LAYOUTS.len()
        } else {
            self.localization_options(field).len()
        };

        if count == 0 {
            return;
        }

        let current = self.localization_indices[field];
        self.localization_indices[field] = if forward {
            (current + 1) % count
        } else {
            (current + count - 1) % count
        };
    }

    fn confirm_localization(&mut self) {
        match self.localization_field {
            4 => {
                let language = self.localization_options(0)[self.localization_indices[0]];
                let country = self.localization_options(1)[self.localization_indices[1]];
                let locale = self.localization_options(2)[self.localization_indices[2]];
                let keyboard = KEYBOARD_LAYOUTS[self.localization_indices[3]].id;

                self.wizard.set_localization(LocalizationConfiguration::new(
                    language, country, locale, keyboard,
                ));
                self.next_screen();
            }
            5 => self.previous_screen(),
            _ => self.next_localization_field(),
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

        // Fresh installation must not discover models on external storage.
        // External repositories belong to the explicit recovery workflow.
        if repository_id.as_str() != "bundled-models" {
            self.external_content_error =
                Some("Fresh installation supports bundled models only.".to_owned());
            return;
        }

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
            InstallationAction::Install => {
                self.installation_requested = true;
            }
            InstallationAction::Back => {
                self.installation_requested = false;
                self.previous_screen();
            }
        }

        self.installation_action = InstallationAction::Back;
    }

    fn take_installation_request(&mut self) -> bool {
        std::mem::take(&mut self.installation_requested)
    }

    fn confirm_screen_action(&mut self) {
        match self.screen_action {
            ScreenAction::Continue
                if self.screen == WizardScreen::HardwareCheck && self.hardware.is_none() => {}
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

    fn discover_hardware(&mut self) {
        if self.hardware.is_some() {
            return;
        }

        self.hardware_error = None;

        let registry = match load_provider_registry() {
            Ok(registry) => registry,
            Err(error) => {
                self.hardware_error = Some(format!("Error loading provider registry: {error}"));
                return;
            }
        };

        let engine = Engine::from_registry(registry);

        match engine.discover_hardware() {
            Ok(hardware) => {
                self.hardware = Some(HardwareSummary {
                    architecture: hardware.cpu().architecture().to_owned(),
                    logical_processor_count: hardware.cpu().logical_processor_count(),
                    total_memory_bytes: hardware.memory().total_bytes(),
                    gpus: hardware
                        .gpus()
                        .iter()
                        .map(|gpu| gpu.identifier().to_owned())
                        .collect(),
                    accelerators: hardware
                        .accelerators()
                        .iter()
                        .map(|accelerator| accelerator.identifier().to_owned())
                        .collect(),
                });
            }
            Err(error) => {
                self.hardware_error = Some(format!("Error discovering hardware: {error}"));
            }
        }
    }

    fn enter_hardware_check(&mut self) {
        self.discover_hardware();
        self.screen_action = ScreenAction::Continue;
        self.screen = WizardScreen::HardwareCheck;
    }

    fn next_screen(&mut self) {
        if self.screen == WizardScreen::Localization {
            self.enter_hardware_check();
            return;
        }

        self.screen = match self.screen {
            WizardScreen::Welcome => WizardScreen::Localization,
            WizardScreen::Localization => WizardScreen::HardwareCheck,
            WizardScreen::HardwareCheck => WizardScreen::ApplianceIdentity,
            WizardScreen::ApplianceIdentity => WizardScreen::RootCredentials,
            WizardScreen::RootCredentials => WizardScreen::Administrator,
            WizardScreen::Administrator => WizardScreen::Profile,
            WizardScreen::Profile => WizardScreen::ContentRepository,
            WizardScreen::ContentRepository => WizardScreen::ExternalContent,
            WizardScreen::ExternalContent => WizardScreen::Storage,
            WizardScreen::Storage => WizardScreen::Review,
            WizardScreen::Review => WizardScreen::Review,
            WizardScreen::ConfirmInstallation => WizardScreen::ConfirmInstallation,
            WizardScreen::Installing => WizardScreen::Installing,
            WizardScreen::InstallationComplete => WizardScreen::InstallationComplete,
            WizardScreen::InstallationFailed => WizardScreen::InstallationFailed,
        };
    }

    fn push_appliance_identity_character(&mut self, character: char) {
        match self.appliance_identity_field {
            ApplianceIdentityField::Hostname => self.appliance_hostname.push(character),
            ApplianceIdentityField::DomainName => self.appliance_domain_name.push(character),
            ApplianceIdentityField::Continue | ApplianceIdentityField::Back => {}
        }

        self.appliance_identity_error = None;
    }

    fn pop_appliance_identity_character(&mut self) {
        match self.appliance_identity_field {
            ApplianceIdentityField::Hostname => {
                self.appliance_hostname.pop();
            }
            ApplianceIdentityField::DomainName => {
                self.appliance_domain_name.pop();
            }
            ApplianceIdentityField::Continue | ApplianceIdentityField::Back => {}
        }

        self.appliance_identity_error = None;
    }

    fn next_appliance_identity_field(&mut self) {
        self.appliance_identity_field = match self.appliance_identity_field {
            ApplianceIdentityField::Hostname => ApplianceIdentityField::DomainName,
            ApplianceIdentityField::DomainName => ApplianceIdentityField::Continue,
            ApplianceIdentityField::Continue => ApplianceIdentityField::Back,
            ApplianceIdentityField::Back => ApplianceIdentityField::Back,
        };
    }

    fn previous_appliance_identity_field(&mut self) {
        self.appliance_identity_field = match self.appliance_identity_field {
            ApplianceIdentityField::Hostname => ApplianceIdentityField::Hostname,
            ApplianceIdentityField::DomainName => ApplianceIdentityField::Hostname,
            ApplianceIdentityField::Continue => ApplianceIdentityField::DomainName,
            ApplianceIdentityField::Back => ApplianceIdentityField::Continue,
        };
    }

    fn confirm_appliance_identity(&mut self) {
        match self.appliance_identity_field {
            ApplianceIdentityField::Hostname => {
                self.appliance_identity_field = ApplianceIdentityField::DomainName;
            }
            ApplianceIdentityField::DomainName => {
                self.appliance_identity_field = ApplianceIdentityField::Continue;
            }
            ApplianceIdentityField::Continue => {
                let hostname = self.appliance_hostname.trim();
                let domain_name = self.appliance_domain_name.trim();

                if hostname.is_empty() {
                    self.appliance_identity_error = Some("Hostname cannot be empty.".to_owned());
                    self.appliance_identity_field = ApplianceIdentityField::Hostname;
                    return;
                }

                if domain_name.is_empty() {
                    self.appliance_identity_error = Some("Domain name cannot be empty.".to_owned());
                    self.appliance_identity_field = ApplianceIdentityField::DomainName;
                    return;
                }

                let valid_label = |label: &str| {
                    !label.is_empty()
                        && label.len() <= 63
                        && label
                            .as_bytes()
                            .first()
                            .is_some_and(u8::is_ascii_alphanumeric)
                        && label
                            .as_bytes()
                            .last()
                            .is_some_and(u8::is_ascii_alphanumeric)
                        && label
                            .bytes()
                            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
                };

                if !valid_label(hostname) {
                    self.appliance_identity_error =
                        Some("Hostname must be a valid DNS label (1-63 characters).".to_owned());
                    self.appliance_identity_field = ApplianceIdentityField::Hostname;
                    return;
                }

                if domain_name.len() > 253 || !domain_name.split('.').all(valid_label) {
                    self.appliance_identity_error =
                        Some("Domain name must contain valid DNS labels.".to_owned());
                    self.appliance_identity_field = ApplianceIdentityField::DomainName;
                    return;
                }

                self.wizard
                    .set_appliance_identity(hostname.to_owned(), domain_name.to_owned());
                self.appliance_identity_error = None;
                self.next_screen();
            }
            ApplianceIdentityField::Back => {
                self.appliance_identity_error = None;
                self.previous_screen();
            }
        }
    }

    fn push_root_credentials_character(&mut self, character: char) {
        match self.root_credentials_field {
            RootCredentialsField::RootPassword => {
                self.credentials.root_password.push(character);
            }
            RootCredentialsField::RootPasswordConfirmation => {
                self.credentials.root_password_confirmation.push(character);
            }
            RootCredentialsField::Back => {}
        }
    }

    fn pop_root_credentials_character(&mut self) {
        match self.root_credentials_field {
            RootCredentialsField::RootPassword => {
                self.credentials.root_password.pop();
            }
            RootCredentialsField::RootPasswordConfirmation => {
                self.credentials.root_password_confirmation.pop();
            }
            RootCredentialsField::Back => {}
        }
    }

    fn next_root_credentials_field(&mut self) {
        self.root_credentials_field = match self.root_credentials_field {
            RootCredentialsField::RootPassword => RootCredentialsField::RootPasswordConfirmation,
            RootCredentialsField::RootPasswordConfirmation => RootCredentialsField::Back,
            RootCredentialsField::Back => RootCredentialsField::Back,
        };
    }

    fn previous_root_credentials_field(&mut self) {
        self.root_credentials_field = match self.root_credentials_field {
            RootCredentialsField::RootPassword => RootCredentialsField::RootPassword,
            RootCredentialsField::RootPasswordConfirmation => RootCredentialsField::RootPassword,
            RootCredentialsField::Back => RootCredentialsField::RootPasswordConfirmation,
        };
    }

    fn validate_root_credentials(&mut self) -> bool {
        let error = if self.credentials.root_password.is_empty() {
            Some((
                RootCredentialsField::RootPassword,
                "Root password cannot be empty",
            ))
        } else if self.credentials.root_password != self.credentials.root_password_confirmation {
            Some((
                RootCredentialsField::RootPasswordConfirmation,
                "Root passwords do not match",
            ))
        } else {
            None
        };

        if let Some((field, error)) = error {
            self.root_credentials_field = field;
            self.root_credentials_error = Some(error.to_owned());
            return false;
        }

        self.root_credentials_error = None;
        true
    }

    fn confirm_root_credentials_field(&mut self) {
        match self.root_credentials_field {
            RootCredentialsField::RootPasswordConfirmation => {
                if self.validate_root_credentials() {
                    self.next_screen();
                }
            }
            RootCredentialsField::Back => {
                self.previous_screen();
                self.root_credentials_field = RootCredentialsField::RootPassword;
            }
            RootCredentialsField::RootPassword => {
                self.next_root_credentials_field();
            }
        }
    }

    fn push_administrator_character(&mut self, character: char) {
        match self.administrator_field {
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
        let error = if self.administrator_username.trim().is_empty() {
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
                self.administrator_field = AdministratorField::Username;
            }
            _ => {
                self.next_administrator_field();
            }
        }
    }

    fn previous_administrator_field(&mut self) {
        self.administrator_field = match self.administrator_field {
            AdministratorField::Back => AdministratorField::AdministratorPasswordConfirmation,
            AdministratorField::Username => AdministratorField::Username,
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
                            let Some(stem) =
                                item.path().file_stem().and_then(|value| value.to_str())
                            else {
                                self.external_content_error =
                                    Some("Selected model has no valid filename.".to_owned());
                                self.pending_model_names.clear();
                                return;
                            };

                            let name = stem
                                .to_ascii_lowercase()
                                .chars()
                                .map(|character| {
                                    if character.is_ascii_alphanumeric()
                                        || matches!(character, '-' | '_' | '.')
                                    {
                                        character
                                    } else {
                                        '-'
                                    }
                                })
                                .collect::<String>();

                            if name.is_empty()
                                || !name
                                    .chars()
                                    .any(|character| character.is_ascii_alphanumeric())
                                || self
                                    .pending_model_names
                                    .iter()
                                    .any(|(_, existing)| existing == &name)
                            {
                                self.external_content_error =
                                    Some(format!("Invalid or duplicate model identifier: {name}"));
                                self.pending_model_names.clear();
                                return;
                            }

                            self.pending_model_names.push((item.id().clone(), name));
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

                let realization_intents = self
                    .pending_model_names
                    .iter()
                    .map(|(item_id, name)| {
                        model::ModelRealizationIntent::new(
                            model::ModelRealizationId::new(name),
                            model::InferenceEngineId::ollama(),
                            item_id.clone(),
                        )
                    })
                    .collect::<Vec<_>>();

                self.wizard
                    .select_external_content(self.pending_external_content.clone());
                self.wizard
                    .set_model_realization_intents(realization_intents);
                self.enter_storage();
            }
            ExternalContentFocus::ContinueWithoutLocalContent => {
                // Optional models may be installed later. Never carry a
                // previously selected model into firstboot after skipping.
                self.pending_external_content.clear();
                self.pending_model_names.clear();
                self.wizard.select_external_content(Vec::new());
                self.wizard.set_model_realization_intents(Vec::new());
                self.external_content_error = None;
                self.enter_storage();
            }
            ExternalContentFocus::Back => {
                self.previous_screen();
            }
        }
    }

    fn previous_screen(&mut self) {
        self.screen = match self.screen {
            WizardScreen::Welcome => WizardScreen::Welcome,
            WizardScreen::Localization => WizardScreen::Welcome,
            WizardScreen::HardwareCheck => WizardScreen::Localization,
            WizardScreen::ApplianceIdentity => WizardScreen::HardwareCheck,
            WizardScreen::RootCredentials => WizardScreen::ApplianceIdentity,
            WizardScreen::Administrator => WizardScreen::RootCredentials,
            WizardScreen::Profile => WizardScreen::Administrator,
            WizardScreen::ContentRepository => WizardScreen::Profile,
            WizardScreen::ExternalContent => WizardScreen::ContentRepository,
            WizardScreen::Storage => WizardScreen::ExternalContent,
            WizardScreen::Review => WizardScreen::Storage,
            WizardScreen::ConfirmInstallation => WizardScreen::Review,
            WizardScreen::Installing => WizardScreen::Installing,
            WizardScreen::InstallationComplete => WizardScreen::InstallationComplete,
            WizardScreen::InstallationFailed => WizardScreen::InstallationFailed,
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

fn execute_appliance_installation<E>(
    engine: &Engine,
    prepared: &engine::PreparedApplianceInstallation,
    executor: &mut E,
) -> Result<(), E::Error>
where
    E: engine::InstallationOperationExecutor,
{
    engine.execute_appliance_installation(prepared, executor)
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
        WizardScreen::Localization => "Localization",
        WizardScreen::HardwareCheck => "Hardware Check",
        WizardScreen::Profile => "Appliance Profile",
        WizardScreen::ContentRepository => "Content Repository",
        WizardScreen::ExternalContent => "External Content",
        WizardScreen::ApplianceIdentity => "Appliance Identity",
        WizardScreen::RootCredentials => "Root Credentials",
        WizardScreen::Storage => "Installation Storage",
        WizardScreen::Administrator => "Administrator",
        WizardScreen::Review => "Review",
        WizardScreen::ConfirmInstallation => "Confirm Installation",
        WizardScreen::Installing => "Installing",
        WizardScreen::InstallationComplete => "Installation Complete",
        WizardScreen::InstallationFailed => "Installation Failed",
    }
}

const DAIA_SIDEBAR_WIDTH: u16 = 26;
const DAIA_MIN_TWO_COLUMN_WIDTH: u16 = 80;
const DAIA_MIN_TWO_COLUMN_BODY_HEIGHT: u16 = 19;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WizardLayout {
    SingleColumn { content: Rect },
    TwoColumn { sidebar: Rect, content: Rect },
}

fn calculate_wizard_layout(area: Rect) -> WizardLayout {
    if area.width < DAIA_MIN_TWO_COLUMN_WIDTH || area.height < DAIA_MIN_TWO_COLUMN_BODY_HEIGHT {
        return WizardLayout::SingleColumn { content: area };
    }

    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(DAIA_SIDEBAR_WIDTH), Constraint::Min(0)])
        .split(area);

    WizardLayout::TwoColumn {
        sidebar: columns[0],
        content: columns[1],
    }
}

/// Keep a selected rendered row visible inside a vertical viewport.
///
/// `selected_start` and `selected_height` are measured in terminal rows
/// after wrapping, not in source-text lines.
fn scroll_offset_for_selection(
    current_offset: usize,
    selected_start: usize,
    selected_height: usize,
    content_height: usize,
    viewport_height: usize,
) -> usize {
    if viewport_height == 0 || content_height == 0 {
        return 0;
    }

    let max_offset = content_height.saturating_sub(viewport_height);
    let mut offset = current_offset.min(max_offset);

    if selected_height == 0 {
        return offset;
    }

    let selected_start = selected_start.min(content_height.saturating_sub(1));
    let selected_end = selected_start
        .saturating_add(selected_height)
        .min(content_height);

    if selected_start < offset {
        offset = selected_start;
    } else if selected_end > offset.saturating_add(viewport_height) {
        if selected_height > viewport_height {
            offset = selected_start;
        } else {
            offset = selected_end.saturating_sub(viewport_height);
        }
    }

    offset.min(max_offset)
}

/// Calculate the viewport offset from the focused logical line.
///
/// Ratatui measures the wrapped height, so long paths and descriptions
/// are accounted for before choosing the visible viewport.
fn focused_configuration_scroll(content: &str, width: u16, height: u16) -> u16 {
    if width == 0 || height == 0 {
        return 0;
    }

    let lines: Vec<&str> = content.split('\n').collect();

    // The combined single-column layout also contains the selected
    // navigation screen. The last marker belongs to the active field.
    let Some(focused_index) = lines
        .iter()
        .rposition(|line| line.trim_start().starts_with("> "))
    else {
        return 0;
    };

    let wrapped_height = |value: &str| {
        Paragraph::new(value)
            .wrap(Wrap { trim: false })
            .line_count(width)
    };

    let selected_start = if focused_index == 0 {
        0
    } else {
        lines[..focused_index]
            .iter()
            .map(|line| wrapped_height(line))
            .sum()
    };

    let selected_height = wrapped_height(lines[focused_index]);
    let content_height = wrapped_height(content);

    scroll_offset_for_selection(
        0,
        selected_start,
        selected_height,
        content_height,
        usize::from(height),
    )
    .min(usize::from(u16::MAX)) as u16
}

fn style_wizard_detail(
    content: &str,
    navigation_lines: usize,
    background: Color,
    secondary: Color,
) -> ratatui::text::Text<'static> {
    let selected_style = Style::default()
        .fg(Color::White)
        .bg(Color::Rgb(24, 63, 104))
        .add_modifier(Modifier::BOLD);

    let navigation_style = Style::default()
        .fg(Color::Rgb(139, 203, 255))
        .bg(background)
        .add_modifier(Modifier::BOLD);

    let normal_style = Style::default().fg(secondary).bg(background);

    let lines = content
        .split('\n')
        .enumerate()
        .map(|(index, line)| {
            let selected = line.trim_start().starts_with("> ");

            let style = if selected && index < navigation_lines {
                navigation_style
            } else if selected {
                selected_style
            } else {
                normal_style
            };

            ratatui::text::Line::styled(line.to_owned(), style)
        })
        .collect::<Vec<_>>();

    ratatui::text::Text::from(lines)
}

fn render(frame: &mut ratatui::Frame<'_>, state: &TuiState) {
    let background = Color::Rgb(11, 32, 57);
    let primary = Color::Rgb(255, 255, 255);
    let secondary = Color::Rgb(185, 208, 227);
    let accent = Color::Rgb(139, 203, 255);
    let border = Color::Rgb(52, 88, 120);

    let base_style = Style::default().fg(primary).bg(background);
    frame.render_widget(Block::default().style(base_style), frame.area());

    let areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(3),
        ])
        .split(frame.area());

    let title = Paragraph::new("Debian AI Appliance")
        .style(Style::default().fg(accent).bg(background))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border))
                .title("DAIA"),
        );

    let profile = state.wizard.profile_name().unwrap_or("Not selected");

    let screens = [
        WizardScreen::Welcome,
        WizardScreen::Localization,
        WizardScreen::HardwareCheck,
        WizardScreen::ApplianceIdentity,
        WizardScreen::RootCredentials,
        WizardScreen::Administrator,
        WizardScreen::Profile,
        WizardScreen::ContentRepository,
        WizardScreen::ExternalContent,
        WizardScreen::Storage,
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
        WizardScreen::Localization => {
            let labels = ["Language", "Country / Region", "Locale", "Keyboard"];
            let mut rows = Vec::new();

            for (field, label) in labels.iter().enumerate() {
                let value = if field == 3 {
                    KEYBOARD_LAYOUTS[state.localization_indices[field]].label
                } else {
                    state.localization_options(field)[state.localization_indices[field]]
                };

                let marker = if state.localization_field == field {
                    ">"
                } else {
                    " "
                };

                rows.push(format!("{marker} {label}: {value}"));
            }

            let continue_marker = if state.localization_field == 4 { ">" } else { " " };
            let back_marker = if state.localization_field == 5 { ">" } else { " " };

            format!(
                "Localization\n\n{}\n\n{continue_marker} Continue\n{back_marker} Back",
                rows.join("\n")
            )
        }
        WizardScreen::HardwareCheck => {
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

            if let Some(hardware) = state.hardware.as_ref() {
                let memory_gib =
                    hardware.total_memory_bytes as f64 / (1024.0 * 1024.0 * 1024.0);

                let gpus = if hardware.gpus.is_empty() {
                    "None detected".to_owned()
                } else {
                    hardware.gpus.join(", ")
                };

                let accelerators = if hardware.accelerators.is_empty() {
                    "None detected".to_owned()
                } else {
                    hardware.accelerators.join(", ")
                };

                format!(
                    "Hardware Check\n\n                     Architecture : {}\n                     Processors   : {} logical\n                     Memory       : {:.1} GiB\n                     GPUs         : {}\n                     Accelerators : {}\n\n                     {continue_marker} Continue\n                     {back_marker} Back",
                    hardware.architecture,
                    hardware.logical_processor_count,
                    memory_gib,
                    gpus,
                    accelerators,
                )
            } else {
                let error = state
                    .hardware_error
                    .as_deref()
                    .unwrap_or("Hardware information is unavailable.");

                format!(
                    "Hardware Check\n\n                     {error}\n\n                     Hardware discovery must succeed before continuing.\n\n                     {back_marker} Back"
                )
            }
        }
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
                        "Bundled Models\n\n                     No bundled models were found on the installation media.\n\n                     {continue_marker} Skip — Install Later\n                     {back_marker} Back\n\n                     Models and other content can be added after installation.{error}"
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
                        "Bundled Models\n\n                     Models available on the installation media:\n\n                     {items}\n\n                     {save_marker} Install Now\n                     {continue_marker} Skip — Install Later\n                     {back_marker} Back\n\n                     Space toggles the highlighted content. Models and other content can also be added after installation.{error}"
                    )
                }
        }
        WizardScreen::ApplianceIdentity => {
            let hostname = if state.appliance_hostname.is_empty() {
                "Not configured"
            } else {
                &state.appliance_hostname
            };

            let domain_name = if state.appliance_domain_name.is_empty() {
                "Not configured"
            } else {
                &state.appliance_domain_name
            };

            let fqdn = if state.appliance_hostname.trim().is_empty()
                || state.appliance_domain_name.trim().is_empty()
            {
                "Not configured".to_owned()
            } else {
                format!(
                    "{}.{}",
                    state.appliance_hostname.trim(),
                    state.appliance_domain_name.trim()
                )
            };

            let marker = |field| {
                if state.appliance_identity_field == field {
                    ">"
                } else {
                    " "
                }
            };

            let error = state
                .appliance_identity_error
                .as_deref()
                .map_or(String::new(), |error| format!("\n\nError: {error}"));

            format!(
                "Appliance Identity\n\n                 Configure the network identity of this DAIA appliance.\n\n                 {} Hostname: {hostname}\n                 {} Domain name: {domain_name}\n                   FQDN: {fqdn}\n\n                 {} Continue\n                 {} Back{error}",
                marker(ApplianceIdentityField::Hostname),
                marker(ApplianceIdentityField::DomainName),
                marker(ApplianceIdentityField::Continue),
                marker(ApplianceIdentityField::Back),
            )
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
        WizardScreen::RootCredentials => {
            let root_password = "*".repeat(state.credentials.root_password.chars().count());
            let root_confirmation =
                "*".repeat(state.credentials.root_password_confirmation.chars().count());

            let marker = |field| {
                if state.root_credentials_field == field {
                    ">"
                } else {
                    " "
                }
            };

            let error = state
                .root_credentials_error
                .as_deref()
                .map_or(String::new(), |error| {
                    format!("\n\n                 Error: {error}")
                });

            format!(
                "Root Credentials\n\n                 {} Root password: {root_password}\n                 {} Confirm password: {root_confirmation}\n\n                 {} Back{error}",
                marker(RootCredentialsField::RootPassword),
                marker(RootCredentialsField::RootPasswordConfirmation),
                marker(RootCredentialsField::Back),
            )
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
                "Administrator\n\n                 {} Username: {username}\n                 {} Display name: {display_name}\n                 {} Password: {administrator_password}\n                 {} Confirm password: {administrator_confirmation}\n\n                 {} Back{error}",
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

            let (hostname, domain_name, fqdn) =
                if let Some(identity) = state.wizard.appliance_identity() {
                    (
                        identity.hostname().to_owned(),
                        identity.domain_name().to_owned(),
                        identity.fqdn(),
                    )
                } else {
                    (
                        "Not configured".to_owned(),
                        "Not configured".to_owned(),
                        "Not configured".to_owned(),
                    )
                };

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
                 Hostname: {hostname}\n\
                 Domain: {domain_name}\n\
                 FQDN: {fqdn}\n\
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
        WizardScreen::Installing => {
            "Installing DAIA\n\n             Installation is in progress.\n             Do not power off the computer."
                .to_owned()
        }
        WizardScreen::InstallationComplete => {
            let error = state
                .review_error
                .as_deref()
                .map(|error| format!("\n\nReboot error: {error}"))
                .unwrap_or_default();

            format!(
                "Installation Complete\n\n\
                 DAIA was installed successfully.\n\n\
                 Remove installation media before rebooting.\n\n\
                 Press R to reboot or Enter to exit.{error}"
            )
        }
        WizardScreen::InstallationFailed => {
            let error = state
                .review_error
                .as_deref()
                .unwrap_or("Installation failed.");

            format!(
                "Installation Failed\n\n                 DAIA could not complete the installation.\n\n                 Error: {error}\n\n                 Press Enter to exit the installer."
            )
        }
    };

    let welcome_text = if state.screen == WizardScreen::Welcome {
        let selected = Style::default()
            .fg(Color::White)
            .bg(Color::Rgb(24, 63, 104))
            .add_modifier(Modifier::BOLD);

        let normal = Style::default().fg(secondary).bg(background);

        let start_style = if state.welcome_action == WelcomeAction::Start {
            selected
        } else {
            normal
        };

        let quit_style = if state.welcome_action == WelcomeAction::Quit {
            selected
        } else {
            normal
        };

        ratatui::text::Text::from(vec![
            ratatui::text::Line::from(""),
            ratatui::text::Line::styled(
                "  Welcome to Debian AI Appliance",
                Style::default()
                    .fg(accent)
                    .bg(background)
                    .add_modifier(Modifier::BOLD),
            ),
            ratatui::text::Line::styled("  Your AI. Your Machine.", normal),
            ratatui::text::Line::from(""),
            ratatui::text::Line::styled("  Start Installation", start_style),
            ratatui::text::Line::styled("  Quit", quit_style),
        ])
    } else {
        style_wizard_detail(
            &format!("{navigation}\n\n{detail}"),
            screens.len(),
            background,
            secondary,
        )
    };

    let body = Paragraph::new(welcome_text)
        .style(base_style)
        .wrap(Wrap { trim: false })
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border))
                .title("Configuration"),
        );

    let controls = match state.screen {
        WizardScreen::Welcome => "↑ / ↓: Navigate    Enter: Select",
        WizardScreen::Localization => {
            "↑ / ↓: Fields    ← / →: Change selection    Enter: Select    Esc: Back"
        }
        WizardScreen::ExternalContent if !state.wizard.external_content_items().is_empty() => {
            "↑ / ↓: Navigate    Space: Toggle    Enter: Select    Esc: Back"
        }
        WizardScreen::ExternalContent => "↑ / ↓: Navigate    Enter: Select    Esc: Back",
        WizardScreen::RootCredentials | WizardScreen::Administrator => {
            "↑ / ↓: Navigate    Enter: Select    Esc: Back    Backspace: Edit"
        }
        WizardScreen::ConfirmInstallation => "↑ / ↓: Navigate    Enter: Select    Esc: Back",
        WizardScreen::Installing => "Installation in progress",
        WizardScreen::InstallationComplete => "R: Reboot    Enter: Exit",
        WizardScreen::InstallationFailed => "Enter: Exit",
        _ => "↑ / ↓: Navigate    Enter: Select    Esc: Back",
    };

    let controls = Paragraph::new(controls)
        .style(Style::default().fg(secondary).bg(background))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border)),
        );

    frame.render_widget(title, areas[0]);

    if state.screen == WizardScreen::Welcome {
        frame.render_widget(body, areas[1]);
    } else {
        match calculate_wizard_layout(areas[1]) {
            WizardLayout::SingleColumn { content } => {
                let combined = format!("{navigation}\n\n{detail}");
                let offset = focused_configuration_scroll(
                    &combined,
                    content.width.saturating_sub(2),
                    content.height.saturating_sub(2),
                );

                frame.render_widget(body.scroll((offset, 0)), content);
            }
            WizardLayout::TwoColumn { sidebar, content } => {
                let navigation_text =
                    style_wizard_detail(&navigation, screens.len(), background, secondary);

                let sidebar_widget = Paragraph::new(navigation_text).style(base_style).block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(border))
                        .title("Navigation"),
                );

                let detail_text = style_wizard_detail(&detail, 0, background, secondary);

                let offset = focused_configuration_scroll(
                    &detail,
                    content.width.saturating_sub(2),
                    content.height.saturating_sub(2),
                );

                let content_widget = Paragraph::new(detail_text)
                    .style(base_style)
                    .wrap(Wrap { trim: false })
                    .scroll((offset, 0))
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .border_style(Style::default().fg(border))
                            .title("Configuration"),
                    );

                frame.render_widget(sidebar_widget, sidebar);
                frame.render_widget(content_widget, content);
            }
        }
    }

    frame.render_widget(controls, areas[2]);
}

struct TerminalSession {
    terminal: Terminal<CrosstermBackend<Stdout>>,
}

impl TerminalSession {
    fn enter() -> io::Result<Self> {
        enable_raw_mode()?;

        let mut output = stdout();
        if let Err(error) = execute!(output, EnterAlternateScreen) {
            let _ = disable_raw_mode();
            return Err(error);
        }

        let backend = CrosstermBackend::new(output);
        match Terminal::new(backend) {
            Ok(mut terminal) => {
                if let Err(error) = terminal.clear() {
                    let _ = disable_raw_mode();
                    let _ = execute!(terminal.backend_mut(), LeaveAlternateScreen);
                    return Err(error);
                }
                Ok(Self { terminal })
            }
            Err(error) => {
                let _ = disable_raw_mode();
                let mut output = stdout();
                let _ = execute!(output, LeaveAlternateScreen);
                Err(error)
            }
        }
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(self.terminal.backend_mut(), LeaveAlternateScreen);
        let _ = self.terminal.show_cursor();
    }
}

fn request_reboot_with(
    mut run_command: impl FnMut(&mut std::process::Command) -> io::Result<std::process::ExitStatus>,
) -> io::Result<()> {
    let status = run_command(std::process::Command::new("systemctl").arg("reboot"))?;

    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "systemctl reboot exited with {status}"
        )))
    }
}

fn request_reboot() -> io::Result<()> {
    request_reboot_with(|command| command.status())
}

fn run() -> io::Result<()> {
    let mut session = TerminalSession::enter()?;
    let terminal = &mut session.terminal;
    let mut state = TuiState::new();

    loop {
        terminal.draw(|frame| render(frame, &state))?;

        if state.take_installation_request() {
            state.review_error = None;
            state.screen = WizardScreen::Installing;
            terminal.draw(|frame| render(frame, &state))?;

            let registry = match load_provider_registry() {
                Ok(registry) => registry,
                Err(error) => {
                    state.review_error = Some(format!("Error loading provider registry: {error}"));
                    state.screen = WizardScreen::InstallationFailed;
                    continue;
                }
            };

            let engine = Engine::from_registry(registry);
            let content_inspector = LocalFilesystemContentInspector::new();
            let storage_inspector = LinuxStorageInspector::new();

            let prepared = match prepare_appliance_installation(
                &engine,
                &state.wizard,
                &state.appliance_profiles,
                &content_inspector,
                &storage_inspector,
            ) {
                Ok(prepared) => prepared,
                Err(error) => {
                    state.review_error = Some(error);
                    state.screen = WizardScreen::InstallationFailed;
                    continue;
                }
            };

            if let Err(error) = load_package_repository() {
                state.review_error = Some(format!("Error loading package repository: {error}"));
                state.screen = WizardScreen::InstallationFailed;
                continue;
            }

            if let Err(error) = engine::validate_installation_commands() {
                state.review_error =
                    Some(format!("Error validating installation commands: {error}"));
                state.screen = WizardScreen::InstallationFailed;
                continue;
            }

            let root_password = state.credentials.root_password.clone();
            let administrator_password = state.credentials.administrator_password.clone();

            let mut executor = engine::SystemInstallationOperationExecutor::new(
                root_password,
                administrator_password,
            );

            if let Err(error) = execute_appliance_installation(&engine, &prepared, &mut executor) {
                state.review_error = Some(format!("Installation failed: {error}"));
                state.screen = WizardScreen::InstallationFailed;
                continue;
            }

            state.review_error = None;
            state.screen = WizardScreen::InstallationComplete;
            continue;
        }

        let Event::Key(key) = event::read()? else {
            continue;
        };

        match key.code {
            KeyCode::Esc
                if state.screen != WizardScreen::Welcome
                    && state.screen != WizardScreen::Installing
                    && state.screen != WizardScreen::InstallationComplete
                    && state.screen != WizardScreen::InstallationFailed =>
            {
                state.previous_screen();
                state.screen_action = ScreenAction::Continue;

                if state.screen == WizardScreen::RootCredentials {
                    state.root_credentials_field = RootCredentialsField::RootPassword;
                }

                if state.screen == WizardScreen::Administrator {
                    state.administrator_field = AdministratorField::Username;
                }
            }
            KeyCode::Down if state.screen == WizardScreen::Localization => {
                state.next_localization_field();
            }
            KeyCode::Up if state.screen == WizardScreen::Localization => {
                state.previous_localization_field();
            }
            KeyCode::Right if state.screen == WizardScreen::Localization => {
                state.cycle_localization_option(true);
            }
            KeyCode::Left if state.screen == WizardScreen::Localization => {
                state.cycle_localization_option(false);
            }
            KeyCode::Enter if state.screen == WizardScreen::Localization => {
                state.confirm_localization();
            }
            KeyCode::Down if state.screen == WizardScreen::Welcome => {
                state.next_welcome_action();
            }
            KeyCode::Up if state.screen == WizardScreen::Welcome => {
                state.previous_welcome_action();
            }
            KeyCode::Char('r' | 'R') if state.screen == WizardScreen::InstallationComplete => {
                state.review_error = None;

                match request_reboot() {
                    Ok(()) => break Ok(()),
                    Err(error) => state.review_error = Some(error.to_string()),
                }
            }
            KeyCode::Enter
                if matches!(
                    state.screen,
                    WizardScreen::InstallationComplete | WizardScreen::InstallationFailed
                ) =>
            {
                break Ok(());
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
            KeyCode::Down if state.screen == WizardScreen::ExternalContent => {
                state.next_external_content();
            }
            KeyCode::Up if state.screen == WizardScreen::ExternalContent => {
                state.previous_external_content();
            }
            KeyCode::Char(' ') if state.screen == WizardScreen::ExternalContent => {
                state.toggle_external_content();
            }
            KeyCode::Down if state.screen == WizardScreen::ApplianceIdentity => {
                state.next_appliance_identity_field();
            }
            KeyCode::Up if state.screen == WizardScreen::ApplianceIdentity => {
                state.previous_appliance_identity_field();
            }
            KeyCode::Backspace if state.screen == WizardScreen::ApplianceIdentity => {
                state.pop_appliance_identity_character();
            }
            KeyCode::Char(character) if state.screen == WizardScreen::ApplianceIdentity => {
                state.push_appliance_identity_character(character);
            }
            KeyCode::Enter if state.screen == WizardScreen::ApplianceIdentity => {
                state.confirm_appliance_identity();
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
            KeyCode::Down if state.screen == WizardScreen::RootCredentials => {
                state.next_root_credentials_field();
            }
            KeyCode::Up if state.screen == WizardScreen::RootCredentials => {
                state.previous_root_credentials_field();
            }
            KeyCode::Backspace if state.screen == WizardScreen::RootCredentials => {
                state.pop_root_credentials_character();
            }
            KeyCode::Char(character) if state.screen == WizardScreen::RootCredentials => {
                state.push_root_credentials_character(character);
            }
            KeyCode::Enter if state.screen == WizardScreen::RootCredentials => {
                state.confirm_root_credentials_field();
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
    }
}

fn main() -> io::Result<()> {
    run()
}

#[cfg(test)]
mod midnight_blue_rendering_tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend};

    fn rendered_text(terminal: &Terminal<TestBackend>, width: u16, height: u16) -> String {
        let buffer = terminal.backend().buffer();

        (0..height)
            .map(|y| {
                (0..width)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn render_scrolled_fixture(content: &str, terminal_width: u16, terminal_height: u16) -> String {
        let mut terminal = Terminal::new(TestBackend::new(terminal_width, terminal_height))
            .expect("test terminal");

        terminal
            .draw(|frame| {
                let areas = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Length(3),
                        Constraint::Min(1),
                        Constraint::Length(3),
                    ])
                    .split(frame.area());

                let content_area = match calculate_wizard_layout(areas[1]) {
                    WizardLayout::SingleColumn { content } => content,
                    WizardLayout::TwoColumn { content, .. } => content,
                };

                let width = content_area.width.saturating_sub(2);
                let height = content_area.height.saturating_sub(2);

                let offset = focused_configuration_scroll(content, width, height);

                let paragraph = Paragraph::new(content)
                    .wrap(Wrap { trim: false })
                    .scroll((offset, 0))
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title("Configuration"),
                    );

                frame.render_widget(paragraph, content_area);
            })
            .expect("render fixture");

        rendered_text(&terminal, terminal_width, terminal_height)
    }

    #[test]
    fn scroll_renders_bottom_selection_at_all_supported_sizes() {
        for (width, height) in [(60, 20), (80, 25), (100, 30), (120, 35)] {
            let content = (0..45)
                .map(|index| {
                    if index == 44 {
                        "> FINAL_SELECTION_VISIBLE".to_owned()
                    } else {
                        format!("  Configuration item {index:02}")
                    }
                })
                .collect::<Vec<_>>()
                .join("\n");

            let rendered = render_scrolled_fixture(&content, width, height);

            assert!(
                rendered.contains("FINAL_SELECTION_VISIBLE"),
                "focused bottom selection invisible at {width}x{height}"
            );
        }
    }

    #[test]
    fn scroll_renders_selection_after_wrapped_model_paths() {
        for (width, height) in [(60, 20), (80, 25), (100, 30), (120, 35)] {
            let mut rows = vec!["Bundled Models".to_owned(), String::new()];

            for index in 0..18 {
                rows.push(format!(
                    "  /media/daia/models/archive/{index:02}/\
                     very-long-model-directory/\
                     qwen2.5-instruct-q4_k_m.gguf"
                ));
            }

            rows.push("> INSTALL_NOW_VISIBLE".to_owned());

            let rendered = render_scrolled_fixture(&rows.join("\n"), width, height);

            assert!(
                rendered.contains("INSTALL_NOW_VISIBLE"),
                "focused action invisible after model paths at {width}x{height}"
            );
        }
    }

    #[test]
    fn scroll_renders_selection_after_wrapped_profile_descriptions() {
        for (width, height) in [(60, 20), (80, 25), (100, 30), (120, 35)] {
            let mut rows = vec!["Choose the appliance profile".to_owned(), String::new()];

            for index in 0..12 {
                rows.push(format!("  Appliance Profile {index}"));
                rows.push(
                    "    A desktop AI appliance with local inference, \
                     model management, external content import, \
                     and configurable services."
                        .to_owned(),
                );
                rows.push(
                    "    Capabilities: desktop, inference, \
                     model-import, content-discovery"
                        .to_owned(),
                );
                rows.push(String::new());
            }

            rows.push("> PROFILE_SELECTION_VISIBLE".to_owned());

            let rendered = render_scrolled_fixture(&rows.join("\n"), width, height);

            assert!(
                rendered.contains("PROFILE_SELECTION_VISIBLE"),
                "focused profile invisible at {width}x{height}"
            );
        }
    }

    #[test]
    fn scroll_renders_continue_and_back_actions() {
        for (width, height) in [(60, 20), (80, 25), (100, 30), (120, 35)] {
            let summary = (0..35)
                .map(|index| format!("  Review item {index:02}"))
                .collect::<Vec<_>>()
                .join("\n");

            for action in [
                "> CONTINUE_ACTION_VISIBLE\n  Back",
                "  Continue\n> BACK_ACTION_VISIBLE",
            ] {
                let content = format!("Review Configuration\n\n{summary}\n\n{action}");

                let rendered = render_scrolled_fixture(&content, width, height);

                let expected = if action.starts_with('>') {
                    "CONTINUE_ACTION_VISIBLE"
                } else {
                    "BACK_ACTION_VISIBLE"
                };

                assert!(
                    rendered.contains(expected),
                    "{expected} invisible at {width}x{height}"
                );
            }
        }
    }

    #[test]
    fn focused_scroll_reveals_last_configuration_item() {
        let content = (0..30)
            .map(|index| {
                let marker = if index == 29 { ">" } else { " " };
                format!("{marker} Configuration item {index:02}")
            })
            .collect::<Vec<_>>()
            .join("\n");

        let offset = focused_configuration_scroll(&content, 52, 17);
        assert!(offset > 0);

        let mut terminal = Terminal::new(TestBackend::new(80, 25)).expect("test terminal");

        terminal
            .draw(|frame| {
                let paragraph = Paragraph::new(content.as_str())
                    .wrap(Wrap { trim: false })
                    .scroll((offset, 0))
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title("Configuration"),
                    );

                frame.render_widget(paragraph, Rect::new(26, 3, 54, 19));
            })
            .expect("render scrolled configuration");

        let rendered = rendered_text(&terminal, 80, 25);
        assert!(rendered.contains("Configuration item 29"));
    }

    #[test]
    fn focused_scroll_accounts_for_wrapped_paths() {
        let path = "/media/daia/models/collection/very-long-directory-name/model.gguf";

        let content = format!("Bundled Models\n\n{}\n> Selected model", path);

        let offset = focused_configuration_scroll(&content, 20, 3);
        assert!(offset > 0);
    }

    #[test]
    fn focused_scroll_uses_detail_marker_after_navigation() {
        let content = "> External Content\n  Storage\n\nDetails\n> Continue";

        let offset = focused_configuration_scroll(content, 20, 3);
        assert!(offset > 0);
    }

    #[test]
    fn focused_scroll_handles_empty_viewports_and_no_selection() {
        assert_eq!(focused_configuration_scroll("> Continue", 0, 10), 0);
        assert_eq!(focused_configuration_scroll("> Continue", 20, 0), 0);
        assert_eq!(focused_configuration_scroll("No focused item", 20, 5), 0);
    }

    #[test]
    fn scroll_helper_keeps_visible_selection_stable() {
        assert_eq!(scroll_offset_for_selection(4, 6, 2, 30, 10), 4);
    }

    #[test]
    fn scroll_helper_moves_down_for_wrapped_selection() {
        assert_eq!(scroll_offset_for_selection(0, 12, 4, 30, 10), 6);
    }

    #[test]
    fn scroll_helper_moves_up_to_selected_row() {
        assert_eq!(scroll_offset_for_selection(15, 5, 2, 30, 10), 5);
    }

    #[test]
    fn scroll_helper_clamps_to_last_valid_viewport() {
        assert_eq!(scroll_offset_for_selection(0, 28, 2, 30, 10), 20);
    }

    #[test]
    fn scroll_helper_handles_selection_taller_than_viewport() {
        assert_eq!(scroll_offset_for_selection(0, 12, 15, 40, 10), 12);
    }

    #[test]
    fn scroll_helper_handles_empty_and_zero_height_viewports() {
        assert_eq!(scroll_offset_for_selection(5, 10, 2, 0, 10), 0);

        assert_eq!(scroll_offset_for_selection(5, 10, 2, 30, 0), 0);

        assert_eq!(scroll_offset_for_selection(5, 10, 0, 30, 10), 5);
    }

    #[test]
    fn scroll_helper_clamps_existing_offset() {
        assert_eq!(scroll_offset_for_selection(100, 25, 1, 30, 10), 20);
    }

    #[test]
    fn long_configuration_text_is_currently_clipped_vertically() {
        let mut terminal = Terminal::new(TestBackend::new(80, 25)).expect("test terminal");

        let background = Color::Rgb(11, 32, 57);
        let secondary = Color::Rgb(185, 208, 227);

        let long_text = (0..30)
            .map(|index| format!("Configuration item {index:02}"))
            .collect::<Vec<_>>()
            .join("\n");

        terminal
            .draw(|frame| {
                let area = Rect::new(26, 3, 54, 19);

                let styled = style_wizard_detail(&long_text, 0, background, secondary);

                let paragraph = Paragraph::new(styled).block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Configuration"),
                );

                frame.render_widget(paragraph, area);
            })
            .expect("render overflowing configuration");

        let rendered = rendered_text(&terminal, 80, 25);

        assert!(rendered.contains("Configuration item 00"));
        assert!(rendered.contains("Configuration item 16"));

        assert!(
            !rendered.contains("Configuration item 29"),
            "Current renderer should demonstrate vertical clipping"
        );
    }

    #[test]
    fn long_configuration_path_is_currently_clipped_horizontally() {
        let mut terminal = Terminal::new(TestBackend::new(80, 25)).expect("test terminal");

        let long_path = "/media/daia/models/collection/very-long-directory-name/model.gguf";

        terminal
            .draw(|frame| {
                let area = Rect::new(26, 3, 54, 19);

                let paragraph = Paragraph::new(long_path).block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Configuration"),
                );

                frame.render_widget(paragraph, area);
            })
            .expect("render long model path");

        let rendered = rendered_text(&terminal, 80, 25);

        assert!(rendered.contains("/media/daia/models"));

        assert!(
            !rendered.contains("model.gguf"),
            "Current renderer should demonstrate horizontal clipping"
        );
    }

    #[test]
    fn wrapped_configuration_path_preserves_filename() {
        let mut terminal = Terminal::new(TestBackend::new(80, 25)).expect("test terminal");

        let path = "/media/daia/models/collection/very-long-directory-name/model.gguf";

        terminal
            .draw(|frame| {
                let area = Rect::new(26, 3, 54, 19);

                let paragraph = Paragraph::new(path).wrap(Wrap { trim: false }).block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Configuration"),
                );

                frame.render_widget(paragraph, area);
            })
            .expect("render wrapped model path");

        let rendered = rendered_text(&terminal, 80, 25);

        assert!(rendered.contains("/media/daia/models"));
        assert!(rendered.contains("model.gguf"));
    }

    #[test]
    fn responsive_renderer_uses_single_column_at_60_by_20() {
        let mut terminal = Terminal::new(TestBackend::new(60, 20)).expect("test terminal");
        let mut state = TuiState::new();
        state.screen = WizardScreen::Localization;

        terminal
            .draw(|frame| render(frame, &state))
            .expect("render narrow terminal");

        let rendered = rendered_text(&terminal, 60, 20);

        assert!(rendered.contains("Localization"));
        assert!(!rendered.contains("Navigation"));
    }

    #[test]
    fn responsive_renderer_uses_sidebar_at_80_by_25() {
        let mut terminal = Terminal::new(TestBackend::new(80, 25)).expect("test terminal");
        let mut state = TuiState::new();
        state.screen = WizardScreen::Localization;

        terminal
            .draw(|frame| render(frame, &state))
            .expect("render 80-column terminal");

        let rendered = rendered_text(&terminal, 80, 25);

        assert!(rendered.contains("Navigation"));
        assert!(rendered.contains("Configuration"));
        assert!(rendered.contains("Language"));
    }

    #[test]
    fn responsive_renderer_uses_sidebar_at_100_by_30() {
        let mut terminal = Terminal::new(TestBackend::new(100, 30)).expect("test terminal");
        let mut state = TuiState::new();
        state.screen = WizardScreen::Localization;

        terminal
            .draw(|frame| render(frame, &state))
            .expect("render 100-column terminal");

        let rendered = rendered_text(&terminal, 100, 30);

        assert!(rendered.contains("Navigation"));
        assert!(rendered.contains("Configuration"));
        assert!(rendered.contains("Language"));
    }

    #[test]
    fn responsive_renderer_uses_sidebar_at_120_by_35() {
        let mut terminal = Terminal::new(TestBackend::new(120, 35)).expect("test terminal");
        let mut state = TuiState::new();
        state.screen = WizardScreen::Localization;

        terminal
            .draw(|frame| render(frame, &state))
            .expect("render 120-column terminal");

        let rendered = rendered_text(&terminal, 120, 35);

        assert!(rendered.contains("Navigation"));
        assert!(rendered.contains("Configuration"));
        assert!(rendered.contains("Language"));
    }

    #[test]
    fn responsive_layout_uses_single_column_on_narrow_terminal() {
        let area = Rect::new(0, 0, 60, 14);

        assert_eq!(
            calculate_wizard_layout(area),
            WizardLayout::SingleColumn { content: area }
        );
    }

    #[test]
    fn responsive_layout_uses_single_column_on_short_terminal() {
        let area = Rect::new(0, 0, 120, 18);

        assert_eq!(
            calculate_wizard_layout(area),
            WizardLayout::SingleColumn { content: area }
        );
    }

    #[test]
    fn responsive_layout_falls_back_below_minimum_body_height() {
        let area = Rect::new(0, 3, 80, 18);

        assert_eq!(
            calculate_wizard_layout(area),
            WizardLayout::SingleColumn { content: area }
        );
    }

    #[test]
    fn responsive_layout_uses_two_columns_at_80_by_25() {
        let area = Rect::new(0, 3, 80, 19);

        match calculate_wizard_layout(area) {
            WizardLayout::TwoColumn { sidebar, content } => {
                assert_eq!(sidebar, Rect::new(0, 3, 26, 19));
                assert_eq!(content, Rect::new(26, 3, 54, 19));
            }
            other => panic!("expected two columns, got {other:?}"),
        }
    }

    #[test]
    fn responsive_layout_preserves_offset_and_width_at_100_by_30() {
        let area = Rect::new(3, 4, 100, 30);

        match calculate_wizard_layout(area) {
            WizardLayout::TwoColumn { sidebar, content } => {
                assert_eq!(sidebar, Rect::new(3, 4, 26, 30));
                assert_eq!(content, Rect::new(29, 4, 74, 30));
            }
            other => panic!("expected two columns, got {other:?}"),
        }
    }

    #[test]
    fn responsive_layout_uses_two_columns_at_120_by_35() {
        let area = Rect::new(0, 0, 120, 35);

        match calculate_wizard_layout(area) {
            WizardLayout::TwoColumn { sidebar, content } => {
                assert_eq!(sidebar, Rect::new(0, 0, 26, 35));
                assert_eq!(content, Rect::new(26, 0, 94, 35));
            }
            other => panic!("expected two columns, got {other:?}"),
        }
    }

    #[test]
    fn wizard_navigation_uses_accent_without_selection_background() {
        let background = Color::Rgb(11, 32, 57);
        let secondary = Color::Rgb(185, 208, 227);

        let text = style_wizard_detail(
            "  Welcome\n> Localization\n  Hardware Check\n\nLocalization\n\n> Language: English",
            3,
            background,
            secondary,
        );

        assert_eq!(text.lines.len(), 7);

        assert_eq!(text.lines[1].spans[0].content, "> Localization");
        assert_eq!(text.lines[1].style.fg, Some(Color::Rgb(139, 203, 255)));
        assert_eq!(text.lines[1].style.bg, Some(background));
        assert!(text.lines[1].style.add_modifier.contains(Modifier::BOLD));

        assert_eq!(text.lines[6].spans[0].content, "> Language: English");
        assert_eq!(text.lines[6].style.fg, Some(Color::White));
        assert_eq!(text.lines[6].style.bg, Some(Color::Rgb(24, 63, 104)));
        assert!(text.lines[6].style.add_modifier.contains(Modifier::BOLD));

        assert_eq!(text.lines[0].style.bg, Some(background));
        assert_eq!(text.lines[2].style.fg, Some(secondary));
    }

    #[test]
    fn wizard_detail_highlights_only_focused_rows() {
        let text = style_wizard_detail(
            "Localization\n\n> Language: en\n  Country: IN\n\n  > Back",
            1,
            Color::Rgb(11, 32, 57),
            Color::Rgb(185, 208, 227),
        );

        let selected = Color::Rgb(24, 63, 104);
        let background = Color::Rgb(11, 32, 57);

        assert_eq!(text.lines.len(), 6);

        assert_eq!(text.lines[0].spans[0].content, "Localization");
        assert_eq!(text.lines[0].style.bg, Some(background));

        assert_eq!(text.lines[2].spans[0].content, "> Language: en");
        assert_eq!(text.lines[2].style.bg, Some(selected));

        assert_eq!(text.lines[3].spans[0].content, "  Country: IN");
        assert_eq!(text.lines[3].style.bg, Some(background));

        assert_eq!(text.lines[5].spans[0].content, "  > Back");
        assert_eq!(text.lines[5].style.bg, Some(selected));
    }

    #[test]
    fn welcome_selection_highlight_follows_focus() {
        let backend = TestBackend::new(100, 35);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        let mut state = TuiState::new();

        let selected_color = Color::Rgb(24, 63, 104);
        let background_color = Color::Rgb(11, 32, 57);

        terminal
            .draw(|frame| render(frame, &state))
            .expect("render Start selection");

        let start_buffer = terminal.backend().buffer();

        assert_eq!(start_buffer[(4, 8)].bg, selected_color);
        assert_eq!(start_buffer[(4, 9)].bg, background_color);

        state.welcome_action = WelcomeAction::Quit;

        terminal
            .draw(|frame| render(frame, &state))
            .expect("render Quit selection");

        let quit_buffer = terminal.backend().buffer();

        assert_eq!(quit_buffer[(4, 8)].bg, background_color);
        assert_eq!(quit_buffer[(4, 9)].bg, selected_color);
    }

    #[test]
    fn midnight_blue_welcome_rendering_regression() {
        let backend = TestBackend::new(100, 35);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        let state = TuiState::new();

        terminal
            .draw(|frame| render(frame, &state))
            .expect("render Welcome screen");

        let buffer = terminal.backend().buffer();

        assert_eq!(
            buffer[(50, 20)].bg,
            Color::Rgb(11, 32, 57),
            "Welcome background should be Midnight Blue",
        );

        let header = (1..25).map(|x| &buffer[(x, 1)]).collect::<Vec<_>>();

        assert!(
            header.iter().any(|cell| {
                cell.fg == Color::Rgb(139, 203, 255) && !cell.symbol().trim().is_empty()
            }),
            "Welcome header should contain cyan-accented text",
        );

        let rendered = (0..35)
            .map(|y| {
                (0..100)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");

        assert!(rendered.contains("Welcome to Debian AI Appliance"));
        assert!(rendered.contains("Start Installation"));
        assert!(rendered.contains("Quit"));
    }
}

#[cfg(test)]
mod tests {
    use super::request_reboot_with;

    use super::{
        AdministratorField, ApplianceIdentityField, ExternalContentFocus, HardwareSummary,
        InstallationAction, RootCredentialsField, ScreenAction, TuiState, WelcomeAction,
        WizardScreen, execute_appliance_installation, render,
    };
    use ratatui::{Terminal, backend::TestBackend};

    #[derive(Default)]
    struct RecordingInstallationExecutor {
        operations: Vec<engine::InstallationOperation>,
    }

    impl engine::InstallationOperationExecutor for RecordingInstallationExecutor {
        type Error = std::convert::Infallible;

        fn execute_operation(
            &mut self,
            operation: &engine::InstallationOperation,
        ) -> Result<(), Self::Error> {
            self.operations.push(operation.clone());
            Ok(())
        }
    }

    #[test]
    fn executes_prepared_installation_through_engine() {
        let mut state = TuiState::new();

        let storage = model::DiscoveredStorage::new(
            "install-disk",
            model::StorageKind::Secondary,
            "/dev/install",
        )
        .with_size_bytes(128 * 1024 * 1024 * 1024);
        let storage_id = storage.id().clone();

        state.wizard.set_discovered_storage(vec![storage]);
        state.wizard.select_storage(storage_id);

        let profile = state
            .appliance_profiles
            .first()
            .expect("at least one appliance profile should exist")
            .clone();
        state.wizard.set_profile_name(profile.name());

        let repositories =
            application::load_content_repositories().expect("content repositories should load");
        let repository = repositories
            .first()
            .expect("at least one content repository should exist")
            .clone();
        let repository_id = repository.id().clone();

        state.wizard.set_content_repositories(repositories);
        state.wizard.select_content_repository(repository_id);
        state.wizard.set_external_content_items(Vec::new());
        state.wizard.select_external_content(Vec::new());
        state.wizard.set_appliance_identity("daia", "home.arpa");
        state
            .wizard
            .set_user_identity("install-admin", "Install Administrator");

        let registry = super::load_provider_registry().expect("provider registry should load");
        let engine = engine::Engine::from_registry(registry);

        struct EmptyContentInspector;

        impl inspector::ContentInspector for EmptyContentInspector {
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

        struct FixedStorageInspector {
            storage: Vec<model::DiscoveredStorage>,
        }

        impl inspector::StorageInspector for FixedStorageInspector {
            fn inspect(
                &self,
            ) -> Result<Vec<model::DiscoveredStorage>, inspector::StorageInspectError> {
                Ok(self.storage.clone())
            }
        }

        let content_inspector = EmptyContentInspector;
        let storage_inspector = FixedStorageInspector {
            storage: state.wizard.selectable_storage().cloned().collect(),
        };

        let prepared = super::prepare_appliance_installation(
            &engine,
            &state.wizard,
            &state.appliance_profiles,
            &content_inspector,
            &storage_inspector,
        )
        .expect("installation should prepare");

        let expected_operations = prepared.installation_plan().operations().to_vec();

        let mut executor = RecordingInstallationExecutor::default();

        execute_appliance_installation(&engine, &prepared, &mut executor)
            .expect("prepared installation should execute");

        assert_eq!(executor.operations, expected_operations);
        assert!(!executor.operations.is_empty());
    }

    #[test]
    fn installation_lifecycle_screens_do_not_navigate_backward() {
        for screen in [
            WizardScreen::Installing,
            WizardScreen::InstallationComplete,
            WizardScreen::InstallationFailed,
        ] {
            let mut state = TuiState::new();
            state.screen = screen;

            state.previous_screen();

            assert_eq!(state.screen, screen);
        }
    }

    #[test]
    fn installation_request_is_consumed_once() {
        let mut state = TuiState::new();

        state.screen = WizardScreen::ConfirmInstallation;
        state.installation_action = InstallationAction::Install;
        state.confirm_installation_action();

        assert!(state.take_installation_request());
        assert!(!state.take_installation_request());
    }

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
    fn selecting_install_requests_execution_without_executing() {
        let mut state = TuiState::new();

        state.screen = WizardScreen::ConfirmInstallation;
        state.installation_action = InstallationAction::Install;

        state.confirm_installation_action();

        assert_eq!(state.screen, WizardScreen::ConfirmInstallation);
        assert_eq!(state.installation_action, InstallationAction::Back);
        assert!(state.take_installation_request());
        assert!(!state.take_installation_request());
    }

    #[test]
    fn confirming_installation_back_returns_to_review() {
        let mut state = TuiState::new();

        state.screen = WizardScreen::ConfirmInstallation;
        state.installation_action = InstallationAction::Back;

        state.confirm_installation_action();

        assert_eq!(state.screen, WizardScreen::Review);
        assert_eq!(state.installation_action, InstallationAction::Back);
        assert!(!state.take_installation_request());
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
        state.wizard.set_appliance_identity("daia", "home.arpa");
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
        assert!(rendered.contains("Hostname: daia"));
        assert!(rendered.contains("Domain: home.arpa"));
        assert!(rendered.contains("FQDN: daia.home.arpa"));
        assert!(rendered.contains("review-admin"));
        assert!(rendered.contains("Review Administrator"));
        assert!(rendered.contains("Root and administrator passwords are configured"));
    }

    #[test]
    fn localization_screen_renders_separate_lines() {
        let mut state = TuiState::new();
        state.screen = WizardScreen::Localization;

        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).expect("test terminal should be created");

        terminal
            .draw(|frame| render(frame, &state))
            .expect("localization screen should render");

        let buffer = terminal.backend().buffer();

        let lines: Vec<String> = (0..buffer.area.height)
            .map(|y| {
                (0..buffer.area.width)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect();

        let rendered = lines.join("\n");

        for label in [
            "Language:",
            "Country / Region:",
            "Locale:",
            "Keyboard:",
            "Continue",
            "Back",
        ] {
            assert!(
                lines.iter().any(|line| line.contains(label)),
                "missing localization line: {label}"
            );
        }

        assert!(
            !rendered.contains(r"\n"),
            "localization screen contains literal newline escapes"
        );

        let language_line = lines
            .iter()
            .position(|line| line.contains("Language:"))
            .expect("language line");

        let country_line = lines
            .iter()
            .position(|line| line.contains("Country / Region:"))
            .expect("country line");

        let locale_line = lines
            .iter()
            .position(|line| line.contains("Locale:"))
            .expect("locale line");

        let keyboard_line = lines
            .iter()
            .position(|line| line.contains("Keyboard:"))
            .expect("keyboard line");

        assert!(language_line < country_line);
        assert!(country_line < locale_line);
        assert!(locale_line < keyboard_line);
    }

    #[test]
    fn localization_can_select_indian_keyboard() {
        let mut state = TuiState::new();
        state.screen = WizardScreen::Localization;
        state.localization_field = 3;

        let indian_index = application::KEYBOARD_LAYOUTS
            .iter()
            .position(|layout| layout.id == "in")
            .expect("Indian keyboard must be available");

        state.localization_indices[3] = indian_index;
        state.localization_field = 4;
        state.confirm_localization();

        let selected = state.wizard.localization().expect("localization saved");
        assert_eq!(selected.country(), "IN");
        assert_eq!(selected.locale(), "en_IN.UTF-8");
        assert_eq!(selected.keyboard_layout(), "in");
        assert_eq!(state.screen, WizardScreen::HardwareCheck);
    }

    #[test]
    fn localization_supports_us_keyboard_for_india() {
        let mut state = TuiState::new();
        state.screen = WizardScreen::Localization;
        state.localization_field = 4;
        state.confirm_localization();

        let selected = state.wizard.localization().expect("localization saved");
        assert_eq!(selected.country(), "IN");
        assert_eq!(selected.keyboard_layout(), "us");
    }

    #[test]
    fn localization_back_preserves_pending_selections() {
        let mut state = TuiState::new();
        state.screen = WizardScreen::Localization;
        state.localization_field = 3;
        state.cycle_localization_option(true);
        let selected_index = state.localization_indices[3];

        state.previous_screen();
        assert_eq!(state.screen, WizardScreen::Welcome);

        state.next_screen();
        assert_eq!(state.screen, WizardScreen::Localization);
        assert_eq!(state.localization_indices[3], selected_index);
    }

    #[test]
    fn localization_field_navigation_is_bounded() {
        let mut state = TuiState::new();

        state.previous_localization_field();
        assert_eq!(state.localization_field, 0);

        for _ in 0..10 {
            state.next_localization_field();
        }
        assert_eq!(state.localization_field, 5);

        state.localization_field = 3;
        let original = state.localization_indices[3];
        state.cycle_localization_option(true);
        assert_ne!(state.localization_indices[3], original);
        state.cycle_localization_option(false);
        assert_eq!(state.localization_indices[3], original);
    }

    #[test]
    fn reboot_request_accepts_successful_command() {
        let result = request_reboot_with(|command| {
            assert_eq!(command.get_program(), "systemctl");
            assert_eq!(
                command.get_args().collect::<Vec<_>>(),
                vec![std::ffi::OsStr::new("reboot")]
            );
            std::process::Command::new("true").status()
        });

        assert!(result.is_ok());
    }

    #[test]
    fn reboot_request_reports_failed_command() {
        let result = request_reboot_with(|_| std::process::Command::new("false").status());

        assert!(result.is_err());
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
        state.pending_external_content = vec![item_id.clone()];
        state.pending_model_names = vec![(item_id.clone(), "test-model".to_owned())];
        state
            .wizard
            .set_model_realization_intents(vec![model::ModelRealizationIntent::new(
                model::ModelRealizationId::new("test-model"),
                model::InferenceEngineId::ollama(),
                item_id,
            )]);
        state.external_content_error = Some("stale error".to_owned());
        state.screen = WizardScreen::ExternalContent;
        state.external_content_focus = ExternalContentFocus::ContinueWithoutLocalContent;

        state.confirm_external_content();

        assert!(state.pending_external_content.is_empty());
        assert!(state.pending_model_names.is_empty());
        assert!(state.wizard.selected_external_content().is_empty());
        assert!(state.wizard.model_realization_intents().is_empty());
        assert_eq!(state.external_content_error, None);
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
    fn automatically_names_selected_models_without_prompt() {
        let mut state = TuiState::new();

        // Use real, structurally valid GGUF fixtures so production model
        // inspection remains enabled during this regression test.
        let directory = std::env::temp_dir().join(format!(
            "daia-tui-model-naming-{}-{:?}",
            std::process::id(),
            std::thread::current().id(),
        ));
        std::fs::create_dir(&directory).expect("fixture directory should exist");

        struct FixtureDirectory(std::path::PathBuf);

        impl Drop for FixtureDirectory {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }

        let _fixture_directory = FixtureDirectory(directory.clone());

        let mut gguf = Vec::new();
        gguf.extend_from_slice(b"GGUF");
        gguf.extend_from_slice(&3_u32.to_le_bytes());
        gguf.extend_from_slice(&0_u64.to_le_bytes());
        gguf.extend_from_slice(&1_u64.to_le_bytes());

        let key = b"general.architecture";
        gguf.extend_from_slice(&(key.len() as u64).to_le_bytes());
        gguf.extend_from_slice(key);
        gguf.extend_from_slice(&8_u32.to_le_bytes());

        let architecture = b"llama";
        gguf.extend_from_slice(&(architecture.len() as u64).to_le_bytes());
        gguf.extend_from_slice(architecture);

        let first_path = directory.join("first.gguf");
        let second_path = directory.join("second.gguf");

        std::fs::write(&first_path, &gguf).expect("first GGUF fixture should exist");
        std::fs::write(&second_path, &gguf).expect("second GGUF fixture should exist");

        let source_id = model::ContentSourceId::new("test-source");
        let first = model::ExternalContentItem::new(source_id.clone(), &first_path);
        let second = model::ExternalContentItem::new(source_id, &second_path);

        let first_id = first.id().clone();
        let second_id = second.id().clone();

        state.wizard.set_external_content_items(vec![first, second]);
        state.pending_external_content = vec![first_id.clone(), second_id.clone()];
        state.external_content_focus = ExternalContentFocus::SaveAndContinue;
        state.screen = WizardScreen::ExternalContent;

        state.confirm_external_content();

        assert!(
            state.external_content_error.is_none(),
            "model inspection failed: {:?}",
            state.external_content_error
        );
        assert_eq!(state.screen, WizardScreen::Storage);
        assert_eq!(
            state.wizard.selected_external_content(),
            &[first_id.clone(), second_id.clone()]
        );
        let intents = state.wizard.model_realization_intents();
        assert_eq!(intents.len(), 2);
        assert_eq!(
            intents
                .iter()
                .map(|intent| intent.id().as_str())
                .collect::<Vec<_>>(),
            vec!["first", "second"]
        );
        assert_eq!(intents[0].source_item_id(), &first_id);
        assert_eq!(intents[1].source_item_id(), &second_id);
        assert_eq!(intents[0].engine(), &model::InferenceEngineId::ollama());
        assert_eq!(intents[1].engine(), &model::InferenceEngineId::ollama());
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

        assert_eq!(state.screen, WizardScreen::Review);
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
        state.wizard.set_appliance_identity("daia", "home.arpa");
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
    fn hardware_check_requires_discovered_hardware_to_continue() {
        let mut state = TuiState::new();
        state.screen = WizardScreen::HardwareCheck;
        state.hardware_error = Some("hardware discovery failed".to_owned());

        state.confirm_screen_action();

        assert_eq!(state.screen, WizardScreen::HardwareCheck);
        assert_eq!(state.hardware, None);
    }

    #[test]
    fn hardware_check_with_discovered_hardware_advances() {
        let mut state = TuiState::new();
        state.screen = WizardScreen::HardwareCheck;
        state.hardware = Some(HardwareSummary {
            architecture: "x86_64".to_owned(),
            logical_processor_count: 4,
            total_memory_bytes: 8 * 1024 * 1024 * 1024,
            gpus: Vec::new(),
            accelerators: Vec::new(),
        });

        state.confirm_screen_action();

        assert_eq!(state.screen, WizardScreen::ApplianceIdentity);
    }

    #[test]
    fn hardware_check_back_returns_to_localization() {
        let mut state = TuiState::new();
        state.screen = WizardScreen::HardwareCheck;
        state.screen_action = ScreenAction::Back;

        state.confirm_screen_action();

        assert_eq!(state.screen, WizardScreen::Localization);
    }

    #[test]
    fn root_credentials_fields_advance_through_back() {
        let mut state = TuiState::new();
        state.screen = WizardScreen::RootCredentials;

        assert_eq!(
            state.root_credentials_field,
            RootCredentialsField::RootPassword
        );

        state.next_root_credentials_field();
        assert_eq!(
            state.root_credentials_field,
            RootCredentialsField::RootPasswordConfirmation
        );

        state.next_root_credentials_field();
        assert_eq!(state.root_credentials_field, RootCredentialsField::Back);

        state.next_root_credentials_field();
        assert_eq!(state.root_credentials_field, RootCredentialsField::Back);

        state.previous_root_credentials_field();
        assert_eq!(
            state.root_credentials_field,
            RootCredentialsField::RootPasswordConfirmation
        );
    }

    #[test]
    fn valid_root_credentials_advance_to_administrator() {
        let mut state = TuiState::new();
        state.screen = WizardScreen::RootCredentials;
        state.root_credentials_field = RootCredentialsField::RootPasswordConfirmation;
        state.credentials.root_password = "root-secret".to_owned();
        state.credentials.root_password_confirmation = "root-secret".to_owned();

        state.confirm_root_credentials_field();

        assert_eq!(state.screen, WizardScreen::Administrator);
        assert_eq!(state.root_credentials_error, None);
    }

    #[test]
    fn empty_root_password_does_not_advance() {
        let mut state = TuiState::new();
        state.screen = WizardScreen::RootCredentials;
        state.root_credentials_field = RootCredentialsField::RootPasswordConfirmation;

        state.confirm_root_credentials_field();

        assert_eq!(state.screen, WizardScreen::RootCredentials);
        assert_eq!(
            state.root_credentials_error.as_deref(),
            Some("Root password cannot be empty")
        );
        assert_eq!(
            state.root_credentials_field,
            RootCredentialsField::RootPassword
        );
    }

    #[test]
    fn mismatched_root_passwords_do_not_advance() {
        let mut state = TuiState::new();
        state.screen = WizardScreen::RootCredentials;
        state.root_credentials_field = RootCredentialsField::RootPasswordConfirmation;
        state.credentials.root_password = "first".to_owned();
        state.credentials.root_password_confirmation = "second".to_owned();

        state.confirm_root_credentials_field();

        assert_eq!(state.screen, WizardScreen::RootCredentials);
        assert_eq!(
            state.root_credentials_error.as_deref(),
            Some("Root passwords do not match")
        );
        assert_eq!(
            state.root_credentials_field,
            RootCredentialsField::RootPasswordConfirmation
        );
    }

    #[test]
    fn administrator_fields_advance_through_back() {
        let mut state = TuiState::new();
        state.screen = WizardScreen::Administrator;

        let fields = [
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
    fn administrator_back_returns_to_root_credentials() {
        let mut state = TuiState::new();
        state.screen = WizardScreen::Administrator;
        state.administrator_field = AdministratorField::Back;

        state.confirm_administrator_field();

        assert_eq!(state.screen, WizardScreen::RootCredentials);
        assert_eq!(state.administrator_field, AdministratorField::Username);
    }

    #[test]
    fn final_administrator_confirmation_advances_to_profile() {
        let mut state = TuiState::new();
        state.screen = WizardScreen::Administrator;
        state.administrator_field = AdministratorField::AdministratorPasswordConfirmation;
        state.administrator_username = "daia-admin".to_owned();
        state.administrator_display_name = "DAIA Administrator".to_owned();
        state.credentials.administrator_password = "admin-secret".to_owned();
        state.credentials.administrator_password_confirmation = "admin-secret".to_owned();

        state.confirm_administrator_field();

        assert_eq!(state.screen, WizardScreen::Profile);
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
            Some("Administrator username cannot be empty")
        );
        assert_eq!(state.administrator_field, AdministratorField::Username);
        assert_eq!(state.wizard.user_configuration(), None);
    }

    #[test]
    fn mismatched_administrator_passwords_do_not_advance() {
        let mut state = TuiState::new();
        state.screen = WizardScreen::Administrator;
        state.administrator_field = AdministratorField::AdministratorPasswordConfirmation;
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

        state.screen = WizardScreen::HardwareCheck;
        state.previous_screen();
        assert_eq!(state.screen, WizardScreen::Localization);

        state.previous_screen();
        assert_eq!(state.screen, WizardScreen::Welcome);

        state.screen = WizardScreen::ApplianceIdentity;
        state.previous_screen();
        assert_eq!(state.screen, WizardScreen::HardwareCheck);

        state.screen = WizardScreen::RootCredentials;
        state.previous_screen();
        assert_eq!(state.screen, WizardScreen::ApplianceIdentity);

        state.screen = WizardScreen::Administrator;
        state.previous_screen();
        assert_eq!(state.screen, WizardScreen::RootCredentials);

        state.screen = WizardScreen::Profile;
        state.previous_screen();
        assert_eq!(state.screen, WizardScreen::Administrator);

        state.screen = WizardScreen::ContentRepository;
        state.previous_screen();
        assert_eq!(state.screen, WizardScreen::Profile);

        state.screen = WizardScreen::ExternalContent;
        state.previous_screen();
        assert_eq!(state.screen, WizardScreen::ContentRepository);

        state.screen = WizardScreen::Storage;
        state.previous_screen();
        assert_eq!(state.screen, WizardScreen::ExternalContent);

        state.screen = WizardScreen::Review;
        state.previous_screen();
        assert_eq!(state.screen, WizardScreen::Storage);

        state.screen = WizardScreen::Welcome;
        state.previous_screen();
        assert_eq!(state.screen, WizardScreen::Welcome);
    }

    #[test]
    fn appliance_identity_requires_hostname_and_domain_name() {
        let mut state = TuiState::new();

        state.screen = WizardScreen::ApplianceIdentity;
        state.appliance_identity_field = ApplianceIdentityField::Continue;
        state.confirm_appliance_identity();

        assert_eq!(state.screen, WizardScreen::ApplianceIdentity);
        assert_eq!(
            state.appliance_identity_error.as_deref(),
            Some("Hostname cannot be empty.")
        );
        assert_eq!(
            state.appliance_identity_field,
            ApplianceIdentityField::Hostname
        );
        assert_eq!(state.wizard.appliance_identity(), None);

        state.appliance_hostname = "daia".to_owned();
        state.appliance_identity_field = ApplianceIdentityField::Continue;
        state.confirm_appliance_identity();

        assert_eq!(state.screen, WizardScreen::ApplianceIdentity);
        assert_eq!(
            state.appliance_identity_error.as_deref(),
            Some("Domain name cannot be empty.")
        );
        assert_eq!(
            state.appliance_identity_field,
            ApplianceIdentityField::DomainName
        );
        assert_eq!(state.wizard.appliance_identity(), None);
    }

    #[test]
    fn appliance_identity_rejects_invalid_dns_labels_before_installation() {
        for (hostname, domain, expected_field) in [
            ("bad host", "home.arpa", ApplianceIdentityField::Hostname),
            ("-daia", "home.arpa", ApplianceIdentityField::Hostname),
            ("daia-", "home.arpa", ApplianceIdentityField::Hostname),
            ("daia_name", "home.arpa", ApplianceIdentityField::Hostname),
            ("daia", "home..arpa", ApplianceIdentityField::DomainName),
            ("daia", "-home.arpa", ApplianceIdentityField::DomainName),
            ("daia", "home_.arpa", ApplianceIdentityField::DomainName),
            ("daia", "home.arpa.", ApplianceIdentityField::DomainName),
        ] {
            let mut state = TuiState::new();
            state.screen = WizardScreen::ApplianceIdentity;
            state.appliance_hostname = hostname.to_owned();
            state.appliance_domain_name = domain.to_owned();
            state.appliance_identity_field = ApplianceIdentityField::Continue;

            state.confirm_appliance_identity();

            assert_eq!(state.screen, WizardScreen::ApplianceIdentity);
            assert_eq!(state.appliance_identity_field, expected_field);
            assert!(state.appliance_identity_error.is_some());
            assert_eq!(state.wizard.appliance_identity(), None);
        }

        for (hostname, domain, expected_field) in [
            (
                "a".repeat(64),
                "home.arpa".to_owned(),
                ApplianceIdentityField::Hostname,
            ),
            (
                "daia".to_owned(),
                format!("{}.arpa", "a".repeat(64)),
                ApplianceIdentityField::DomainName,
            ),
            (
                "daia".to_owned(),
                "a".repeat(254),
                ApplianceIdentityField::DomainName,
            ),
        ] {
            let mut state = TuiState::new();
            state.screen = WizardScreen::ApplianceIdentity;
            state.appliance_hostname = hostname;
            state.appliance_domain_name = domain;
            state.appliance_identity_field = ApplianceIdentityField::Continue;

            state.confirm_appliance_identity();

            assert_eq!(state.screen, WizardScreen::ApplianceIdentity);
            assert_eq!(state.appliance_identity_field, expected_field);
            assert!(state.appliance_identity_error.is_some());
            assert_eq!(state.wizard.appliance_identity(), None);
        }
    }

    #[test]
    fn appliance_identity_is_committed_before_entering_root_credentials() {
        let mut state = TuiState::new();

        state.screen = WizardScreen::ApplianceIdentity;
        state.appliance_hostname = "  daia  ".to_owned();
        state.appliance_domain_name = "  home.arpa  ".to_owned();
        state.appliance_identity_field = ApplianceIdentityField::Continue;
        state.confirm_appliance_identity();

        let identity = state
            .wizard
            .appliance_identity()
            .expect("appliance identity should be committed");

        assert_eq!(identity.hostname(), "daia");
        assert_eq!(identity.domain_name(), "home.arpa");
        assert_eq!(identity.fqdn(), "daia.home.arpa");
        assert_eq!(state.screen, WizardScreen::RootCredentials);
        assert_eq!(state.appliance_identity_error, None);
    }

    #[test]
    fn navigates_forward_and_backward_through_wizard_screens() {
        let mut state = TuiState::new();

        let screens = [
            WizardScreen::Localization,
            WizardScreen::HardwareCheck,
            WizardScreen::ApplianceIdentity,
            WizardScreen::RootCredentials,
            WizardScreen::Administrator,
            WizardScreen::Profile,
            WizardScreen::ContentRepository,
            WizardScreen::ExternalContent,
            WizardScreen::Storage,
            WizardScreen::Review,
        ];

        for screen in screens {
            state.next_screen();
            assert_eq!(state.screen, screen);
        }

        state.next_screen();
        assert_eq!(state.screen, WizardScreen::Review);

        for screen in screens[..9].iter().rev() {
            state.previous_screen();
            assert_eq!(state.screen, *screen);
        }

        state.previous_screen();
        assert_eq!(state.screen, WizardScreen::Welcome);

        state.previous_screen();
        assert_eq!(state.screen, WizardScreen::Welcome);
    }
}
