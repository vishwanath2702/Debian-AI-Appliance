//! DAIA terminal user interface.

use application::{WizardState, load_appliance_profiles};
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
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
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum ScreenAction {
    #[default]
    Continue,
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
#[derive(Debug)]
struct TuiState {
    wizard: WizardState,
    appliance_profiles: Vec<model::ApplianceProfile>,
    selected_profile_index: usize,
    credentials: CredentialState,
    administrator_username: String,
    administrator_display_name: String,
    administrator_field: AdministratorField,
    administrator_error: Option<String>,
    welcome_action: WelcomeAction,
    screen_action: ScreenAction,
    screen: WizardScreen,
}

impl TuiState {
    fn new() -> Self {
        let appliance_profiles = load_appliance_profiles()
            .map(|repository| repository.profiles().to_vec())
            .unwrap_or_default();

        Self {
            wizard: WizardState::new(),
            appliance_profiles,
            selected_profile_index: 0,
            credentials: CredentialState::default(),
            administrator_username: String::new(),
            administrator_display_name: String::new(),
            administrator_field: AdministratorField::default(),
            administrator_error: None,
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

    fn next_screen_action(&mut self) {
        self.screen_action = ScreenAction::Back;
    }

    fn previous_screen_action(&mut self) {
        self.screen_action = ScreenAction::Continue;
    }

    fn confirm_screen_action(&mut self) {
        match self.screen_action {
            ScreenAction::Continue => self.next_screen(),
            ScreenAction::Back => self.previous_screen(),
        }

        self.screen_action = ScreenAction::Continue;
    }

    fn next_welcome_action(&mut self) {
        self.welcome_action = WelcomeAction::Quit;
    }

    fn previous_welcome_action(&mut self) {
        self.welcome_action = WelcomeAction::Start;
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

    fn previous_screen(&mut self) {
        self.screen = match self.screen {
            WizardScreen::Welcome => WizardScreen::Welcome,
            WizardScreen::Profile => WizardScreen::Welcome,
            WizardScreen::ContentRepository => WizardScreen::Profile,
            WizardScreen::ExternalContent => WizardScreen::ContentRepository,
            WizardScreen::Storage => WizardScreen::ExternalContent,
            WizardScreen::Administrator => WizardScreen::Storage,
            WizardScreen::Review => WizardScreen::Administrator,
        };
    }
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
        _ => {
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

            format!(
                "Selected profile: {profile}\n\n                 {continue_marker} Continue\n                 {back_marker} Back"
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
        WizardScreen::Administrator => {
            "↑ / ↓: Navigate    Enter: Select    Esc: Back    Backspace: Edit"
        }
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
            KeyCode::Down
                if matches!(
                    state.screen,
                    WizardScreen::ContentRepository
                        | WizardScreen::ExternalContent
                        | WizardScreen::Storage
                        | WizardScreen::Review
                ) =>
            {
                state.next_screen_action();
            }
            KeyCode::Up
                if matches!(
                    state.screen,
                    WizardScreen::ContentRepository
                        | WizardScreen::ExternalContent
                        | WizardScreen::Storage
                        | WizardScreen::Review
                ) =>
            {
                state.previous_screen_action();
            }
            KeyCode::Enter
                if matches!(
                    state.screen,
                    WizardScreen::ContentRepository
                        | WizardScreen::ExternalContent
                        | WizardScreen::Storage
                        | WizardScreen::Review
                ) =>
            {
                state.confirm_screen_action();
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
    use super::{AdministratorField, ScreenAction, TuiState, WelcomeAction, WizardScreen};

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
