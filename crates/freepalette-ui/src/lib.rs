mod autostart;
mod hotkey;
mod icon;
mod tray;

use std::{
    fs, io,
    path::PathBuf,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use freepalette_core::{Action, Config, RankedResult, SearchResult};
use freepalette_daemon::{
    default_ipc_endpoint_path, read_default_ipc_endpoint, send_ipc_request, ActionExecutionPolicy,
    ClipboardRecordOutcome, DaemonError, DaemonState, HotkeyState, IpcEndpoint, IpcError,
    IpcRequest, IpcResponse,
};
use thiserror::Error;

pub use autostart::{UiAutostart, UiAutostartError, UiAutostartStatus};
pub use hotkey::{UiHotkeyBridge, UiHotkeyError};
pub use icon::{app_icon_rgba, APP_ICON_SIZE};
pub use tray::{TrayCommand, UiTray, UiTrayError};

const DAEMON_START_TIMEOUT: Duration = Duration::from_secs(5);
const DAEMON_START_POLL_INTERVAL: Duration = Duration::from_millis(100);

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
    ipc_endpoint: Option<IpcEndpoint>,
    ipc_status: Option<IpcStatus>,
    daemon_connection: DaemonConnection,
    pending_shell_confirmation: Option<PendingShellConfirmation>,
}

impl PaletteState {
    pub fn from_default_config() -> Result<Self, UiError> {
        Ok(Self::from_daemon(DaemonState::from_default_config()?))
    }

    pub fn from_default_config_with_ipc() -> Result<Self, UiError> {
        let mut state = Self::from_default_config()?;
        if !state.try_connect_default_ipc() {
            state.try_start_and_connect_default_ipc();
        }
        Ok(state)
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
            ipc_endpoint: None,
            ipc_status: None,
            daemon_connection: DaemonConnection::InProcess,
            pending_shell_confirmation: None,
        }
    }

    #[cfg(test)]
    fn from_daemon_with_ipc_endpoint(daemon: DaemonState, endpoint: IpcEndpoint) -> Self {
        Self {
            query: String::new(),
            results: Vec::new(),
            selected: None,
            status: PaletteStatus::Ready,
            daemon,
            ipc_endpoint: Some(endpoint),
            ipc_status: None,
            daemon_connection: DaemonConnection::Connected,
            pending_shell_confirmation: None,
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

    pub fn hotkey_summary(&self) -> String {
        self.ipc_status
            .as_ref()
            .map(|status| status.hotkey.clone())
            .unwrap_or_else(|| self.daemon.hotkey_state().summary())
    }

    pub fn provider_ids(&self) -> Vec<String> {
        self.ipc_status
            .as_ref()
            .map(|status| status.providers.clone())
            .unwrap_or_else(|| self.daemon.provider_ids())
    }

    pub fn clipboard_history_len(&self) -> usize {
        self.ipc_status
            .as_ref()
            .map(|status| status.clipboard_history_len)
            .unwrap_or_else(|| self.daemon.clipboard_history_len())
    }

    pub fn clipboard_capture_enabled(&self) -> bool {
        self.ipc_status
            .as_ref()
            .map(|status| status.clipboard_capture_enabled)
            .unwrap_or_else(|| self.daemon.clipboard_capture_enabled())
    }

    pub fn recent_result_count(&self) -> usize {
        self.ipc_status
            .as_ref()
            .map(|status| status.recent_result_count)
            .unwrap_or_else(|| self.daemon.recent_result_count())
    }

    pub fn local_state_path(&self) -> Option<String> {
        self.ipc_status
            .as_ref()
            .and_then(|status| status.local_state_path.clone())
            .or_else(|| {
                self.daemon
                    .local_state_path()
                    .map(|path| path.display().to_string())
            })
    }

    pub fn config(&self) -> &Config {
        self.ipc_status
            .as_ref()
            .map(|status| &status.config)
            .unwrap_or_else(|| self.daemon.config())
    }

    pub fn daemon_connection_summary(&self) -> String {
        match (&self.ipc_endpoint, &self.daemon_connection) {
            (Some(endpoint), _) => format!("daemon IPC at {}", endpoint.address),
            (None, DaemonConnection::InProcess) => "in-process palette state".to_string(),
            (None, DaemonConnection::StartUnavailable(message))
            | (None, DaemonConnection::StartFailed(message)) => {
                format!("in-process palette state ({message})")
            }
            (None, DaemonConnection::Connected) => "in-process palette state".to_string(),
        }
    }

    pub fn set_query(&mut self, query: impl Into<String>) {
        self.pending_shell_confirmation = None;
        self.query = query.into();
        self.refresh_results();
    }

    pub fn record_clipboard_text(
        &mut self,
        text: impl Into<String>,
    ) -> Result<ClipboardRecordOutcome, UiError> {
        self.pending_shell_confirmation = None;
        let text = text.into();
        let outcome = if self.ipc_endpoint.is_some() {
            match self.ipc_response(IpcRequest::RecordClipboardText { text: text.clone() }) {
                Ok(IpcResponse::ClipboardRecorded { outcome, message }) => {
                    self.refresh_ipc_status();
                    self.refresh_results();
                    self.status = PaletteStatus::Info(message);
                    return Ok(outcome);
                }
                Ok(_) => {
                    self.status = PaletteStatus::Error(
                        "daemon returned an unexpected clipboard response".to_string(),
                    );
                    return Ok(ClipboardRecordOutcome::CaptureDisabled);
                }
                Err(IpcRequestFailure::Transport(error)) => {
                    self.disconnect_ipc_after_error(&error);
                    self.daemon.record_clipboard_text(text)?
                }
                Err(IpcRequestFailure::Daemon(message)) => {
                    self.status = PaletteStatus::Error(message);
                    return Ok(ClipboardRecordOutcome::CaptureDisabled);
                }
            }
        } else {
            self.daemon.record_clipboard_text(text)?
        };
        self.refresh_results();
        self.status = PaletteStatus::Info(clipboard_record_message(&outcome));
        Ok(outcome)
    }

    pub fn clear_clipboard_history(&mut self) -> Result<usize, UiError> {
        self.pending_shell_confirmation = None;
        let removed = if self.ipc_endpoint.is_some() {
            match self.ipc_response(IpcRequest::ClearClipboardHistory) {
                Ok(IpcResponse::ClipboardCleared { removed }) => {
                    self.refresh_ipc_status();
                    removed
                }
                Ok(_) => {
                    self.status = PaletteStatus::Error(
                        "daemon returned an unexpected clipboard clear response".to_string(),
                    );
                    return Ok(0);
                }
                Err(IpcRequestFailure::Transport(error)) => {
                    self.disconnect_ipc_after_error(&error);
                    self.daemon.clear_clipboard_history()?
                }
                Err(IpcRequestFailure::Daemon(message)) => {
                    self.status = PaletteStatus::Error(message);
                    return Ok(0);
                }
            }
        } else {
            self.daemon.clear_clipboard_history()?
        };
        self.refresh_results();
        self.status = PaletteStatus::Info(format!("Cleared {removed} clipboard item(s)"));
        Ok(removed)
    }

    pub fn record_clipboard_text_in_background(
        &mut self,
        text: impl Into<String>,
    ) -> Result<ClipboardRecordOutcome, UiError> {
        let text = text.into();
        let outcome = if self.ipc_endpoint.is_some() {
            match self.ipc_response(IpcRequest::RecordClipboardText { text: text.clone() }) {
                Ok(IpcResponse::ClipboardRecorded { outcome, .. }) => {
                    self.refresh_ipc_status();
                    outcome
                }
                Ok(_) => ClipboardRecordOutcome::CaptureDisabled,
                Err(IpcRequestFailure::Transport(error)) => {
                    self.disconnect_ipc_after_error(&error);
                    self.daemon.record_clipboard_text(text)?
                }
                Err(IpcRequestFailure::Daemon(_)) => ClipboardRecordOutcome::CaptureDisabled,
            }
        } else {
            self.daemon.record_clipboard_text(text)?
        };
        self.refresh_results();
        Ok(outcome)
    }

    pub fn set_provider_enabled(
        &mut self,
        provider_id: &str,
        enabled: bool,
    ) -> Result<(), UiError> {
        if self.ipc_endpoint.is_some() {
            match self.ipc_response(IpcRequest::SetProviderEnabled {
                provider_id: provider_id.to_string(),
                enabled,
            }) {
                Ok(response) => {
                    self.apply_ipc_config_response(response);
                    self.refresh_results();
                    self.status = PaletteStatus::Info("Config updated".to_string());
                    return Ok(());
                }
                Err(IpcRequestFailure::Transport(error)) => {
                    self.disconnect_ipc_after_error(&error);
                }
                Err(IpcRequestFailure::Daemon(message)) => {
                    self.status = PaletteStatus::Error(message);
                    return Ok(());
                }
            }
        }

        let mut config = self.daemon.config().clone();
        match provider_id {
            "apps" => config.providers.apps = enabled,
            "calculator" => config.providers.calculator = enabled,
            "shell" => config.providers.shell = enabled,
            "clipboard" => config.providers.clipboard = enabled,
            _ => {
                self.status = PaletteStatus::Error(format!("Unknown provider: {provider_id}"));
                return Ok(());
            }
        }

        self.daemon.update_config(config)?;
        self.refresh_results();
        self.status = PaletteStatus::Info("Config updated".to_string());
        Ok(())
    }

    pub fn set_clipboard_capture_enabled(&mut self, enabled: bool) -> Result<(), UiError> {
        if self.ipc_endpoint.is_some() {
            match self.ipc_response(IpcRequest::SetClipboardCapture { enabled }) {
                Ok(response) => {
                    self.apply_ipc_config_response(response);
                    self.refresh_results();
                    self.status =
                        PaletteStatus::Info("Clipboard capture setting updated".to_string());
                    return Ok(());
                }
                Err(IpcRequestFailure::Transport(error)) => {
                    self.disconnect_ipc_after_error(&error);
                }
                Err(IpcRequestFailure::Daemon(message)) => {
                    self.status = PaletteStatus::Error(message);
                    return Ok(());
                }
            }
        }

        let mut config = self.daemon.config().clone();
        config.clipboard.capture = enabled;
        self.daemon.update_config(config)?;
        self.refresh_results();
        self.status = PaletteStatus::Info("Clipboard capture setting updated".to_string());
        Ok(())
    }

    pub fn reset_for_next_activation(&mut self) {
        self.query.clear();
        self.results.clear();
        self.selected = None;
        self.status = PaletteStatus::Ready;
        self.pending_shell_confirmation = None;
    }

    pub fn move_selection(&mut self, direction: SelectionDirection) {
        let Some(current) = self.selected else {
            return;
        };

        self.pending_shell_confirmation = None;
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

        if let Action::RunShell { command } = ranked.result.primary_action() {
            self.pending_shell_confirmation = Some(PendingShellConfirmation {
                result_id: ranked.result.id.clone(),
                command: command.clone(),
            });
            self.status = PaletteStatus::Info("Confirm shell command before running".to_string());
            return PaletteExecution::NeedsShellConfirmation {
                command: command.clone(),
            };
        }

        self.pending_shell_confirmation = None;
        let hide_palette = action_hides_palette_after_success(ranked.result.primary_action());
        let result = ranked.result.clone();
        match self.execute_result(&result, false) {
            Ok(outcome) => {
                self.status = PaletteStatus::Info(outcome);
                PaletteExecution::Completed { hide_palette }
            }
            Err(error) => {
                self.status = PaletteStatus::Error(error);
                PaletteExecution::Failed
            }
        }
    }

    pub fn execute_confirmed_shell(&mut self) -> PaletteExecution {
        let Some(index) = self.selected else {
            self.pending_shell_confirmation = None;
            self.status = PaletteStatus::Info("No result selected".to_string());
            return PaletteExecution::NoSelection;
        };

        let Some(ranked) = self.results.get(index) else {
            self.pending_shell_confirmation = None;
            self.status = PaletteStatus::Error("Selected result is unavailable".to_string());
            self.selected = None;
            return PaletteExecution::SelectedUnavailable;
        };

        let Action::RunShell { command } = ranked.result.primary_action() else {
            self.pending_shell_confirmation = None;
            self.status =
                PaletteStatus::Error("Selected result is not a shell command".to_string());
            return PaletteExecution::SelectedUnavailable;
        };

        let Some(pending) = &self.pending_shell_confirmation else {
            self.status = PaletteStatus::Error(
                "Shell confirmation expired; select the command again".to_string(),
            );
            return PaletteExecution::Blocked;
        };

        if pending.result_id != ranked.result.id || pending.command != *command {
            self.pending_shell_confirmation = None;
            self.status = PaletteStatus::Error(
                "Shell confirmation no longer matches the selected command".to_string(),
            );
            return PaletteExecution::Blocked;
        }

        self.pending_shell_confirmation = None;
        let result = ranked.result.clone();
        match self.execute_result(&result, true) {
            Ok(outcome) => {
                self.status = PaletteStatus::Info(outcome);
                PaletteExecution::Completed {
                    hide_palette: false,
                }
            }
            Err(error) => {
                self.status = PaletteStatus::Error(error);
                PaletteExecution::Failed
            }
        }
    }

    pub fn cancel_shell_confirmation(&mut self) {
        self.pending_shell_confirmation = None;
        self.status = PaletteStatus::Info("Shell command was not run".to_string());
    }

    pub fn reload_config(&mut self) {
        self.pending_shell_confirmation = None;
        if self.ipc_endpoint.is_some() {
            match self.ipc_response(IpcRequest::ReloadConfig) {
                Ok(response) => {
                    self.apply_ipc_config_response(response);
                    let query = self.query.clone();
                    self.set_query(query);
                    self.status = PaletteStatus::Info("Config reloaded".to_string());
                    return;
                }
                Err(IpcRequestFailure::Transport(error)) => {
                    self.disconnect_ipc_after_error(&error);
                }
                Err(IpcRequestFailure::Daemon(message)) => {
                    self.status = PaletteStatus::Error(message);
                    return;
                }
            }
        }

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
            self.pending_shell_confirmation = None;
            return;
        }

        if self.ipc_endpoint.is_some() {
            match self.ipc_response(IpcRequest::Search {
                query: self.query.clone(),
                limit: None,
            }) {
                Ok(IpcResponse::Search { results }) => {
                    self.apply_results(results);
                    return;
                }
                Ok(_) => {
                    self.results.clear();
                    self.selected = None;
                    self.status = PaletteStatus::Error(
                        "daemon returned an unexpected search response".to_string(),
                    );
                    return;
                }
                Err(IpcRequestFailure::Transport(error)) => {
                    self.disconnect_ipc_after_error(&error);
                }
                Err(IpcRequestFailure::Daemon(message)) => {
                    self.results.clear();
                    self.selected = None;
                    self.status = PaletteStatus::Error(message);
                    return;
                }
            }
        }

        match self.daemon.search(&self.query, None) {
            Ok(results) => {
                self.apply_results(results);
            }
            Err(error) => {
                self.results.clear();
                self.selected = None;
                self.status = PaletteStatus::Error(error.to_string());
            }
        }
    }

    fn apply_results(&mut self, results: Vec<RankedResult>) {
        self.selected = if results.is_empty() { None } else { Some(0) };
        self.results = results;
        self.status = PaletteStatus::Ready;
    }

    fn execute_result(
        &mut self,
        result: &SearchResult,
        allow_shell: bool,
    ) -> Result<String, String> {
        if self.ipc_endpoint.is_some() {
            return match self.ipc_response(IpcRequest::ExecuteResult {
                result: result.clone(),
                allow_shell,
            }) {
                Ok(IpcResponse::Executed { message, .. }) => {
                    self.refresh_ipc_status();
                    Ok(message)
                }
                Ok(_) => Err("daemon returned an unexpected execution response".to_string()),
                Err(IpcRequestFailure::Transport(error)) => {
                    self.disconnect_ipc_after_error(&error);
                    Err(format!("daemon IPC execution failed: {error}"))
                }
                Err(IpcRequestFailure::Daemon(message)) => Err(message),
            };
        }

        let policy = if allow_shell {
            ActionExecutionPolicy::AllowShellCommands
        } else {
            ActionExecutionPolicy::BlockShellCommands
        };
        self.daemon
            .execute_result(result, policy)
            .map(|outcome| outcome.message)
            .map_err(|error| error.to_string())
    }

    fn try_connect_default_ipc(&mut self) -> bool {
        let Ok(endpoint) = read_default_ipc_endpoint() else {
            return false;
        };
        if let Ok(response) = ipc_response_for_endpoint(&endpoint, IpcRequest::Status) {
            self.ipc_endpoint = Some(endpoint);
            self.daemon_connection = DaemonConnection::Connected;
            self.apply_ipc_status_response(response);
            return true;
        }
        false
    }

    fn try_start_and_connect_default_ipc(&mut self) {
        match start_default_ipc_daemon() {
            Ok(endpoint) => match ipc_response_for_endpoint(&endpoint, IpcRequest::Status) {
                Ok(response) => {
                    self.ipc_endpoint = Some(endpoint);
                    self.daemon_connection = DaemonConnection::Connected;
                    self.apply_ipc_status_response(response);
                }
                Err(error) => {
                    self.daemon_connection =
                        DaemonConnection::StartFailed(format!("daemon status failed: {error}"));
                }
            },
            Err(DaemonStartError::ExecutableNotFound) => {
                self.daemon_connection =
                    DaemonConnection::StartUnavailable("daemon executable not found".to_string());
            }
            Err(error) => {
                self.daemon_connection = DaemonConnection::StartFailed(error.to_string());
            }
        }
    }

    fn refresh_ipc_status(&mut self) {
        if self.ipc_endpoint.is_none() {
            return;
        }
        match self.ipc_response(IpcRequest::Status) {
            Ok(response) => self.apply_ipc_status_response(response),
            Err(IpcRequestFailure::Transport(error)) => self.disconnect_ipc_after_error(&error),
            Err(IpcRequestFailure::Daemon(message)) => self.status = PaletteStatus::Error(message),
        }
    }

    fn apply_ipc_config_response(&mut self, response: IpcResponse) {
        match response {
            IpcResponse::ConfigUpdated {
                providers,
                config,
                provider_config: _,
                clipboard_capture_enabled,
            }
            | IpcResponse::Reloaded {
                providers,
                config,
                provider_config: _,
                clipboard_capture_enabled,
            } => {
                let previous = self.ipc_status.take();
                self.ipc_status = Some(IpcStatus {
                    providers,
                    config,
                    clipboard_capture_enabled,
                    clipboard_history_len: previous
                        .as_ref()
                        .map(|status| status.clipboard_history_len)
                        .unwrap_or(0),
                    recent_result_count: previous
                        .as_ref()
                        .map(|status| status.recent_result_count)
                        .unwrap_or(0),
                    hotkey: previous
                        .as_ref()
                        .map(|status| status.hotkey.clone())
                        .unwrap_or_else(|| "global hotkey disabled".to_string()),
                    local_state_path: previous.and_then(|status| status.local_state_path),
                });
                self.refresh_ipc_status();
            }
            other => self.apply_ipc_status_response(other),
        }
    }

    fn apply_ipc_status_response(&mut self, response: IpcResponse) {
        let IpcResponse::Status {
            providers,
            config,
            provider_config: _,
            clipboard_capture_enabled,
            clipboard_history_len,
            recent_result_count,
            hotkey,
            local_state_path,
        } = response
        else {
            return;
        };

        self.ipc_status = Some(IpcStatus {
            providers,
            config,
            clipboard_capture_enabled,
            clipboard_history_len,
            recent_result_count,
            hotkey,
            local_state_path,
        });
    }

    fn ipc_response(&self, request: IpcRequest) -> Result<IpcResponse, IpcRequestFailure> {
        let endpoint = self
            .ipc_endpoint
            .as_ref()
            .ok_or_else(|| IpcRequestFailure::Daemon("daemon IPC is not connected".to_string()))?;
        ipc_response_for_endpoint(endpoint, request)
    }

    fn disconnect_ipc_after_error(&mut self, error: &IpcError) {
        self.ipc_endpoint = None;
        self.ipc_status = None;
        self.daemon_connection =
            DaemonConnection::StartFailed(format!("daemon IPC unavailable: {error}"));
        tracing::debug!(%error, "daemon IPC unavailable; using in-process palette state");
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum DaemonConnection {
    InProcess,
    Connected,
    StartUnavailable(String),
    StartFailed(String),
}

#[derive(Debug, Clone)]
struct IpcStatus {
    providers: Vec<String>,
    config: Config,
    clipboard_capture_enabled: bool,
    clipboard_history_len: usize,
    recent_result_count: usize,
    hotkey: String,
    local_state_path: Option<String>,
}

#[derive(Debug)]
enum IpcRequestFailure {
    Transport(IpcError),
    Daemon(String),
}

#[derive(Debug, Error)]
enum DaemonStartError {
    #[error("daemon executable not found")]
    ExecutableNotFound,
    #[error("failed to locate current UI executable: {0}")]
    CurrentExecutable(io::Error),
    #[error("failed to remove stale daemon endpoint at {path}: {source}")]
    EndpointRemove { path: PathBuf, source: io::Error },
    #[error("failed to start daemon executable at {path}: {source}")]
    Spawn { path: PathBuf, source: io::Error },
    #[error("timed out waiting for daemon IPC endpoint")]
    Timeout,
}

fn ipc_response_for_endpoint(
    endpoint: &IpcEndpoint,
    request: IpcRequest,
) -> Result<IpcResponse, IpcRequestFailure> {
    let reply = send_ipc_request(endpoint, request).map_err(IpcRequestFailure::Transport)?;
    if !reply.ok {
        return Err(IpcRequestFailure::Daemon(reply.error.unwrap_or_else(
            || "daemon rejected request without a message".to_string(),
        )));
    }

    reply.response.ok_or_else(|| {
        IpcRequestFailure::Daemon("daemon returned an empty successful response".to_string())
    })
}

impl std::fmt::Display for IpcRequestFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Transport(error) => write!(formatter, "{error}"),
            Self::Daemon(message) => formatter.write_str(message),
        }
    }
}

fn start_default_ipc_daemon() -> Result<IpcEndpoint, DaemonStartError> {
    let executable = daemon_executable_path()?.ok_or(DaemonStartError::ExecutableNotFound)?;
    remove_stale_ipc_endpoint()?;

    let mut command = Command::new(&executable);
    command
        .arg("serve")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    apply_background_process_flags(&mut command);
    let _child = command.spawn().map_err(|source| DaemonStartError::Spawn {
        path: executable,
        source,
    })?;

    wait_for_default_ipc_endpoint()
}

fn daemon_executable_path() -> Result<Option<PathBuf>, DaemonStartError> {
    let current_executable =
        std::env::current_exe().map_err(DaemonStartError::CurrentExecutable)?;
    Ok(daemon_executable_candidates(&current_executable)
        .into_iter()
        .find(|candidate| candidate.is_file()))
}

fn daemon_executable_candidates(current_executable: &std::path::Path) -> Vec<PathBuf> {
    let Some(directory) = current_executable.parent() else {
        return Vec::new();
    };
    vec![directory.join(daemon_executable_file_name())]
}

#[cfg(windows)]
fn daemon_executable_file_name() -> &'static str {
    "freepalette-daemon.exe"
}

#[cfg(not(windows))]
fn daemon_executable_file_name() -> &'static str {
    "freepalette-daemon"
}

fn remove_stale_ipc_endpoint() -> Result<(), DaemonStartError> {
    let Some(path) = default_ipc_endpoint_path() else {
        return Ok(());
    };
    match fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(DaemonStartError::EndpointRemove { path, source }),
    }
}

fn wait_for_default_ipc_endpoint() -> Result<IpcEndpoint, DaemonStartError> {
    let deadline = Instant::now() + DAEMON_START_TIMEOUT;
    while Instant::now() < deadline {
        if let Ok(endpoint) = read_default_ipc_endpoint() {
            if send_ipc_request(&endpoint, IpcRequest::Status).is_ok() {
                return Ok(endpoint);
            }
        }
        thread::sleep(DAEMON_START_POLL_INTERVAL);
    }

    Err(DaemonStartError::Timeout)
}

#[cfg(windows)]
fn apply_background_process_flags(command: &mut Command) {
    use std::os::windows::process::CommandExt;

    const DETACHED_PROCESS: u32 = 0x0000_0008;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    command.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn apply_background_process_flags(_command: &mut Command) {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaletteExecution {
    NoSelection,
    SelectedUnavailable,
    Blocked,
    NeedsShellConfirmation { command: String },
    Completed { hide_palette: bool },
    Failed,
}

impl PaletteExecution {
    pub fn should_hide_palette(&self) -> bool {
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct PendingShellConfirmation {
    result_id: String,
    command: String,
}

fn action_hides_palette_after_success(action: &Action) -> bool {
    matches!(action, Action::LaunchApp { .. } | Action::OpenPath { .. })
}

fn clipboard_record_message(outcome: &ClipboardRecordOutcome) -> String {
    match outcome {
        ClipboardRecordOutcome::Stored => "Stored current clipboard text".to_string(),
        ClipboardRecordOutcome::CaptureDisabled => {
            "Clipboard capture is disabled in config".to_string()
        }
        ClipboardRecordOutcome::ProviderDisabled => {
            "Clipboard provider is disabled in config".to_string()
        }
        ClipboardRecordOutcome::RetentionDisabled => {
            "Clipboard retention is disabled in config".to_string()
        }
        ClipboardRecordOutcome::IgnoredEmpty => "Clipboard text is empty".to_string(),
        ClipboardRecordOutcome::IgnoredTooLarge {
            byte_count,
            max_bytes,
        } => format!("Clipboard text is too large: {byte_count} bytes exceeds {max_bytes}"),
    }
}

#[cfg(test)]
mod tests {
    use std::net::TcpListener;
    use std::path::PathBuf;

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
    fn execute_selected_requests_shell_confirmation() {
        let mut state = PaletteState::from_config(Config::default())
            .expect("default providers should register");

        state.set_query("> echo hello");
        let execution = state.execute_selected();

        assert_eq!(
            execution,
            PaletteExecution::NeedsShellConfirmation {
                command: "echo hello".to_string()
            }
        );
        assert_eq!(
            state.status(),
            &PaletteStatus::Info("Confirm shell command before running".to_string())
        );
    }

    #[test]
    fn confirmed_shell_command_requires_pending_confirmation() {
        let mut state = PaletteState::from_config(Config::default())
            .expect("default providers should register");

        state.set_query("> echo hello");
        let execution = state.execute_confirmed_shell();

        assert_eq!(execution, PaletteExecution::Blocked);
        assert_eq!(
            state.status(),
            &PaletteStatus::Error(
                "Shell confirmation expired; select the command again".to_string()
            )
        );
    }

    #[test]
    fn confirmed_shell_command_runs_after_confirmation() {
        let mut state = PaletteState::from_config(Config::default())
            .expect("default providers should register");

        state.set_query("> echo freepalette-shell-confirmed");
        let confirmation = state.execute_selected();
        let execution = state.execute_confirmed_shell();

        assert_eq!(
            confirmation,
            PaletteExecution::NeedsShellConfirmation {
                command: "echo freepalette-shell-confirmed".to_string()
            }
        );
        assert_eq!(
            execution,
            PaletteExecution::Completed {
                hide_palette: false
            }
        );
        assert_eq!(
            state.status(),
            &PaletteStatus::Info("shell exited with 0: freepalette-shell-confirmed".to_string())
        );
    }

    #[test]
    fn changing_query_clears_shell_confirmation() {
        let mut state = PaletteState::from_config(Config::default())
            .expect("default providers should register");

        state.set_query("> echo hello");
        assert!(matches!(
            state.execute_selected(),
            PaletteExecution::NeedsShellConfirmation { .. }
        ));

        state.set_query("calc 2+2");
        let execution = state.execute_confirmed_shell();

        assert_eq!(execution, PaletteExecution::SelectedUnavailable);
        assert_eq!(
            state.status(),
            &PaletteStatus::Error("Selected result is not a shell command".to_string())
        );
    }

    #[test]
    fn cancel_shell_confirmation_keeps_command_unrun() {
        let mut state = PaletteState::from_config(Config::default())
            .expect("default providers should register");

        state.set_query("> echo hello");
        let confirmation = state.execute_selected();
        state.cancel_shell_confirmation();
        let execution = state.execute_confirmed_shell();

        assert!(matches!(
            confirmation,
            PaletteExecution::NeedsShellConfirmation { .. }
        ));
        assert_eq!(execution, PaletteExecution::Blocked);
        assert_eq!(
            state.status(),
            &PaletteStatus::Error(
                "Shell confirmation expired; select the command again".to_string()
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

    #[test]
    fn settings_accessors_report_local_state() {
        let state = PaletteState::from_config(Config::default())
            .expect("default providers should register");

        assert!(state.provider_ids().iter().any(|id| id == "calculator"));
        assert_eq!(state.clipboard_history_len(), 0);
        assert_eq!(state.recent_result_count(), 0);
        assert_eq!(state.hotkey_summary(), "global hotkey disabled");
    }

    #[test]
    fn clipboard_record_status_does_not_echo_contents() {
        let mut state = PaletteState::from_config(Config::default())
            .expect("default providers should register");

        let outcome = state
            .record_clipboard_text("private-token-value")
            .expect("clipboard record should return a policy outcome");

        assert_eq!(outcome, ClipboardRecordOutcome::CaptureDisabled);
        assert_eq!(
            state.status(),
            &PaletteStatus::Info("Clipboard capture is disabled in config".to_string())
        );
    }

    #[test]
    fn provider_toggle_updates_visible_provider_state() {
        let mut state = PaletteState::from_config(Config::default())
            .expect("default providers should register");

        state
            .set_provider_enabled("calculator", false)
            .expect("provider toggle should update config");

        assert!(!state.provider_ids().iter().any(|id| id == "calculator"));
        assert_eq!(
            state.status(),
            &PaletteStatus::Info("Config updated".to_string())
        );
    }

    #[test]
    fn clipboard_capture_toggle_controls_recording_policy() {
        let mut state = PaletteState::from_config(Config::default())
            .expect("default providers should register");

        state
            .set_clipboard_capture_enabled(true)
            .expect("clipboard capture toggle should update config");
        let outcome = state
            .record_clipboard_text("temporary clipboard item")
            .expect("clipboard record should use updated policy");

        assert_eq!(outcome, ClipboardRecordOutcome::Stored);
        assert_eq!(state.clipboard_history_len(), 1);
    }

    #[test]
    fn dead_ipc_search_falls_back_to_in_process_state() {
        let listener =
            TcpListener::bind("127.0.0.1:0").expect("test should bind an unused local port");
        let address = listener
            .local_addr()
            .expect("test listener should report local address")
            .to_string();
        drop(listener);
        let endpoint = IpcEndpoint {
            address,
            token: "test-token".to_string(),
            pid: 0,
        };
        let daemon =
            DaemonState::from_config(Config::default()).expect("default providers should register");
        let mut state = PaletteState::from_daemon_with_ipc_endpoint(daemon, endpoint);

        state.set_query("calc 2+2");

        assert_eq!(state.results()[0].result.title, "2+2 = 4");
        assert!(state.ipc_endpoint.is_none());
        assert!(matches!(
            state.daemon_connection,
            DaemonConnection::StartFailed(_)
        ));
    }

    #[test]
    fn daemon_executable_candidate_uses_sibling_binary() {
        let current_executable = PathBuf::from("target")
            .join("debug")
            .join(format!("freepalette-ui{}", std::env::consts::EXE_SUFFIX));
        let candidates = daemon_executable_candidates(&current_executable);

        assert_eq!(candidates.len(), 1);
        assert_eq!(
            candidates[0],
            PathBuf::from("target")
                .join("debug")
                .join(daemon_executable_file_name())
        );
    }
}
