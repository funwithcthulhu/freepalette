use freepalette_core::{Action, Config, RankedResult};
use freepalette_daemon::{ActionExecutionPolicy, DaemonError, DaemonState};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum UiError {
    #[error(transparent)]
    Daemon(#[from] DaemonError),
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

    pub fn set_query(&mut self, query: impl Into<String>) {
        self.query = query.into();
        self.refresh_results();
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

    pub fn execute_selected(&mut self) {
        let Some(index) = self.selected else {
            self.status = PaletteStatus::Info("No result selected".to_string());
            return;
        };

        let Some(ranked) = self.results.get(index) else {
            self.status = PaletteStatus::Error("Selected result is unavailable".to_string());
            self.selected = None;
            return;
        };

        if matches!(ranked.result.action, Action::RunShell { .. }) {
            self.status = PaletteStatus::Error(
                "Shell commands cannot run from the UI yet; use the CLI with --allow-shell"
                    .to_string(),
            );
            return;
        }

        match self
            .daemon
            .execute_result(&ranked.result, ActionExecutionPolicy::BlockShellCommands)
        {
            Ok(outcome) => {
                self.status = PaletteStatus::Info(outcome.message);
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
        state.execute_selected();

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
        state.execute_selected();

        assert_eq!(
            state.status(),
            &PaletteStatus::Error(
                "Shell commands cannot run from the UI yet; use the CLI with --allow-shell"
                    .to_string()
            )
        );
    }
}
