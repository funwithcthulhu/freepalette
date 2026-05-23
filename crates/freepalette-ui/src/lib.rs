mod autostart;
mod hotkey;
mod icon;
mod tray;

use freepalette_core::{Action, Config, RankedResult};
use freepalette_daemon::{ActionExecutionPolicy, DaemonError, DaemonState, HotkeyState};
use thiserror::Error;

pub use autostart::{UiAutostart, UiAutostartError, UiAutostartStatus};
pub use hotkey::{UiHotkeyBridge, UiHotkeyError};
pub use icon::{app_icon_rgba, APP_ICON_SIZE};
pub use tray::{TrayCommand, UiTray, UiTrayError};

#[derive(Debug, Error)]
pub enum UiError {
    #[error(transparent)]
    Daemon(#[from] DaemonError),
    #[error(transparent)]
    Hotkey(#[from] UiHotkeyError),
}

pub struct PaletteState {
    query: String,
    results: Vec<RankedResult>,
    selected: Option<usize>,
    status: PaletteStatus,
    daemon: DaemonState,
}

impl PaletteState {
    pub fn from_default_config() -> Result<Self, UiError> {
        Ok(Self::from_daemon(DaemonState::from_default_config()?))
    }

    pub fn from_config(config: Config) -> Result<Self, UiError> {
        Ok(Self::from_daemon(DaemonState::from_config(config)?))
    }

    pub fn from_daemon(daemon: DaemonState) -> Self {
        Self {
            query: String::new(),
            results: Vec::new(),
            selected: None,
            status: PaletteStatus::Ready,
            daemon,
        }
    }

    pub fn query(&self) -> &str {
        &self.query
    }

    pub fn results(&self) -> &[RankedResult] {
        &self.results
    }

    pub fn selected_index(&self) -> Option<usize> {
        self.selected
    }

    pub fn status(&self) -> &PaletteStatus {
        &self.status
    }

    pub fn set_status_info(&mut self, message: impl Into<String>) {
        self.status = PaletteStatus::Info(message.into());
    }

    pub fn set_status_error(&mut self, message: impl Into<String>) {
        self.status = PaletteStatus::Error(message.into());
    }

    pub fn hotkey_state(&self) -> &HotkeyState {
        self.daemon.hotkey_state()
    }

    pub fn set_query(&mut self, query: impl Into<String>) {
        self.query = query.into();
        self.refresh_results();
    }

    pub fn reset_for_next_activation(&mut self) {
        self.query.clear();
        self.results.clear();
        self.selected = None;
        self.status = PaletteStatus::Ready;
    }

    pub fn move_selection(&mut self, direction: SelectionDirection) {
        let Some(current) = self.selected else {
            return;
        };

        let next = match direction {
            SelectionDirection::Previous => current.saturating_sub(1),
            SelectionDirection::Next => (current + 1).min(self.results.len().saturating_sub(1)),
        };

        self.selected = Some(next);
    }

    pub fn execute_selected(&mut self) -> PaletteExecution {
        let Some(index) = self.selected else {
            self.status = PaletteStatus::Info("No result selected".to_string());
            return PaletteExecution::NoSelection;
        };

        let Some(ranked) = self.results.get(index) else {
            self.status = PaletteStatus::Error("Selected result is unavailable".to_string());
            self.selected = None;
            return PaletteExecution::SelectedUnavailable;
        };

        if matches!(&ranked.result.action, Action::RunShell { .. }) {
            self.status = PaletteStatus::Error(
                "Shell commands cannot run from the UI yet; use the CLI with --allow-shell"
                    .to_string(),
            );
            return PaletteExecution::Blocked;
        }

        let hide_palette = action_hides_palette_after_success(&ranked.result.action);
        match self
            .daemon
            .execute_result(&ranked.result, ActionExecutionPolicy::BlockShellCommands)
        {
            Ok(outcome) => {
                self.status = PaletteStatus::Info(outcome.message);
                PaletteExecution::Completed { hide_palette }
            }
            Err(error) => {
                self.status = PaletteStatus::Error(error.to_string());
                PaletteExecution::Failed
            }
        }
    }

    pub fn reload_config(&mut self) {
        match self.daemon.reload_config() {
            Ok(()) => {
                let query = self.query.clone();
                self.set_query(query);
                self.status = PaletteStatus::Info("Config reloaded".to_string());
            }
            Err(error) => {
                self.status = PaletteStatus::Error(error.to_string());
            }
        }
    }

    fn refresh_results(&mut self) {
        if self.query.trim().is_empty() {
            self.results.clear();
            self.selected = None;
            self.status = PaletteStatus::Ready;
            return;
        }

        match self.daemon.search(&self.query, None) {
            Ok(results) => {
                self.selected = if results.is_empty() { None } else { Some(0) };
                self.results = results;
                self.status = PaletteStatus::Ready;
            }
            Err(error) => {
                self.results.clear();
                self.selected = None;
                self.status = PaletteStatus::Error(error.to_string());
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaletteExecution {
    NoSelection,
    SelectedUnavailable,
    Blocked,
    Completed { hide_palette: bool },
    Failed,
}

impl PaletteExecution {
    pub fn should_hide_palette(self) -> bool {
        matches!(self, Self::Completed { hide_palette: true })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionDirection {
    Previous,
    Next,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaletteStatus {
    Ready,
    Info(String),
    Error(String),
}

impl PaletteStatus {
    pub fn message(&self) -> Option<&str> {
        match self {
            Self::Ready => None,
            Self::Info(message) | Self::Error(message) => Some(message),
        }
    }

    pub fn is_error(&self) -> bool {
        matches!(self, Self::Error(_))
    }
}

fn action_hides_palette_after_success(action: &Action) -> bool {
    matches!(action, Action::LaunchApp { .. } | Action::OpenPath { .. })
}

#[cfg(test)]
mod tests {
    use freepalette_core::{AppEntry, GeneralConfig, ProviderConfig};

    use super::*;

    fn app_only_state() -> PaletteState {
        let mut first = AppEntry::new("First App", "first.exe");
        first.keywords = vec!["freepalette-selection-test".to_string()];
        let mut second = AppEntry::new("Second App", "second.exe");
        second.keywords = vec!["freepalette-selection-test".to_string()];

        PaletteState::from_config(Config {
            general: GeneralConfig { max_results: 10 },
            providers: ProviderConfig {
                apps: true,
                calculator: false,
                shell: false,
                clipboard: false,
            },
            apps: vec![first, second],
            ..Default::default()
        })
        .expect("test app provider should register")
    }

    #[test]
    fn empty_query_clears_results() {
        let mut state = app_only_state();

        state.set_query("freepalette-selection-test");
        assert!(!state.results().is_empty());

        state.set_query("");

        assert!(state.results().is_empty());
        assert_eq!(state.selected_index(), None);
        assert_eq!(state.status(), &PaletteStatus::Ready);
    }

    #[test]
    fn search_selects_first_result() {
        let mut state = PaletteState::from_config(Config::default())
            .expect("default providers should register");

        state.set_query("calc 2+2");

        assert_eq!(state.selected_index(), Some(0));
        assert_eq!(state.results()[0].result.title, "2+2 = 4");
    }

    #[test]
    fn selection_navigation_stays_in_bounds() {
        let mut state = app_only_state();

        state.set_query("freepalette-selection-test");
        state.move_selection(SelectionDirection::Next);
        state.move_selection(SelectionDirection::Next);

        assert_eq!(state.selected_index(), state.results().len().checked_sub(1));

        state.move_selection(SelectionDirection::Previous);
        state.move_selection(SelectionDirection::Previous);

        assert_eq!(state.selected_index(), Some(0));
    }

    #[test]
    fn execute_selected_records_outcome() {
        let mut state = PaletteState::from_config(Config::default())
            .expect("default providers should register");

        state.set_query("calc 2+2");
        let execution = state.execute_selected();

        assert_eq!(
            execution,
            PaletteExecution::Completed {
                hide_palette: false
            }
        );
        assert_eq!(
            state.status(),
            &PaletteStatus::Info("calculator result ready to copy: 4".to_string())
        );
    }

    #[test]
    fn execute_selected_blocks_shell_commands() {
        let mut state = PaletteState::from_config(Config::default())
            .expect("default providers should register");

        state.set_query("> echo hello");
        let execution = state.execute_selected();

        assert_eq!(execution, PaletteExecution::Blocked);
        assert_eq!(
            state.status(),
            &PaletteStatus::Error(
                "Shell commands cannot run from the UI yet; use the CLI with --allow-shell"
                    .to_string()
            )
        );
    }

    #[test]
    fn reset_for_next_activation_clears_visible_state() {
        let mut state = PaletteState::from_config(Config::default())
            .expect("default providers should register");

        state.set_query("calc 2+2");
        assert!(!state.results().is_empty());

        state.reset_for_next_activation();

        assert_eq!(state.query(), "");
        assert!(state.results().is_empty());
        assert_eq!(state.selected_index(), None);
        assert_eq!(state.status(), &PaletteStatus::Ready);
    }

    #[test]
    fn reload_config_keeps_current_query_visible() {
        let mut state = PaletteState::from_config(Config::default())
            .expect("default providers should register");

        state.set_query("calc 2+2");
        state.reload_config();

        assert_eq!(state.query(), "calc 2+2");
        assert_eq!(state.results()[0].result.title, "2+2 = 4");
        assert_eq!(
            state.status(),
            &PaletteStatus::Info("Config reloaded".to_string())
        );
    }

    #[test]
    fn app_launch_actions_hide_palette_after_successful_execution() {
        let launch = Action::LaunchApp {
            command: "notepad.exe".to_string(),
            args: Vec::new(),
        };
        let open_path = Action::OpenPath {
            path: "C:\\ProgramData\\Microsoft\\Windows\\Start Menu\\Programs\\Example.lnk"
                .to_string(),
        };
        let copy_text = Action::CopyText {
            text: "4".to_string(),
        };

        assert!(action_hides_palette_after_success(&launch));
        assert!(action_hides_palette_after_success(&open_path));
        assert!(!action_hides_palette_after_success(&copy_text));
    }
}
