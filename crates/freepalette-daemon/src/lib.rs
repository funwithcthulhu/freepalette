use std::path::{Path, PathBuf};

use freepalette_core::{
    providers::{
        AppLauncherProvider, CalculatorProvider, ClipboardHistoryProvider, ShellCommandProvider,
    },
    Action, ActionOutcome, AppIndexReport, Config, CoreError, ProviderRegistry, RankedResult,
    SearchResult,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DaemonError {
    #[error(transparent)]
    Core(#[from] CoreError),
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
}

impl DaemonState {
    /// Build state from an in-memory config.
    pub fn new(config: Config) -> Result<Self, DaemonError> {
        Self::from_config(config)
    }

    /// Build state from an in-memory config.
    pub fn from_config(config: Config) -> Result<Self, DaemonError> {
        Self::from_loaded_config(ConfigSource::Provided, config)
    }

    /// Load the platform default config file when it exists, otherwise use defaults.
    pub fn from_default_config() -> Result<Self, DaemonError> {
        let config = Config::load_default_or_default()?;
        Self::from_loaded_config(ConfigSource::Default, config)
    }

    /// Load config from an explicit path.
    pub fn load_from_path(path: &Path) -> Result<Self, DaemonError> {
        let config = Config::load_from_path(path)?;
        Self::from_loaded_config(ConfigSource::Path(path.to_path_buf()), config)
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
        Ok(self.registry.search(query, limit)?)
    }

    /// Execute a selected search result after applying the requested action policy.
    pub fn execute_result(
        &self,
        result: &SearchResult,
        policy: ActionExecutionPolicy,
    ) -> Result<ActionOutcome, DaemonError> {
        ensure_action_allowed(&result.action, policy)?;
        Ok(self.registry.execute(result)?)
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

    fn from_loaded_config(
        config_source: ConfigSource,
        config: Config,
    ) -> Result<Self, DaemonError> {
        let runtime = build_runtime(&config)?;
        Ok(Self {
            config_source,
            config,
            registry: runtime.registry,
            app_index_report: runtime.app_index_report,
            clipboard_history: Vec::new(),
        })
    }

    fn replace_config(&mut self, config: Config) -> Result<(), DaemonError> {
        let runtime = build_runtime(&config)?;
        self.config = config;
        self.registry = runtime.registry;
        self.app_index_report = runtime.app_index_report;
        Ok(())
    }

    fn rebuild_registry(&mut self) -> Result<(), DaemonError> {
        let runtime = build_runtime(&self.config)?;
        self.registry = runtime.registry;
        self.app_index_report = runtime.app_index_report;
        Ok(())
    }
}

struct DaemonRuntime {
    registry: ProviderRegistry,
    app_index_report: Option<AppIndexReport>,
}

fn build_runtime(config: &Config) -> Result<DaemonRuntime, CoreError> {
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
        registry.register(ClipboardHistoryProvider::empty())?;
    }

    Ok(DaemonRuntime {
        registry,
        app_index_report,
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

#[cfg(test)]
mod tests {
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    use freepalette_core::{AppEntry, GeneralConfig, ProviderConfig};

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
        })
        .expect("daemon state should initialize");

        let results = state
            .search("freepalette-daemon-limit", Some(2))
            .expect("search should succeed");

        assert_eq!(results.len(), 2);
    }

    #[test]
    fn execute_result_allows_non_shell_actions() {
        let state = DaemonState::from_config(Config {
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
        let state = DaemonState::from_config(Config {
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
        let state = DaemonState::from_config(Config::default())
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
