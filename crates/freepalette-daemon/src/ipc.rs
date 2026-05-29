use std::{
    fs,
    io::{self, BufRead, BufReader, Write},
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
};

use directories::ProjectDirs;
use freepalette_core::{AppIndexReport, Config, ProviderConfig, RankedResult, SearchResult};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::ClipboardRecordOutcome;
use crate::{ActionExecutionPolicy, DaemonError, DaemonState};

const IPC_ENDPOINT_FILE_NAME: &str = "ipc.json";
const DEFAULT_IPC_BIND_ADDR: &str = "127.0.0.1:0";

#[derive(Debug, Error)]
pub enum IpcError {
    #[error(transparent)]
    Daemon(#[from] DaemonError),
    #[error("failed to bind daemon IPC listener at {address}: {source}")]
    Bind { address: String, source: io::Error },
    #[error("failed to accept daemon IPC connection: {0}")]
    Accept(io::Error),
    #[error("failed to read daemon IPC endpoint at {path}: {source}")]
    EndpointRead { path: PathBuf, source: io::Error },
    #[error("failed to write daemon IPC endpoint at {path}: {source}")]
    EndpointWrite { path: PathBuf, source: io::Error },
    #[error("failed to parse daemon IPC endpoint at {path}: {source}")]
    EndpointParse {
        path: PathBuf,
        source: serde_json::Error,
    },
    #[error("failed to serialize daemon IPC endpoint for {path}: {source}")]
    EndpointSerialize {
        path: PathBuf,
        source: serde_json::Error,
    },
    #[error("failed to connect to daemon IPC at {address}: {source}")]
    Connect { address: String, source: io::Error },
    #[error("failed to write daemon IPC request: {0}")]
    RequestWrite(io::Error),
    #[error("failed to read daemon IPC response: {0}")]
    ResponseRead(io::Error),
    #[error("failed to parse daemon IPC request: {0}")]
    RequestParse(serde_json::Error),
    #[error("failed to parse daemon IPC response: {0}")]
    ResponseParse(serde_json::Error),
    #[error("failed to generate daemon IPC token: {0}")]
    Token(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IpcEndpoint {
    pub address: String,
    pub token: String,
    pub pid: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IpcEnvelope {
    pub token: String,
    pub request: IpcRequest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum IpcRequest {
    Status,
    Search {
        query: String,
        limit: Option<usize>,
    },
    Execute {
        query: String,
        limit: Option<usize>,
        allow_shell: bool,
    },
    ExecuteResult {
        result: SearchResult,
        allow_shell: bool,
    },
    RecordClipboardText {
        text: String,
    },
    ClearClipboardHistory,
    SetProviderEnabled {
        provider_id: String,
        enabled: bool,
    },
    SetClipboardCapture {
        enabled: bool,
    },
    RefreshAppIndex,
    ReloadConfig,
    Shutdown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum IpcResponse {
    Status {
        providers: Vec<String>,
        config: Config,
        provider_config: ProviderConfig,
        clipboard_capture_enabled: bool,
        clipboard_history_len: usize,
        recent_result_count: usize,
        hotkey: String,
        local_state_path: Option<String>,
    },
    Search {
        results: Vec<RankedResult>,
    },
    Executed {
        provider: String,
        title: String,
        message: String,
    },
    ClipboardRecorded {
        outcome: ClipboardRecordOutcome,
        message: String,
    },
    ClipboardCleared {
        removed: usize,
    },
    ConfigUpdated {
        providers: Vec<String>,
        config: Config,
        provider_config: ProviderConfig,
        clipboard_capture_enabled: bool,
    },
    AppIndexRefreshed {
        report: Option<AppIndexReport>,
    },
    Reloaded {
        providers: Vec<String>,
        config: Config,
        provider_config: ProviderConfig,
        clipboard_capture_enabled: bool,
    },
    ShuttingDown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IpcReply {
    pub ok: bool,
    pub response: Option<IpcResponse>,
    pub error: Option<String>,
}

impl IpcReply {
    fn ok(response: IpcResponse) -> Self {
        Self {
            ok: true,
            response: Some(response),
            error: None,
        }
    }

    fn error(error: impl Into<String>) -> Self {
        Self {
            ok: false,
            response: None,
            error: Some(error.into()),
        }
    }
}

pub fn default_ipc_endpoint_path() -> Option<PathBuf> {
    ProjectDirs::from("org", "freepalette", "freepalette").map(|dirs| {
        dirs.runtime_dir()
            .unwrap_or_else(|| dirs.data_local_dir())
            .join(IPC_ENDPOINT_FILE_NAME)
    })
}

pub fn read_default_ipc_endpoint() -> Result<IpcEndpoint, IpcError> {
    let path = default_ipc_endpoint_path().ok_or_else(|| IpcError::EndpointRead {
        path: PathBuf::from(IPC_ENDPOINT_FILE_NAME),
        source: io::Error::new(io::ErrorKind::NotFound, "runtime directory unavailable"),
    })?;
    read_ipc_endpoint(&path)
}

pub fn send_ipc_request(endpoint: &IpcEndpoint, request: IpcRequest) -> Result<IpcReply, IpcError> {
    let mut stream = TcpStream::connect(&endpoint.address).map_err(|source| IpcError::Connect {
        address: endpoint.address.clone(),
        source,
    })?;
    let envelope = IpcEnvelope {
        token: endpoint.token.clone(),
        request,
    };
    let request = serde_json::to_string(&envelope).map_err(IpcError::RequestParse)?;
    stream
        .write_all(request.as_bytes())
        .map_err(IpcError::RequestWrite)?;
    stream.write_all(b"\n").map_err(IpcError::RequestWrite)?;

    let mut response = String::new();
    BufReader::new(stream)
        .read_line(&mut response)
        .map_err(IpcError::ResponseRead)?;
    serde_json::from_str(&response).map_err(IpcError::ResponseParse)
}

pub fn serve_ipc(state: DaemonState, bind_addr: Option<&str>) -> Result<(), IpcError> {
    let bind_addr = bind_addr.unwrap_or(DEFAULT_IPC_BIND_ADDR);
    let listener = TcpListener::bind(bind_addr).map_err(|source| IpcError::Bind {
        address: bind_addr.to_string(),
        source,
    })?;
    let address = listener
        .local_addr()
        .map_err(|source| IpcError::Bind {
            address: bind_addr.to_string(),
            source,
        })?
        .to_string();
    let endpoint_path = default_ipc_endpoint_path().ok_or_else(|| IpcError::EndpointWrite {
        path: PathBuf::from(IPC_ENDPOINT_FILE_NAME),
        source: io::Error::new(io::ErrorKind::NotFound, "runtime directory unavailable"),
    })?;
    let endpoint = IpcEndpoint {
        address,
        token: generate_token()?,
        pid: std::process::id(),
    };
    write_ipc_endpoint(&endpoint_path, &endpoint)?;
    let _endpoint_guard = EndpointFileGuard {
        path: endpoint_path.clone(),
    };
    let mut state = state;

    tracing::info!(address = %endpoint.address, endpoint = %endpoint_path.display(), "daemon IPC server listening");
    println!("daemon IPC listening at {}", endpoint.address);
    println!("daemon IPC endpoint: {}", endpoint_path.display());

    for stream in listener.incoming() {
        let stream = stream.map_err(IpcError::Accept)?;
        match handle_connection(&mut state, stream, &endpoint.token) {
            Ok(ConnectionAction::Continue) => {}
            Ok(ConnectionAction::Shutdown) => break,
            Err(error) => tracing::warn!(%error, "daemon IPC connection failed"),
        }
    }

    Ok(())
}

pub fn handle_ipc_request(
    state: &mut DaemonState,
    request: IpcRequest,
) -> Result<IpcResponse, DaemonError> {
    match request {
        IpcRequest::Status => Ok(IpcResponse::Status {
            providers: state.provider_ids(),
            config: state.config().clone(),
            provider_config: state.config().providers.clone(),
            clipboard_capture_enabled: state.clipboard_capture_enabled(),
            clipboard_history_len: state.clipboard_history_len(),
            recent_result_count: state.recent_result_count(),
            hotkey: state.hotkey_state().summary(),
            local_state_path: state
                .local_state_path()
                .map(|path| path.display().to_string()),
        }),
        IpcRequest::Search { query, limit } => Ok(IpcResponse::Search {
            results: state.search(&query, limit)?,
        }),
        IpcRequest::Execute {
            query,
            limit,
            allow_shell,
        } => execute_top_result(state, &query, limit, allow_shell),
        IpcRequest::ExecuteResult {
            result,
            allow_shell,
        } => execute_result(state, result, allow_shell),
        IpcRequest::RecordClipboardText { text } => {
            let outcome = state.record_clipboard_text(text)?;
            Ok(IpcResponse::ClipboardRecorded {
                message: clipboard_record_message(&outcome),
                outcome,
            })
        }
        IpcRequest::ClearClipboardHistory => {
            let removed = state.clear_clipboard_history()?;
            Ok(IpcResponse::ClipboardCleared { removed })
        }
        IpcRequest::SetProviderEnabled {
            provider_id,
            enabled,
        } => {
            let mut config = state.config().clone();
            match provider_id.as_str() {
                "apps" => config.providers.apps = enabled,
                "calculator" => config.providers.calculator = enabled,
                "shell" => config.providers.shell = enabled,
                "clipboard" => config.providers.clipboard = enabled,
                _ => return Err(DaemonError::UnknownProvider(provider_id)),
            }
            state.update_config(config)?;
            Ok(config_updated_response(state))
        }
        IpcRequest::SetClipboardCapture { enabled } => {
            let mut config = state.config().clone();
            config.clipboard.capture = enabled;
            state.update_config(config)?;
            Ok(config_updated_response(state))
        }
        IpcRequest::RefreshAppIndex => {
            let report = state.refresh_app_index()?.cloned();
            Ok(IpcResponse::AppIndexRefreshed { report })
        }
        IpcRequest::ReloadConfig => {
            state.reload_config()?;
            Ok(IpcResponse::Reloaded {
                providers: state.provider_ids(),
                config: state.config().clone(),
                provider_config: state.config().providers.clone(),
                clipboard_capture_enabled: state.clipboard_capture_enabled(),
            })
        }
        IpcRequest::Shutdown => Ok(IpcResponse::ShuttingDown),
    }
}

fn execute_result(
    state: &mut DaemonState,
    result: SearchResult,
    allow_shell: bool,
) -> Result<IpcResponse, DaemonError> {
    let policy = if allow_shell {
        ActionExecutionPolicy::AllowShellCommands
    } else {
        ActionExecutionPolicy::BlockShellCommands
    };
    let outcome = state.execute_result(&result, policy)?;

    Ok(IpcResponse::Executed {
        provider: result.provider.to_string(),
        title: result.title,
        message: outcome.message,
    })
}

fn execute_top_result(
    state: &mut DaemonState,
    query: &str,
    limit: Option<usize>,
    allow_shell: bool,
) -> Result<IpcResponse, DaemonError> {
    let results = state.search(query, limit)?;
    let Some(first) = results.first() else {
        return Ok(IpcResponse::Executed {
            provider: String::new(),
            title: String::new(),
            message: "Nothing to run".to_string(),
        });
    };
    let policy = if allow_shell {
        ActionExecutionPolicy::AllowShellCommands
    } else {
        ActionExecutionPolicy::BlockShellCommands
    };
    let result = first.result.clone();
    let outcome = state.execute_result(&result, policy)?;

    Ok(IpcResponse::Executed {
        provider: result.provider.to_string(),
        title: result.title,
        message: outcome.message,
    })
}

fn config_updated_response(state: &DaemonState) -> IpcResponse {
    IpcResponse::ConfigUpdated {
        providers: state.provider_ids(),
        config: state.config().clone(),
        provider_config: state.config().providers.clone(),
        clipboard_capture_enabled: state.clipboard_capture_enabled(),
    }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConnectionAction {
    Continue,
    Shutdown,
}

fn handle_connection(
    state: &mut DaemonState,
    mut stream: TcpStream,
    token: &str,
) -> Result<ConnectionAction, IpcError> {
    let mut line = String::new();
    BufReader::new(stream.try_clone().map_err(IpcError::ResponseRead)?)
        .read_line(&mut line)
        .map_err(IpcError::ResponseRead)?;
    let (reply, action) = reply_for_line(state, &line, token);
    let reply = serde_json::to_string(&reply).map_err(IpcError::ResponseParse)?;
    stream
        .write_all(reply.as_bytes())
        .map_err(IpcError::RequestWrite)?;
    stream.write_all(b"\n").map_err(IpcError::RequestWrite)?;
    Ok(action)
}

fn reply_for_line(
    state: &mut DaemonState,
    line: &str,
    token: &str,
) -> (IpcReply, ConnectionAction) {
    let mut action = ConnectionAction::Continue;
    let reply = match serde_json::from_str::<IpcEnvelope>(line) {
        Ok(envelope) if envelope.token == token => {
            if envelope.request == IpcRequest::Shutdown {
                action = ConnectionAction::Shutdown;
            }
            match handle_ipc_request(state, envelope.request) {
                Ok(response) => IpcReply::ok(response),
                Err(error) => IpcReply::error(error.to_string()),
            }
        }
        Ok(_) => IpcReply::error("invalid daemon IPC token"),
        Err(error) => IpcReply::error(format!("invalid daemon IPC request: {error}")),
    };
    (reply, action)
}

fn read_ipc_endpoint(path: &Path) -> Result<IpcEndpoint, IpcError> {
    let contents = fs::read_to_string(path).map_err(|source| IpcError::EndpointRead {
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_str(&contents).map_err(|source| IpcError::EndpointParse {
        path: path.to_path_buf(),
        source,
    })
}

fn write_ipc_endpoint(path: &Path, endpoint: &IpcEndpoint) -> Result<(), IpcError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| IpcError::EndpointWrite {
            path: path.to_path_buf(),
            source,
        })?;
    }
    let contents =
        serde_json::to_string_pretty(endpoint).map_err(|source| IpcError::EndpointSerialize {
            path: path.to_path_buf(),
            source,
        })?;
    fs::write(path, contents).map_err(|source| IpcError::EndpointWrite {
        path: path.to_path_buf(),
        source,
    })
}

fn generate_token() -> Result<String, IpcError> {
    let mut bytes = [0_u8; 32];
    getrandom::fill(&mut bytes).map_err(|source| IpcError::Token(source.to_string()))?;
    Ok(hex_encode(&bytes))
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(hex_digit(byte >> 4));
        output.push(hex_digit(byte & 0x0f));
    }
    output
}

fn hex_digit(value: u8) -> char {
    match value {
        0..=9 => char::from(b'0' + value),
        10..=15 => char::from(b'a' + (value - 10)),
        _ => '0',
    }
}

struct EndpointFileGuard {
    path: PathBuf,
}

impl Drop for EndpointFileGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use freepalette_core::{ClipboardConfig, Config, ProviderConfig};

    use super::*;

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

    fn default_test_state() -> DaemonState {
        DaemonState::from_config(Config::default()).expect("daemon state should initialize")
    }

    fn assert_structured_error(reply: IpcReply, expected: &str) {
        assert!(!reply.ok);
        assert!(reply.response.is_none());
        let error = reply.error.expect("error reply should include a message");
        assert!(
            error.contains(expected),
            "expected error containing {expected:?}, got {error:?}"
        );
    }

    fn envelope_json(token: &str, request: IpcRequest) -> String {
        serde_json::to_string(&IpcEnvelope {
            token: token.to_string(),
            request,
        })
        .expect("test envelope should serialize")
    }

    #[test]
    fn status_request_reports_daemon_state() {
        let mut state =
            DaemonState::from_config(Config::default()).expect("daemon state should initialize");

        let response =
            handle_ipc_request(&mut state, IpcRequest::Status).expect("status should succeed");

        let IpcResponse::Status { providers, .. } = response else {
            unreachable!("status request should return status response");
        };
        assert!(providers.iter().any(|provider| provider == "calculator"));
    }

    #[test]
    fn search_request_returns_ranked_results() {
        let mut state = DaemonState::from_config(Config {
            providers: provider_config(false, true, false, false),
            ..Default::default()
        })
        .expect("daemon state should initialize");

        let response = handle_ipc_request(
            &mut state,
            IpcRequest::Search {
                query: "calc 2+2".to_string(),
                limit: None,
            },
        )
        .expect("search should succeed");

        let IpcResponse::Search { results } = response else {
            unreachable!("search request should return search response");
        };
        assert_eq!(results[0].result.title, "2+2 = 4");
    }

    #[test]
    fn ipc_rejects_missing_or_wrong_token() {
        let mut state = default_test_state();
        let (reply, action) = reply_for_line(
            &mut state,
            r#"{"request":{"type":"status"}}"#,
            "correct-token",
        );

        assert_eq!(action, ConnectionAction::Continue);
        assert_structured_error(reply, "invalid daemon IPC request");

        let (reply, action) = reply_for_line(
            &mut state,
            r#"{"token":"wrong-token","request":{"type":"status"}}"#,
            "correct-token",
        );

        assert_eq!(action, ConnectionAction::Continue);
        assert_structured_error(reply, "invalid daemon IPC token");
    }

    #[test]
    fn ipc_malformed_json_returns_one_structured_error() {
        let mut state = default_test_state();
        let (reply, action) = reply_for_line(&mut state, "{not-json", "correct-token");

        assert_eq!(action, ConnectionAction::Continue);
        assert_structured_error(reply, "invalid daemon IPC request");
    }

    #[test]
    fn ipc_unknown_method_returns_one_structured_error() {
        let mut state = default_test_state();
        let (reply, action) = reply_for_line(
            &mut state,
            r#"{"token":"correct-token","request":{"type":"unknown-method"}}"#,
            "correct-token",
        );

        assert_eq!(action, ConnectionAction::Continue);
        assert_structured_error(reply, "invalid daemon IPC request");
    }

    #[test]
    fn ipc_replies_do_not_echo_clipboard_contents() {
        let secret = "private-clipboard-token";
        let mut state = DaemonState::from_config(Config {
            providers: provider_config(false, false, false, true),
            clipboard: ClipboardConfig {
                capture: true,
                max_entries: 10,
                max_entry_bytes: 1024,
            },
            ..Default::default()
        })
        .expect("daemon state should initialize");
        let token = "correct-token";

        let (record_reply, action) = reply_for_line(
            &mut state,
            &envelope_json(
                token,
                IpcRequest::RecordClipboardText {
                    text: secret.to_string(),
                },
            ),
            token,
        );
        assert_eq!(action, ConnectionAction::Continue);
        assert!(record_reply.ok);
        let record_json =
            serde_json::to_string(&record_reply).expect("record reply should serialize");
        assert!(!record_json.contains(secret));

        let (status_reply, action) =
            reply_for_line(&mut state, &envelope_json(token, IpcRequest::Status), token);
        assert_eq!(action, ConnectionAction::Continue);
        assert!(status_reply.ok);
        let status_json =
            serde_json::to_string(&status_reply).expect("status reply should serialize");
        assert!(!status_json.contains(secret));

        let (error_reply, action) = reply_for_line(
            &mut state,
            &format!(
                r#"{{"token":"{token}","request":{{"type":"record-clipboard-text","text":"{secret}""#
            ),
            token,
        );
        assert_eq!(action, ConnectionAction::Continue);
        assert!(!error_reply.ok);
        let error_json = serde_json::to_string(&error_reply).expect("error reply should serialize");
        assert!(!error_json.contains(secret));
    }

    #[test]
    fn execute_request_keeps_shell_guard() {
        let mut state = DaemonState::from_config(Config {
            providers: provider_config(false, false, true, false),
            ..Default::default()
        })
        .expect("daemon state should initialize");

        let error = handle_ipc_request(
            &mut state,
            IpcRequest::Execute {
                query: "> echo hello".to_string(),
                limit: None,
                allow_shell: false,
            },
        )
        .expect_err("shell execution should stay blocked");

        assert!(matches!(error, DaemonError::ShellCommandBlocked));
    }

    #[test]
    fn execute_result_request_keeps_shell_guard() {
        let mut state = DaemonState::from_config(Config {
            providers: provider_config(false, false, true, false),
            ..Default::default()
        })
        .expect("daemon state should initialize");
        let results = state
            .search("> echo hello", None)
            .expect("shell search should succeed");

        let error = handle_ipc_request(
            &mut state,
            IpcRequest::ExecuteResult {
                result: results[0].result.clone(),
                allow_shell: false,
            },
        )
        .expect_err("selected shell execution should stay blocked");

        assert!(matches!(error, DaemonError::ShellCommandBlocked));
    }

    #[test]
    fn execute_result_request_runs_selected_non_shell_result() {
        let mut state = DaemonState::from_config(Config {
            providers: provider_config(false, true, false, false),
            ..Default::default()
        })
        .expect("daemon state should initialize");
        let results = state
            .search("calc 2+2", None)
            .expect("calculator search should succeed");

        let response = handle_ipc_request(
            &mut state,
            IpcRequest::ExecuteResult {
                result: results[0].result.clone(),
                allow_shell: false,
            },
        )
        .expect("selected calculator result should execute");

        let IpcResponse::Executed { message, .. } = response else {
            unreachable!("execute-result request should return execution response");
        };
        assert_eq!(message, "calculator result ready to copy: 4");
    }

    #[test]
    fn ipc_provider_toggle_updates_daemon_config() {
        let mut state =
            DaemonState::from_config(Config::default()).expect("daemon state should initialize");

        let response = handle_ipc_request(
            &mut state,
            IpcRequest::SetProviderEnabled {
                provider_id: "calculator".to_string(),
                enabled: false,
            },
        )
        .expect("provider toggle should update config");

        let IpcResponse::ConfigUpdated {
            provider_config, ..
        } = response
        else {
            unreachable!("provider toggle should return config update response");
        };
        assert!(!provider_config.calculator);
        assert!(!state.provider_ids().iter().any(|id| id == "calculator"));
    }

    #[test]
    fn refresh_app_index_request_returns_latest_report() {
        let mut state = DaemonState::from_config(Config {
            providers: provider_config(true, false, false, false),
            ..Default::default()
        })
        .expect("daemon state should initialize");

        let response = handle_ipc_request(&mut state, IpcRequest::RefreshAppIndex)
            .expect("app index refresh should succeed");

        let IpcResponse::AppIndexRefreshed { report } = response else {
            unreachable!("refresh request should return app index report response");
        };
        assert!(report.is_some());
    }

    #[test]
    fn refresh_app_index_request_reports_disabled_provider() {
        let mut state = DaemonState::from_config(Config {
            providers: provider_config(false, true, false, false),
            ..Default::default()
        })
        .expect("daemon state should initialize");

        let response = handle_ipc_request(&mut state, IpcRequest::RefreshAppIndex)
            .expect("app index refresh should succeed");

        let IpcResponse::AppIndexRefreshed { report } = response else {
            unreachable!("refresh request should return app index report response");
        };
        assert!(report.is_none());
    }

    #[test]
    fn shutdown_request_returns_shutdown_response() {
        let mut state =
            DaemonState::from_config(Config::default()).expect("daemon state should initialize");

        let response = handle_ipc_request(&mut state, IpcRequest::Shutdown)
            .expect("shutdown request should be accepted");

        assert_eq!(response, IpcResponse::ShuttingDown);
    }

    #[test]
    fn token_generation_returns_hex_token() {
        let token = generate_token().expect("token generation should succeed");

        assert_eq!(token.len(), 64);
        assert!(token.chars().all(|character| character.is_ascii_hexdigit()));
    }
}
