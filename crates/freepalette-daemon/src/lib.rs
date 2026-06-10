mod hotkey;
mod ipc;

use std::{
    fs, io,
    path::{Path, PathBuf},
};

use directories::ProjectDirs;
use freepalette_core::{
    providers::{
        AppLauncherProvider, CalculatorProvider, ClipboardHistoryProvider, ShellCommandProvider,
    },
    Action, ActionOutcome, AppIndexReport, Config, CoreError, ProviderRegistry, RankedResult,
    ResultKind, SearchResult,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[cfg(windows)]
pub use hotkey::windows_global_hotkey;
pub use hotkey::{HotkeyBinding, HotkeyError, HotkeyKey, HotkeyModifiers, HotkeyState};
pub use hotkey::{HotkeyLoopError, HotkeyLoopStatus};
pub use ipc::{
    default_ipc_endpoint_path, handle_ipc_request, read_default_ipc_endpoint, send_ipc_request,
    serve_ipc, IpcEndpoint, IpcError, IpcReply, IpcRequest, IpcResponse,
};

#[derive(Debug, Error)]
pub enum DaemonError {
    #[error(transparent)]
    Core(#[from] CoreError),
    #[error(transparent)]
    Hotkey(#[from] HotkeyError),
    #[error(transparent)]
    HotkeyLoop(#[from] HotkeyLoopError),
    #[error("failed to read local state at {path}: {source}")]
    StateRead { path: PathBuf, source: io::Error },
    #[error("failed to parse local state at {path}: {source}")]
    StateParse {
        path: PathBuf,
        source: serde_json::Error,
    },
    #[error("failed to serialize local state for {path}: {source}")]
    StateSerialize {
        path: PathBuf,
        source: serde_json::Error,
    },
    #[error("failed to write local state at {path}: {source}")]
    StateWrite { path: PathBuf, source: io::Error },
    #[error("unknown provider: {0}")]
    UnknownProvider(String),
    #[error("refusing to run shell command without explicit permission")]
    ShellCommandBlocked,
}

/// Policy for executing actions that can have local side effects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionExecutionPolicy {
    /// Allow app launches, no-op actions, and copy actions, but block shell commands.
    BlockShellCommands,
    /// Allow shell commands after the caller has made that permission explicit.
    AllowShellCommands,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ConfigSource {
    Default,
    Path(PathBuf),
    Provided,
}

/// Local state shared by the CLI, UI, and future long-running daemon process.
pub struct DaemonState {
    config_source: ConfigSource,
    config: Config,
    registry: ProviderRegistry,
    app_index_report: Option<AppIndexReport>,
    clipboard_history: Vec<String>,
    recent_result_ids: Vec<String>,
    local_state_path: Option<PathBuf>,
    hotkey_state: HotkeyState,
}

const RECENT_RESULT_LIMIT: usize = 25;
const RECENT_RESULT_SCORE_BOOST: i64 = 90;
const LOCAL_STATE_FILE_NAME: &str = "state.json";
const LOCAL_STATE_VERSION: u32 = 1;

impl DaemonState {
    /// Build state from an in-memory config.
    pub fn new(config: Config) -> Result<Self, DaemonError> {
        Self::from_config(config)
    }

    /// Build state from an in-memory config.
    pub fn from_config(config: Config) -> Result<Self, DaemonError> {
        Self::from_loaded_config(ConfigSource::Provided, config, None, LocalState::default())
    }

    /// Load the platform default config file when it exists, otherwise use defaults.
    pub fn from_default_config() -> Result<Self, DaemonError> {
        let config = Config::load_default_or_default()?;
        let local_state_path = default_local_state_path();
        let local_state = load_local_state(local_state_path.as_deref())?;
        Self::from_loaded_config(ConfigSource::Default, config, local_state_path, local_state)
    }

    /// Load config from an explicit path.
    pub fn load_from_path(path: &Path) -> Result<Self, DaemonError> {
        let config = Config::load_from_path(path)?;
        let local_state_path = default_local_state_path();
        let local_state = load_local_state(local_state_path.as_deref())?;
        Self::from_loaded_config(
            ConfigSource::Path(path.to_path_buf()),
            config,
            local_state_path,
            local_state,
        )
    }

    /// Reload config from the original source and rebuild provider state.
    pub fn reload_config(&mut self) -> Result<(), DaemonError> {
        let config = match &self.config_source {
            ConfigSource::Default => Config::load_default_or_default()?,
            ConfigSource::Path(path) => Config::load_from_path(path)?,
            ConfigSource::Provided => self.config.clone(),
        };

        self.replace_config(config)
    }

    /// Rebuild provider state so platform app indexing runs again.
    pub fn refresh_app_index(&mut self) -> Result<Option<&AppIndexReport>, DaemonError> {
        self.rebuild_registry()?;
        Ok(self.app_index_report.as_ref())
    }

    /// Search enabled providers with either an explicit limit or the configured default.
    pub fn search(
        &self,
        query: &str,
        limit: Option<usize>,
    ) -> Result<Vec<RankedResult>, DaemonError> {
        let limit = limit.unwrap_or(self.config.general.max_results);
        let search_limit = limit.saturating_add(self.recent_result_ids.len());
        let mut results = self.registry.search(query, search_limit)?;
        apply_recency_boost(&mut results, &self.recent_result_ids);
        results.truncate(limit);
        Ok(results)
    }

    /// Execute a selected search result after applying the requested action policy.
    pub fn execute_result(
        &mut self,
        result: &SearchResult,
        policy: ActionExecutionPolicy,
    ) -> Result<ActionOutcome, DaemonError> {
        ensure_action_allowed(result.primary_action(), policy)?;
        let outcome = self.registry.execute(result)?;
        self.record_recent_result(result);
        self.save_local_state()?;
        Ok(outcome)
    }

    /// Return the latest app index report when the app provider is enabled.
    pub fn app_index_report(&self) -> Option<&AppIndexReport> {
        self.app_index_report.as_ref()
    }

    /// Return registered provider IDs in registry order.
    pub fn provider_ids(&self) -> Vec<String> {
        self.registry
            .provider_ids()
            .into_iter()
            .map(|id| id.to_string())
            .collect()
    }

    /// Return the number of clipboard history items currently held in memory.
    pub fn clipboard_history_len(&self) -> usize {
        self.clipboard_history.len()
    }

    /// Return whether clipboard capture is enabled in the loaded config.
    pub fn clipboard_capture_enabled(&self) -> bool {
        self.config.providers.clipboard && self.config.clipboard.capture
    }

    /// Return the number of successfully executed results held for local recency ranking.
    pub fn recent_result_count(&self) -> usize {
        self.recent_result_ids.len()
    }

    /// Return the path used for local state persistence, when available.
    pub fn local_state_path(&self) -> Option<&Path> {
        self.local_state_path.as_deref()
    }

    /// Return the loaded config.
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Replace the current config and write it when this state was loaded from a path.
    pub fn update_config(&mut self, config: Config) -> Result<(), DaemonError> {
        match &self.config_source {
            ConfigSource::Path(path) => config.write_to_path(path)?,
            ConfigSource::Default => {
                if let Some(path) = Config::default_path() {
                    config.write_to_path(&path)?;
                    self.config_source = ConfigSource::Path(path);
                }
            }
            ConfigSource::Provided => {}
        }

        self.replace_config(config)
    }

    /// Return the current global-hotkey setup state.
    pub fn hotkey_state(&self) -> &HotkeyState {
        &self.hotkey_state
    }

    /// Run the foreground global-hotkey loop.
    ///
    /// On Windows, this blocks while the configured hotkey is registered. On
    /// unsupported platforms or when the hotkey is disabled, it returns a
    /// status immediately.
    pub fn run_hotkey_loop(&self) -> Result<HotkeyLoopStatus, DaemonError> {
        Ok(hotkey::run_hotkey_loop(&self.hotkey_state)?)
    }

    /// Add one clipboard item to the in-memory history.
    ///
    /// This does not read the system clipboard. A future long-running daemon can
    /// call this after a platform clipboard watcher is designed.
    pub fn record_clipboard_text(
        &mut self,
        text: impl Into<String>,
    ) -> Result<ClipboardRecordOutcome, DaemonError> {
        if !self.config.providers.clipboard {
            return Ok(ClipboardRecordOutcome::ProviderDisabled);
        }
        if !self.config.clipboard.capture {
            return Ok(ClipboardRecordOutcome::CaptureDisabled);
        }
        if self.config.clipboard.max_entries == 0 {
            return Ok(ClipboardRecordOutcome::RetentionDisabled);
        }

        let text = text.into();
        if text.trim().is_empty() {
            return Ok(ClipboardRecordOutcome::IgnoredEmpty);
        }
        if self
            .clipboard_history
            .first()
            .is_some_and(|existing| existing == &text)
        {
            return Ok(ClipboardRecordOutcome::Stored);
        }

        let byte_count = text.len();
        if byte_count > self.config.clipboard.max_entry_bytes {
            return Ok(ClipboardRecordOutcome::IgnoredTooLarge {
                byte_count,
                max_bytes: self.config.clipboard.max_entry_bytes,
            });
        }

        self.clipboard_history
            .retain(|existing| existing.as_str() != text.as_str());
        self.clipboard_history.insert(0, text);
        self.enforce_clipboard_limits();
        self.rebuild_registry()?;
        self.save_local_state()?;

        Ok(ClipboardRecordOutcome::Stored)
    }

    /// Clear all in-memory clipboard history and return the number of entries removed.
    pub fn clear_clipboard_history(&mut self) -> Result<usize, DaemonError> {
        let removed = self.clipboard_history.len();
        self.clipboard_history.clear();
        self.rebuild_registry()?;
        self.save_local_state()?;
        Ok(removed)
    }

    fn from_loaded_config(
        config_source: ConfigSource,
        config: Config,
        local_state_path: Option<PathBuf>,
        local_state: LocalState,
    ) -> Result<Self, DaemonError> {
        let mut clipboard_history = local_state.clipboard_history;
        apply_clipboard_config(&mut clipboard_history, &config);
        save_local_state(
            local_state_path.as_deref(),
            &LocalState {
                version: LOCAL_STATE_VERSION,
                clipboard_history: clipboard_history.clone(),
                recent_result_ids: local_state.recent_result_ids.clone(),
            },
        )?;
        let runtime = build_runtime(&config, &clipboard_history)?;
        Ok(Self {
            config_source,
            config,
            registry: runtime.registry,
            app_index_report: runtime.app_index_report,
            clipboard_history,
            recent_result_ids: local_state.recent_result_ids,
            local_state_path,
            hotkey_state: runtime.hotkey_state,
        })
    }

    #[cfg(test)]
    fn from_config_with_local_state_path(
        config: Config,
        local_state_path: PathBuf,
    ) -> Result<Self, DaemonError> {
        let local_state = load_local_state(Some(&local_state_path))?;
        Self::from_loaded_config(
            ConfigSource::Provided,
            config,
            Some(local_state_path),
            local_state,
        )
    }

    fn replace_config(&mut self, config: Config) -> Result<(), DaemonError> {
        let mut clipboard_history = self.clipboard_history.clone();
        apply_clipboard_config(&mut clipboard_history, &config);
        let runtime = build_runtime(&config, &clipboard_history)?;
        self.config = config;
        self.clipboard_history = clipboard_history;
        self.registry = runtime.registry;
        self.app_index_report = runtime.app_index_report;
        self.hotkey_state = runtime.hotkey_state;
        self.save_local_state()?;
        Ok(())
    }

    fn rebuild_registry(&mut self) -> Result<(), DaemonError> {
        let runtime = build_runtime(&self.config, &self.clipboard_history)?;
        self.registry = runtime.registry;
        self.app_index_report = runtime.app_index_report;
        self.hotkey_state = runtime.hotkey_state;
        Ok(())
    }

    fn enforce_clipboard_limits(&mut self) {
        if self.clipboard_history.len() > self.config.clipboard.max_entries {
            self.clipboard_history
                .truncate(self.config.clipboard.max_entries);
        }
    }

    fn record_recent_result(&mut self, result: &SearchResult) {
        if result.kind == ResultKind::Clipboard {
            return;
        }

        let identity = result_identity(result);
        self.recent_result_ids
            .retain(|existing| existing != &identity);
        self.recent_result_ids.insert(0, identity);
        if self.recent_result_ids.len() > RECENT_RESULT_LIMIT {
            self.recent_result_ids.truncate(RECENT_RESULT_LIMIT);
        }
    }

    fn save_local_state(&self) -> Result<(), DaemonError> {
        save_local_state(
            self.local_state_path.as_deref(),
            &LocalState {
                version: LOCAL_STATE_VERSION,
                clipboard_history: self.clipboard_history.clone(),
                recent_result_ids: self.recent_result_ids.clone(),
            },
        )
    }
}

struct DaemonRuntime {
    registry: ProviderRegistry,
    app_index_report: Option<AppIndexReport>,
    hotkey_state: HotkeyState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClipboardRecordOutcome {
    Stored,
    CaptureDisabled,
    ProviderDisabled,
    RetentionDisabled,
    IgnoredEmpty,
    IgnoredTooLarge { byte_count: usize, max_bytes: usize },
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
struct LocalState {
    #[serde(default)]
    version: u32,
    #[serde(default)]
    clipboard_history: Vec<String>,
    #[serde(default)]
    recent_result_ids: Vec<String>,
}

fn apply_clipboard_config(clipboard_history: &mut Vec<String>, config: &Config) {
    if !config.providers.clipboard || !config.clipboard.capture {
        clipboard_history.clear();
        return;
    }

    clipboard_history.retain(|entry| entry.len() <= config.clipboard.max_entry_bytes);
    if clipboard_history.len() > config.clipboard.max_entries {
        clipboard_history.truncate(config.clipboard.max_entries);
    }
}

fn build_runtime(
    config: &Config,
    clipboard_history: &[String],
) -> Result<DaemonRuntime, DaemonError> {
    let mut registry = ProviderRegistry::new();
    let mut app_index_report = None;

    if config.providers.apps {
        let app_provider = AppLauncherProvider::from_config(config);
        app_index_report = Some(app_provider.index_report());
        registry.register(app_provider)?;
    }
    if config.providers.calculator {
        registry.register(CalculatorProvider)?;
    }
    if config.providers.shell {
        registry.register(ShellCommandProvider)?;
    }
    if config.providers.clipboard {
        registry.register(ClipboardHistoryProvider::with_items(
            clipboard_history.to_vec(),
        ))?;
    }

    Ok(DaemonRuntime {
        registry,
        app_index_report,
        hotkey_state: HotkeyState::from_config(&config.hotkey)?,
    })
}

fn default_local_state_path() -> Option<PathBuf> {
    ProjectDirs::from("org", "freepalette", "freepalette")
        .map(|dirs| dirs.data_local_dir().join(LOCAL_STATE_FILE_NAME))
}

fn load_local_state(path: Option<&Path>) -> Result<LocalState, DaemonError> {
    let Some(path) = path else {
        return Ok(LocalState::default());
    };
    if !path.exists() {
        return Ok(LocalState::default());
    }

    let contents = fs::read_to_string(path).map_err(|source| DaemonError::StateRead {
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_str(&contents).map_err(|source| DaemonError::StateParse {
        path: path.to_path_buf(),
        source,
    })
}

fn save_local_state(path: Option<&Path>, state: &LocalState) -> Result<(), DaemonError> {
    let Some(path) = path else {
        return Ok(());
    };

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| DaemonError::StateWrite {
            path: path.to_path_buf(),
            source,
        })?;
    }

    let contents =
        serde_json::to_string_pretty(state).map_err(|source| DaemonError::StateSerialize {
            path: path.to_path_buf(),
            source,
        })?;
    fs::write(path, contents).map_err(|source| DaemonError::StateWrite {
        path: path.to_path_buf(),
        source,
    })
}

fn ensure_action_allowed(
    action: &Action,
    policy: ActionExecutionPolicy,
) -> Result<(), DaemonError> {
    // Search can return shell actions, but execution requires an explicit
    // caller policy so display-only paths cannot run shell commands by mistake.
    if matches!(action, Action::RunShell { .. })
        && policy == ActionExecutionPolicy::BlockShellCommands
    {
        return Err(DaemonError::ShellCommandBlocked);
    }

    Ok(())
}

fn apply_recency_boost(results: &mut [RankedResult], recent_result_ids: &[String]) {
    if recent_result_ids.is_empty() {
        return;
    }

    for result in results.iter_mut() {
        if let Some(position) = recent_result_ids
            .iter()
            .position(|recent| recent == &result_identity(&result.result))
        {
            result.score += RECENT_RESULT_SCORE_BOOST - position as i64;
        }
    }

    results.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| left.result.title.cmp(&right.result.title))
            .then_with(|| left.result.id.cmp(&right.result.id))
    });
}

fn result_identity(result: &SearchResult) -> String {
    format!("{}:{}", result.provider, result.id)
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    use freepalette_core::{
        AppEntry, ClipboardConfig, GeneralConfig, HotkeyConfig, ProviderConfig,
    };

    use super::*;

    fn temp_config_path(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after Unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("freepalette-daemon-{name}-{unique}.toml"))
    }

    fn provider_config(
        apps: bool,
        calculator: bool,
        shell: bool,
        clipboard: bool,
    ) -> ProviderConfig {
        ProviderConfig {
            apps,
            calculator,
            shell,
            clipboard,
        }
    }

    #[test]
    fn search_uses_configured_default_limit() {
        let mut first = AppEntry::new("First App", "first.exe");
        first.keywords = vec!["freepalette-daemon-limit".to_string()];
        let mut second = AppEntry::new("Second App", "second.exe");
        second.keywords = vec!["freepalette-daemon-limit".to_string()];
        let state = DaemonState::from_config(Config {
            general: GeneralConfig { max_results: 1 },
            providers: provider_config(true, false, false, false),
            apps: vec![first, second],
            ..Default::default()
        })
        .expect("daemon state should initialize");

        let results = state
            .search("freepalette-daemon-limit", None)
            .expect("search should succeed");

        assert_eq!(results.len(), 1);
    }

    #[test]
    fn explicit_limit_overrides_configured_default() {
        let mut first = AppEntry::new("First App", "first.exe");
        first.keywords = vec!["freepalette-daemon-limit".to_string()];
        let mut second = AppEntry::new("Second App", "second.exe");
        second.keywords = vec!["freepalette-daemon-limit".to_string()];
        let state = DaemonState::from_config(Config {
            general: GeneralConfig { max_results: 1 },
            providers: provider_config(true, false, false, false),
            apps: vec![first, second],
            ..Default::default()
        })
        .expect("daemon state should initialize");

        let results = state
            .search("freepalette-daemon-limit", Some(2))
            .expect("search should succeed");

        assert_eq!(results.len(), 2);
    }

    #[test]
    fn execute_result_allows_non_shell_actions() {
        let mut state = DaemonState::from_config(Config {
            providers: provider_config(false, true, false, false),
            ..Default::default()
        })
        .expect("daemon state should initialize");
        let results = state
            .search("calc 2+2", None)
            .expect("calculator search should succeed");

        let outcome = state
            .execute_result(
                &results[0].result,
                ActionExecutionPolicy::BlockShellCommands,
            )
            .expect("calculator result should execute");

        assert_eq!(outcome.message, "calculator result ready to copy: 4");
    }

    #[test]
    fn execute_result_blocks_shell_without_explicit_policy() {
        let mut state = DaemonState::from_config(Config {
            providers: provider_config(false, false, true, false),
            ..Default::default()
        })
        .expect("daemon state should initialize");
        let results = state
            .search("> echo hello", None)
            .expect("shell search should succeed");

        let error = state
            .execute_result(
                &results[0].result,
                ActionExecutionPolicy::BlockShellCommands,
            )
            .expect_err("shell command should be blocked");

        assert!(matches!(error, DaemonError::ShellCommandBlocked));
        assert_eq!(
            error.to_string(),
            "refusing to run shell command without explicit permission"
        );
    }

    #[test]
    fn default_config_registers_shell_provider_but_keeps_execution_blocked() {
        let mut state = DaemonState::from_config(Config::default())
            .expect("default daemon state should initialize");

        assert!(state.provider_ids().iter().any(|id| id == "shell"));

        let results = state
            .search("> echo hello", None)
            .expect("shell search should succeed with default config");
        let shell_result = results
            .iter()
            .find(|ranked| matches!(ranked.result.action, Action::RunShell { .. }))
            .expect("default config should expose shell search results");

        let error = state
            .execute_result(
                &shell_result.result,
                ActionExecutionPolicy::BlockShellCommands,
            )
            .expect_err("shell execution should still require explicit permission");

        assert!(matches!(error, DaemonError::ShellCommandBlocked));
    }

    #[test]
    fn load_from_path_minimal_config_uses_default_provider_values() {
        let path = temp_config_path("minimal-provider-defaults");
        fs::write(
            &path,
            r#"
                [providers]
                apps = false
            "#,
        )
        .expect("test config should be writable");

        let state = DaemonState::load_from_path(&path).expect("minimal config should load");
        fs::remove_file(&path).expect("test config should be removable");

        assert_eq!(
            state.provider_ids(),
            vec!["calculator", "shell", "clipboard"]
        );
        assert!(state.app_index_report().is_none());
    }

    #[test]
    fn stale_app_entry_does_not_change_shell_execution_guard() {
        let missing_app = temp_config_path("missing-app-target").with_extension("exe");
        let mut state = DaemonState::from_config(Config {
            providers: provider_config(true, false, true, false),
            apps: vec![AppEntry::new(
                "Stale App",
                missing_app.to_string_lossy().into_owned(),
            )],
            ..Default::default()
        })
        .expect("daemon state with stale app should initialize");

        let results = state
            .search("> echo hello", None)
            .expect("shell search should succeed with stale app present");
        let shell_result = results
            .iter()
            .find(|ranked| matches!(ranked.result.action, Action::RunShell { .. }))
            .expect("shell result should remain visible");

        let error = state
            .execute_result(
                &shell_result.result,
                ActionExecutionPolicy::BlockShellCommands,
            )
            .expect_err("shell execution should remain blocked");

        assert!(matches!(error, DaemonError::ShellCommandBlocked));
    }

    #[test]
    fn successful_execution_records_local_recency() {
        let mut state = DaemonState::from_config(Config {
            providers: provider_config(false, true, false, false),
            ..Default::default()
        })
        .expect("daemon state should initialize");
        let results = state
            .search("calc 2+2", None)
            .expect("calculator search should succeed");

        state
            .execute_result(
                &results[0].result,
                ActionExecutionPolicy::BlockShellCommands,
            )
            .expect("calculator execution should succeed");

        assert_eq!(state.recent_result_count(), 1);
    }

    #[test]
    fn blocked_shell_execution_does_not_record_recency() {
        let mut state = DaemonState::from_config(Config {
            providers: provider_config(false, false, true, false),
            ..Default::default()
        })
        .expect("daemon state should initialize");
        let results = state
            .search("> echo hello", None)
            .expect("shell search should succeed");

        let error = state
            .execute_result(
                &results[0].result,
                ActionExecutionPolicy::BlockShellCommands,
            )
            .expect_err("shell execution should remain blocked");

        assert!(matches!(error, DaemonError::ShellCommandBlocked));
        assert_eq!(state.recent_result_count(), 0);
    }

    #[test]
    fn recent_result_gets_small_ordering_boost() {
        let alpha_first = RankedResult {
            result: SearchResult::new(
                "test".into(),
                "alpha-first",
                "Alpha First",
                ResultKind::System,
                Action::Noop {
                    message: "first".to_string(),
                },
            ),
            score: 100,
        };
        let alpha_second = RankedResult {
            result: SearchResult::new(
                "test".into(),
                "alpha-second",
                "Alpha Second",
                ResultKind::System,
                Action::Noop {
                    message: "second".to_string(),
                },
            ),
            score: 100,
        };
        let recent_result_ids = vec![result_identity(&alpha_second.result)];
        let mut results = vec![alpha_first, alpha_second];

        apply_recency_boost(&mut results, &recent_result_ids);

        assert_eq!(results[0].result.title, "Alpha Second");
        assert!(results[0].score > results[1].score);
    }

    #[test]
    fn clipboard_execution_is_not_recorded_in_recency() {
        let mut state = DaemonState::from_config(Config {
            providers: provider_config(false, false, false, true),
            clipboard: ClipboardConfig {
                capture: true,
                max_entries: 10,
                max_entry_bytes: 128,
            },
            ..Default::default()
        })
        .expect("daemon state should initialize");
        state
            .record_clipboard_text("private-token-value")
            .expect("clipboard record should store");

        let results = state
            .search("private-token-value", None)
            .expect("clipboard search should succeed");

        state
            .execute_result(
                &results[0].result,
                ActionExecutionPolicy::BlockShellCommands,
            )
            .expect("clipboard copy action should execute");

        assert_eq!(state.recent_result_count(), 0);
    }

    #[test]
    fn local_state_persists_clipboard_history() {
        let state_path = temp_config_path("local-state-clipboard").with_extension("json");
        let config = Config {
            providers: provider_config(false, false, false, true),
            clipboard: ClipboardConfig {
                capture: true,
                max_entries: 10,
                max_entry_bytes: 128,
            },
            ..Default::default()
        };
        let mut state =
            DaemonState::from_config_with_local_state_path(config.clone(), state_path.clone())
                .expect("daemon state should initialize with test local state");

        state
            .record_clipboard_text("persisted clipboard item")
            .expect("clipboard record should persist local state");
        let reloaded = DaemonState::from_config_with_local_state_path(config, state_path.clone())
            .expect("daemon state should reload persisted local state");
        fs::remove_file(&state_path).expect("test local state should be removable");

        assert_eq!(reloaded.clipboard_history_len(), 1);
        assert_eq!(
            reloaded
                .search("persisted clipboard item", None)
                .expect("persisted clipboard item should be searchable")[0]
                .result
                .title,
            "persisted clipboard item"
        );
    }

    #[test]
    fn disabled_clipboard_capture_clears_persisted_clipboard_history() {
        let state_path = temp_config_path("local-state-disabled-clipboard").with_extension("json");
        let enabled_config = Config {
            providers: provider_config(false, false, false, true),
            clipboard: ClipboardConfig {
                capture: true,
                max_entries: 10,
                max_entry_bytes: 128,
            },
            ..Default::default()
        };
        let mut state =
            DaemonState::from_config_with_local_state_path(enabled_config, state_path.clone())
                .expect("daemon state should initialize with test local state");
        state
            .record_clipboard_text("persisted clipboard item")
            .expect("clipboard record should persist local state");

        let disabled_config = Config {
            providers: provider_config(false, false, false, true),
            clipboard: ClipboardConfig {
                capture: false,
                max_entries: 10,
                max_entry_bytes: 128,
            },
            ..Default::default()
        };
        let reloaded =
            DaemonState::from_config_with_local_state_path(disabled_config, state_path.clone())
                .expect("disabled capture should reload and scrub clipboard state");
        let state_file =
            fs::read_to_string(&state_path).expect("scrubbed local state should be readable");
        fs::remove_file(&state_path).expect("test local state should be removable");

        assert_eq!(reloaded.clipboard_history_len(), 0);
        assert!(!state_file.contains("persisted clipboard item"));
    }

    #[test]
    fn local_state_persists_non_clipboard_recency() {
        let state_path = temp_config_path("local-state-recency").with_extension("json");
        let config = Config {
            providers: provider_config(false, true, false, false),
            ..Default::default()
        };
        let mut state =
            DaemonState::from_config_with_local_state_path(config.clone(), state_path.clone())
                .expect("daemon state should initialize with test local state");
        let result = state
            .search("calc 2+2", None)
            .expect("calculator result should be searchable")[0]
            .result
            .clone();

        state
            .execute_result(&result, ActionExecutionPolicy::BlockShellCommands)
            .expect("calculator execution should persist recency");
        let reloaded = DaemonState::from_config_with_local_state_path(config, state_path.clone())
            .expect("daemon state should reload persisted recency");
        fs::remove_file(&state_path).expect("test local state should be removable");

        assert_eq!(reloaded.recent_result_count(), 1);
    }

    #[test]
    fn app_index_report_is_available_when_app_provider_is_enabled() {
        let state = DaemonState::from_config(Config {
            providers: provider_config(true, false, false, false),
            ..Default::default()
        })
        .expect("daemon state should initialize");

        assert!(state.app_index_report().is_some());
    }

    #[test]
    fn app_index_report_is_absent_when_app_provider_is_disabled() {
        let mut state = DaemonState::from_config(Config {
            providers: provider_config(false, false, false, false),
            ..Default::default()
        })
        .expect("daemon state should initialize");

        let report = state
            .refresh_app_index()
            .expect("app index refresh should rebuild registry");

        assert!(report.is_none());
    }

    #[test]
    fn clipboard_provider_can_be_disabled() {
        let state = DaemonState::from_config(Config {
            providers: provider_config(false, true, false, false),
            ..Default::default()
        })
        .expect("daemon state should initialize");

        assert_eq!(state.provider_ids(), vec!["calculator"]);
        assert_eq!(state.clipboard_history_len(), 0);
    }

    #[test]
    fn default_config_does_not_record_clipboard_history() {
        let mut state = DaemonState::from_config(Config::default())
            .expect("default daemon state should initialize");

        let outcome = state
            .record_clipboard_text("private-token-value")
            .expect("clipboard record should return a policy outcome");

        assert_eq!(outcome, ClipboardRecordOutcome::CaptureDisabled);
        assert_eq!(state.clipboard_history_len(), 0);
    }

    #[test]
    fn clipboard_provider_disabled_prevents_recording() {
        let mut state = DaemonState::from_config(Config {
            providers: provider_config(false, false, false, false),
            clipboard: ClipboardConfig {
                capture: true,
                ..Default::default()
            },
            ..Default::default()
        })
        .expect("daemon state should initialize");

        let outcome = state
            .record_clipboard_text("private-token-value")
            .expect("clipboard record should return a policy outcome");

        assert_eq!(outcome, ClipboardRecordOutcome::ProviderDisabled);
        assert_eq!(state.clipboard_history_len(), 0);
    }

    #[test]
    fn clipboard_history_respects_retention_limit() {
        let mut state = DaemonState::from_config(Config {
            providers: provider_config(false, false, false, true),
            clipboard: ClipboardConfig {
                capture: true,
                max_entries: 2,
                max_entry_bytes: 128,
            },
            ..Default::default()
        })
        .expect("daemon state should initialize");

        assert_eq!(
            state
                .record_clipboard_text("first")
                .expect("clipboard record should succeed"),
            ClipboardRecordOutcome::Stored
        );
        assert_eq!(
            state
                .record_clipboard_text("second")
                .expect("clipboard record should succeed"),
            ClipboardRecordOutcome::Stored
        );
        assert_eq!(
            state
                .record_clipboard_text("third")
                .expect("clipboard record should succeed"),
            ClipboardRecordOutcome::Stored
        );

        assert_eq!(state.clipboard_history_len(), 2);
    }

    #[test]
    fn clipboard_history_ignores_empty_and_oversized_entries() {
        let mut state = DaemonState::from_config(Config {
            providers: provider_config(false, false, false, true),
            clipboard: ClipboardConfig {
                capture: true,
                max_entries: 10,
                max_entry_bytes: 5,
            },
            ..Default::default()
        })
        .expect("daemon state should initialize");

        assert_eq!(
            state
                .record_clipboard_text("   ")
                .expect("empty clipboard record should be ignored"),
            ClipboardRecordOutcome::IgnoredEmpty
        );
        assert_eq!(
            state
                .record_clipboard_text("secret-token")
                .expect("oversized clipboard record should be ignored"),
            ClipboardRecordOutcome::IgnoredTooLarge {
                byte_count: 12,
                max_bytes: 5,
            }
        );
        assert_eq!(state.clipboard_history_len(), 0);
    }

    #[test]
    fn clearing_clipboard_history_removes_stored_entries() {
        let mut state = DaemonState::from_config(Config {
            providers: provider_config(false, false, false, true),
            clipboard: ClipboardConfig {
                capture: true,
                max_entries: 10,
                max_entry_bytes: 128,
            },
            ..Default::default()
        })
        .expect("daemon state should initialize");
        state
            .record_clipboard_text("private-token-value")
            .expect("clipboard record should succeed");

        let removed = state
            .clear_clipboard_history()
            .expect("clipboard clear should rebuild provider registry");

        assert_eq!(removed, 1);
        assert_eq!(state.clipboard_history_len(), 0);
        assert!(state
            .search("private-token-value", None)
            .expect("search should succeed")
            .is_empty());
    }

    #[test]
    fn clipboard_search_results_do_not_create_shell_actions() {
        let mut state = DaemonState::from_config(Config {
            providers: provider_config(false, false, false, true),
            clipboard: ClipboardConfig {
                capture: true,
                max_entries: 10,
                max_entry_bytes: 128,
            },
            ..Default::default()
        })
        .expect("daemon state should initialize");
        state
            .record_clipboard_text("> echo should-not-run")
            .expect("clipboard record should succeed");

        let results = state
            .search("echo", None)
            .expect("clipboard search should succeed");

        assert!(!results.is_empty());
        assert!(results
            .iter()
            .all(|ranked| !matches!(ranked.result.action, Action::RunShell { .. })));
    }

    #[test]
    fn daemon_reports_hotkey_state_from_config() {
        let state = DaemonState::from_config(Config {
            hotkey: HotkeyConfig {
                enabled: true,
                key: "Space".to_string(),
                ctrl: true,
                alt: true,
                shift: false,
                meta: false,
            },
            ..Default::default()
        })
        .expect("daemon state should initialize with hotkey config");

        if cfg!(target_os = "windows") {
            assert!(matches!(
                state.hotkey_state(),
                HotkeyState::ReadyForWindowsMessageLoop(_)
            ));
        } else {
            assert!(matches!(
                state.hotkey_state(),
                HotkeyState::UnsupportedPlatform { .. }
            ));
        }
    }

    #[test]
    fn reload_config_rebuilds_provider_registry_from_path() {
        let path = temp_config_path("reload");
        fs::write(
            &path,
            r#"
                [providers]
                apps = false
                calculator = true
                shell = false
                clipboard = false
            "#,
        )
        .expect("test config should be writable");

        let mut state = DaemonState::load_from_path(&path).expect("daemon state should load");
        assert_eq!(state.config_source, ConfigSource::Path(path.clone()));
        assert_eq!(state.provider_ids(), vec!["calculator"]);

        fs::write(
            &path,
            r#"
                [providers]
                apps = false
                calculator = false
                shell = true
                clipboard = false
            "#,
        )
        .expect("test config should be writable");

        state
            .reload_config()
            .expect("daemon state should reload config");
        fs::remove_file(&path).expect("test config should be removable");

        assert_eq!(state.provider_ids(), vec!["shell"]);
    }

    #[test]
    fn refresh_app_index_preserves_searchable_app_provider_state() {
        let mut configured = AppEntry::new("Refresh Target", "refresh-target.exe");
        configured.keywords = vec!["refresh-target-keyword".to_string()];
        let mut state = DaemonState::from_config(Config {
            providers: provider_config(true, false, false, false),
            apps: vec![configured],
            ..Default::default()
        })
        .expect("daemon state should initialize");

        let report = state
            .refresh_app_index()
            .expect("app index refresh should succeed")
            .expect("app provider should have an index report");
        assert!(report
            .entries
            .iter()
            .any(|entry| entry.name == "Refresh Target"));

        let results = state
            .search("refresh-target-keyword", None)
            .expect("search should use refreshed provider registry");
        assert!(results
            .iter()
            .any(|ranked| ranked.result.title == "Refresh Target"));
    }

    #[test]
    fn reload_config_updates_app_index_report_when_app_provider_changes() {
        let path = temp_config_path("reload-apps");
        fs::write(
            &path,
            r#"
                [providers]
                apps = false
                calculator = false
                shell = false
                clipboard = false
            "#,
        )
        .expect("test config should be writable");

        let mut state = DaemonState::load_from_path(&path).expect("daemon state should load");
        assert!(state.app_index_report().is_none());
        assert!(state.provider_ids().is_empty());

        fs::write(
            &path,
            r#"
                [providers]
                apps = true
                calculator = false
                shell = false
                clipboard = false

                [[apps]]
                name = "Reloaded App"
                command = "reloaded-app.exe"
                keywords = ["reload-app-keyword"]
            "#,
        )
        .expect("test config should be writable");

        state
            .reload_config()
            .expect("daemon state should reload config");
        fs::remove_file(&path).expect("test config should be removable");

        assert_eq!(state.provider_ids(), vec!["apps"]);
        let report = state
            .app_index_report()
            .expect("app provider should have an index report after reload");
        assert!(report
            .entries
            .iter()
            .any(|entry| entry.name == "Reloaded App"));
    }
}
