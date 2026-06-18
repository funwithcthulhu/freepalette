use std::{
    sync::{Mutex, MutexGuard},
    time::{Duration, Instant},
};

use freepalette_core::{Action, RankedResult, ResultKind};
use freepalette_ui::{
    PaletteExecution, PaletteState, PaletteStatus, SelectionDirection, TrayCommand, UiAutostart,
    UiHotkeyBridge, UiTray,
};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, RunEvent, State, WebviewWindow, Window, WindowEvent};

const MAIN_WINDOW_LABEL: &str = "main";
const PALETTE_UPDATED_EVENT: &str = "palette-updated";
const PALETTE_SHOWN_EVENT: &str = "palette-shown";
const LIFECYCLE_POLL_INTERVAL: Duration = Duration::from_millis(100);
const CLIPBOARD_POLL_INTERVAL: Duration = Duration::from_secs(2);

struct AppState {
    palette: Mutex<PaletteState>,
    close_policy: WindowClosePolicy,
}

impl AppState {
    fn new(palette: PaletteState, close_policy: WindowClosePolicy) -> Self {
        Self {
            palette: Mutex::new(palette),
            close_policy,
        }
    }
}

struct UiLifecycle {
    hotkey_bridge: UiHotkeyBridge,
    tray: Option<UiTray>,
    next_clipboard_poll: Instant,
}

impl UiLifecycle {
    fn new(hotkey_bridge: UiHotkeyBridge, tray: Option<UiTray>) -> Self {
        Self {
            hotkey_bridge,
            tray,
            next_clipboard_poll: Instant::now() + CLIPBOARD_POLL_INTERVAL,
        }
    }

    fn is_active(&self) -> bool {
        self.hotkey_bridge.is_active() || self.tray.as_ref().map(UiTray::is_active).unwrap_or(false)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WindowClosePolicy {
    CloseProcess,
    HideToBackground,
}

impl WindowClosePolicy {
    fn from_lifecycle(hotkey_active: bool, tray_active: bool) -> Self {
        if hotkey_active || tray_active {
            Self::HideToBackground
        } else {
            Self::CloseProcess
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PaletteSnapshot {
    query: String,
    results: Vec<RankedResult>,
    selected_index: Option<usize>,
    status: StatusSnapshot,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SettingsSnapshot {
    provider_ids: Vec<String>,
    providers: ProviderSettingsSnapshot,
    clipboard_capture_enabled: bool,
    clipboard_history_len: usize,
    recent_result_count: usize,
    hotkey_summary: String,
    local_state_path: Option<String>,
    daemon_connection: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProviderSettingsSnapshot {
    apps: bool,
    calculator: bool,
    shell: bool,
    clipboard: bool,
}

#[derive(Debug, Serialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
enum StatusSnapshot {
    Ready,
    Info { message: String },
    Error { message: String },
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ExecutionSnapshot {
    execution: ExecutionState,
    palette: PaletteSnapshot,
}

#[derive(Debug, Serialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
enum ExecutionState {
    NoSelection,
    SelectedUnavailable,
    Blocked,
    NeedsShellConfirmation { command: String },
    Completed { hide_palette: bool },
    Failed,
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_target(false).init();

    let mut palette = PaletteState::from_default_config_with_ipc()?;
    let hotkey_bridge = UiHotkeyBridge::from_state(palette.hotkey_state())?;
    if let Some(label) = hotkey_bridge.label() {
        tracing::info!(hotkey = %label, "UI global hotkey registered");
    }

    let tray = create_tray(&mut palette);
    let tray_active = tray.as_ref().map(UiTray::is_active).unwrap_or(false);
    let close_policy = WindowClosePolicy::from_lifecycle(hotkey_bridge.is_active(), tray_active);
    let mut lifecycle = UiLifecycle::new(hotkey_bridge, tray);

    let app = tauri::Builder::default()
        .manage(AppState::new(palette, close_policy))
        .on_window_event(|window, event| {
            if window.label() != MAIN_WINDOW_LABEL {
                return;
            }

            if let WindowEvent::CloseRequested { api, .. } = event {
                let state = window.state::<AppState>();
                if state.close_policy == WindowClosePolicy::HideToBackground {
                    api.prevent_close();
                    if let Err(error) = hide_palette_window(window, &state) {
                        tracing::warn!(%error, "failed to hide palette on close request");
                    }
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            palette_snapshot,
            search_palette,
            move_selection,
            execute_selected,
            execute_confirmed_shell,
            cancel_shell_confirmation,
            settings_snapshot,
            record_current_clipboard,
            clear_clipboard_history,
            set_provider_enabled,
            set_clipboard_capture,
            reload_config,
            reset_palette,
            close_palette_window
        ])
        .build(tauri::generate_context!())
        .map_err(|error| anyhow::anyhow!("failed to build freepalette UI: {error}"))?;

    let mut tick_thread_started = false;
    app.run(move |app_handle, event| match event {
        RunEvent::Ready if lifecycle.is_active() && !tick_thread_started => {
            start_lifecycle_tick_thread(app_handle.clone());
            tick_thread_started = true;
        }
        RunEvent::MainEventsCleared => poll_lifecycle(app_handle, &mut lifecycle),
        _ => {}
    });

    Ok(())
}

#[tauri::command]
fn palette_snapshot(state: State<'_, AppState>) -> Result<PaletteSnapshot, String> {
    let palette = lock_palette(&state)?;
    Ok(snapshot(&palette))
}

#[tauri::command]
fn search_palette(query: String, state: State<'_, AppState>) -> Result<PaletteSnapshot, String> {
    let mut palette = lock_palette(&state)?;
    palette.set_query(query);
    Ok(snapshot(&palette))
}

#[tauri::command]
fn move_selection(
    direction: String,
    state: State<'_, AppState>,
) -> Result<PaletteSnapshot, String> {
    let direction = selection_direction(&direction)?;
    let mut palette = lock_palette(&state)?;
    palette.move_selection(direction);
    Ok(snapshot(&palette))
}

#[tauri::command]
fn execute_selected(state: State<'_, AppState>) -> Result<ExecutionSnapshot, String> {
    let mut palette = lock_palette(&state)?;
    let clipboard_text = selected_clipboard_text(&palette);
    let mut execution = palette.execute_selected();
    if let (PaletteExecution::Completed { .. }, Some(text)) = (&execution, clipboard_text) {
        match write_system_clipboard_text(&text) {
            Ok(()) => palette.set_status_info(format!(
                "Copied {} bytes from clipboard history",
                text.len()
            )),
            Err(error) => {
                palette.set_status_error(error);
                execution = PaletteExecution::Failed;
            }
        }
    }
    let execution = execution_state(execution);
    Ok(ExecutionSnapshot {
        execution,
        palette: snapshot(&palette),
    })
}

#[tauri::command]
fn execute_confirmed_shell(state: State<'_, AppState>) -> Result<ExecutionSnapshot, String> {
    let mut palette = lock_palette(&state)?;
    let execution = execution_state(palette.execute_confirmed_shell());
    Ok(ExecutionSnapshot {
        execution,
        palette: snapshot(&palette),
    })
}

#[tauri::command]
fn cancel_shell_confirmation(state: State<'_, AppState>) -> Result<PaletteSnapshot, String> {
    let mut palette = lock_palette(&state)?;
    palette.cancel_shell_confirmation();
    Ok(snapshot(&palette))
}

#[tauri::command]
fn settings_snapshot(state: State<'_, AppState>) -> Result<SettingsSnapshot, String> {
    let palette = lock_palette(&state)?;
    Ok(settings(&palette))
}

#[tauri::command]
fn record_current_clipboard(state: State<'_, AppState>) -> Result<PaletteSnapshot, String> {
    let text = read_system_clipboard_text()?;
    let mut palette = lock_palette(&state)?;
    palette
        .record_clipboard_text(text)
        .map_err(|error| error.to_string())?;
    Ok(snapshot(&palette))
}

#[tauri::command]
fn clear_clipboard_history(state: State<'_, AppState>) -> Result<PaletteSnapshot, String> {
    let mut palette = lock_palette(&state)?;
    palette
        .clear_clipboard_history()
        .map_err(|error| error.to_string())?;
    Ok(snapshot(&palette))
}

#[tauri::command]
fn set_provider_enabled(
    provider_id: String,
    enabled: bool,
    state: State<'_, AppState>,
) -> Result<PaletteSnapshot, String> {
    let mut palette = lock_palette(&state)?;
    palette
        .set_provider_enabled(&provider_id, enabled)
        .map_err(|error| error.to_string())?;
    Ok(snapshot(&palette))
}

#[tauri::command]
fn set_clipboard_capture(
    enabled: bool,
    state: State<'_, AppState>,
) -> Result<PaletteSnapshot, String> {
    let mut palette = lock_palette(&state)?;
    palette
        .set_clipboard_capture_enabled(enabled)
        .map_err(|error| error.to_string())?;
    Ok(snapshot(&palette))
}

#[tauri::command]
fn reload_config(state: State<'_, AppState>) -> Result<PaletteSnapshot, String> {
    let mut palette = lock_palette(&state)?;
    palette.reload_config();
    Ok(snapshot(&palette))
}

#[tauri::command]
fn reset_palette(state: State<'_, AppState>) -> Result<PaletteSnapshot, String> {
    let mut palette = lock_palette(&state)?;
    palette.reset_for_next_activation();
    Ok(snapshot(&palette))
}

#[tauri::command]
fn close_palette_window(window: Window, state: State<'_, AppState>) -> Result<(), String> {
    match state.close_policy {
        WindowClosePolicy::CloseProcess => window.close().map_err(|error| error.to_string()),
        WindowClosePolicy::HideToBackground => hide_palette_window(&window, &state),
    }
}

fn lock_palette(state: &AppState) -> Result<MutexGuard<'_, PaletteState>, String> {
    state
        .palette
        .lock()
        .map_err(|_| "palette state lock was poisoned".to_string())
}

fn snapshot(palette: &PaletteState) -> PaletteSnapshot {
    PaletteSnapshot {
        query: palette.query().to_string(),
        results: palette.results().to_vec(),
        selected_index: palette.selected_index(),
        status: status_snapshot(palette.status()),
    }
}

fn settings(palette: &PaletteState) -> SettingsSnapshot {
    let config = palette.config();
    SettingsSnapshot {
        provider_ids: palette.provider_ids(),
        providers: ProviderSettingsSnapshot {
            apps: config.providers.apps,
            calculator: config.providers.calculator,
            shell: config.providers.shell,
            clipboard: config.providers.clipboard,
        },
        clipboard_capture_enabled: palette.clipboard_capture_enabled(),
        clipboard_history_len: palette.clipboard_history_len(),
        recent_result_count: palette.recent_result_count(),
        hotkey_summary: palette.hotkey_summary(),
        local_state_path: palette.local_state_path(),
        daemon_connection: palette.daemon_connection_summary(),
    }
}

fn read_system_clipboard_text() -> Result<String, String> {
    let mut clipboard = arboard::Clipboard::new()
        .map_err(|error| format!("failed to open system clipboard: {error}"))?;
    clipboard
        .get_text()
        .map_err(|error| format!("failed to read text from system clipboard: {error}"))
}

fn write_system_clipboard_text(text: &str) -> Result<(), String> {
    let mut clipboard = arboard::Clipboard::new()
        .map_err(|error| format!("failed to open system clipboard: {error}"))?;
    clipboard
        .set_text(text.to_string())
        .map_err(|error| format!("failed to write text to system clipboard: {error}"))
}

fn selected_clipboard_text(palette: &PaletteState) -> Option<String> {
    let selected = palette.selected_index()?;
    let result = &palette.results().get(selected)?.result;
    if result.kind != ResultKind::Clipboard {
        return None;
    }

    let Action::CopyText { text } = result.primary_action() else {
        return None;
    };

    Some(text.clone())
}

fn status_snapshot(status: &PaletteStatus) -> StatusSnapshot {
    match status {
        PaletteStatus::Ready => StatusSnapshot::Ready,
        PaletteStatus::Info(message) => StatusSnapshot::Info {
            message: message.clone(),
        },
        PaletteStatus::Error(message) => StatusSnapshot::Error {
            message: message.clone(),
        },
    }
}

fn selection_direction(direction: &str) -> Result<SelectionDirection, String> {
    match direction {
        "previous" => Ok(SelectionDirection::Previous),
        "next" => Ok(SelectionDirection::Next),
        _ => Err(format!("unknown selection direction: {direction}")),
    }
}

fn execution_state(execution: PaletteExecution) -> ExecutionState {
    match execution {
        PaletteExecution::NoSelection => ExecutionState::NoSelection,
        PaletteExecution::SelectedUnavailable => ExecutionState::SelectedUnavailable,
        PaletteExecution::Blocked => ExecutionState::Blocked,
        PaletteExecution::NeedsShellConfirmation { command } => {
            ExecutionState::NeedsShellConfirmation { command }
        }
        PaletteExecution::Completed { hide_palette } => ExecutionState::Completed { hide_palette },
        PaletteExecution::Failed => ExecutionState::Failed,
    }
}

fn start_lifecycle_tick_thread(app_handle: AppHandle) {
    // The hotkey and tray handles stay on Tauri's main event loop because the
    // Windows tray types are not Send. This helper only wakes that loop.
    let _tick_thread = std::thread::spawn(move || loop {
        if app_handle.run_on_main_thread(|| {}).is_err() {
            break;
        }

        std::thread::sleep(LIFECYCLE_POLL_INTERVAL);
    });
}

fn poll_lifecycle(app_handle: &AppHandle, lifecycle: &mut UiLifecycle) {
    if lifecycle.hotkey_bridge.take_activation_request() {
        show_palette_from_lifecycle(app_handle);
    }

    if let Some(command) = lifecycle.tray.as_ref().and_then(UiTray::poll_command) {
        handle_lifecycle_command(app_handle, lifecycle.tray.as_ref(), command);
    }

    poll_clipboard_from_lifecycle(app_handle, lifecycle);
}

fn poll_clipboard_from_lifecycle(app_handle: &AppHandle, lifecycle: &mut UiLifecycle) {
    if Instant::now() < lifecycle.next_clipboard_poll {
        return;
    }
    lifecycle.next_clipboard_poll = Instant::now() + CLIPBOARD_POLL_INTERVAL;

    let state = app_handle.state::<AppState>();
    let capture_enabled = match lock_palette(&state) {
        Ok(palette) => palette.clipboard_capture_enabled(),
        Err(error) => {
            tracing::warn!(%error, "failed to read clipboard capture state");
            return;
        }
    };
    if !capture_enabled {
        return;
    }

    let text = match read_system_clipboard_text() {
        Ok(text) => text,
        Err(error) => {
            tracing::debug!(%error, "background clipboard read failed");
            return;
        }
    };

    match lock_palette(&state) {
        Ok(mut palette) => {
            if let Err(error) = palette.record_clipboard_text_in_background(text) {
                tracing::warn!(%error, "background clipboard record failed");
                return;
            }
            emit_palette_update(app_handle);
        }
        Err(error) => tracing::warn!(%error, "failed to update palette from clipboard poll"),
    };
}

fn handle_lifecycle_command(app_handle: &AppHandle, tray: Option<&UiTray>, command: TrayCommand) {
    match command {
        TrayCommand::Show => show_palette_from_lifecycle(app_handle),
        TrayCommand::Hide => hide_palette_from_lifecycle(app_handle),
        TrayCommand::ReloadConfig => reload_config_from_lifecycle(app_handle),
        TrayCommand::EnableAutostart => enable_autostart_from_lifecycle(app_handle, tray),
        TrayCommand::DisableAutostart => disable_autostart_from_lifecycle(app_handle, tray),
        TrayCommand::Quit => app_handle.exit(0),
    }
}

fn show_palette_from_lifecycle(app_handle: &AppHandle) {
    if let Err(error) = show_palette_window(app_handle) {
        tracing::warn!(%error, "failed to show palette from UI lifecycle");
    }
}

fn hide_palette_from_lifecycle(app_handle: &AppHandle) {
    if let Err(error) = hide_main_palette_window(app_handle) {
        tracing::warn!(%error, "failed to hide palette from UI lifecycle");
    }
}

fn reload_config_from_lifecycle(app_handle: &AppHandle) {
    let state = app_handle.state::<AppState>();
    match lock_palette(&state) {
        Ok(mut palette) => {
            palette.reload_config();
            emit_palette_update(app_handle);
        }
        Err(error) => tracing::warn!(%error, "failed to reload config from UI lifecycle"),
    };
}

fn enable_autostart_from_lifecycle(app_handle: &AppHandle, tray: Option<&UiTray>) {
    match UiAutostart::enable() {
        Ok(shortcut) => {
            set_lifecycle_status(
                app_handle,
                PaletteStatusUpdate::Info(format!(
                    "Launch at sign-in enabled: {}",
                    shortcut.display()
                )),
            );
            refresh_tray_autostart_menu(tray);
        }
        Err(error) => {
            set_lifecycle_status(
                app_handle,
                PaletteStatusUpdate::Error(format!("Could not enable launch at sign-in: {error}")),
            );
        }
    }
}

fn disable_autostart_from_lifecycle(app_handle: &AppHandle, tray: Option<&UiTray>) {
    match UiAutostart::disable() {
        Ok(shortcut) => {
            set_lifecycle_status(
                app_handle,
                PaletteStatusUpdate::Info(format!(
                    "Launch at sign-in disabled: {}",
                    shortcut.display()
                )),
            );
            refresh_tray_autostart_menu(tray);
        }
        Err(error) => {
            set_lifecycle_status(
                app_handle,
                PaletteStatusUpdate::Error(format!("Could not disable launch at sign-in: {error}")),
            );
        }
    }
}

enum PaletteStatusUpdate {
    Info(String),
    Error(String),
}

fn set_lifecycle_status(app_handle: &AppHandle, update: PaletteStatusUpdate) {
    let state = app_handle.state::<AppState>();
    match lock_palette(&state) {
        Ok(mut palette) => {
            match update {
                PaletteStatusUpdate::Info(message) => palette.set_status_info(message),
                PaletteStatusUpdate::Error(message) => palette.set_status_error(message),
            }
            emit_palette_update(app_handle);
        }
        Err(error) => tracing::warn!(%error, "failed to update palette status from UI lifecycle"),
    };
}

fn refresh_tray_autostart_menu(tray: Option<&UiTray>) {
    if let Some(tray) = tray {
        tray.refresh_autostart_menu();
    }
}

fn show_palette_window(app_handle: &AppHandle) -> Result<(), String> {
    let window = main_window(app_handle)?;
    window.show().map_err(|error| error.to_string())?;
    window.unminimize().map_err(|error| error.to_string())?;
    window.set_focus().map_err(|error| error.to_string())?;
    emit_palette_shown(app_handle);
    Ok(())
}

fn hide_main_palette_window(app_handle: &AppHandle) -> Result<(), String> {
    let window = main_window(app_handle)?;
    let state = app_handle.state::<AppState>();
    reset_palette_for_hide(&state)?;
    window.hide().map_err(|error| error.to_string())?;
    emit_palette_update(&window);
    Ok(())
}

fn hide_palette_window(window: &Window, state: &AppState) -> Result<(), String> {
    reset_palette_for_hide(state)?;
    window.hide().map_err(|error| error.to_string())?;
    emit_palette_update(window);
    Ok(())
}

fn reset_palette_for_hide(state: &AppState) -> Result<(), String> {
    let mut palette = lock_palette(state)?;
    palette.reset_for_next_activation();
    Ok(())
}

fn main_window(app_handle: &AppHandle) -> Result<WebviewWindow, String> {
    app_handle
        .get_webview_window(MAIN_WINDOW_LABEL)
        .ok_or_else(|| "main palette window is unavailable".to_string())
}

fn emit_palette_update<R>(emitter: &R)
where
    R: Emitter<tauri::Wry>,
{
    if let Err(error) = emitter.emit(PALETTE_UPDATED_EVENT, ()) {
        tracing::warn!(%error, "failed to emit palette update event");
    }
}

fn emit_palette_shown<R>(emitter: &R)
where
    R: Emitter<tauri::Wry>,
{
    if let Err(error) = emitter.emit(PALETTE_SHOWN_EVENT, ()) {
        tracing::warn!(%error, "failed to emit palette shown event");
    }
}

#[cfg(windows)]
fn create_tray(state: &mut PaletteState) -> Option<UiTray> {
    match UiTray::new() {
        Ok(tray) => Some(tray),
        Err(error) => {
            let message = format!("Tray unavailable: {error}");
            tracing::warn!("{message}");
            state.set_status_error(message);
            None
        }
    }
}

#[cfg(not(windows))]
fn create_tray(_state: &mut PaletteState) -> Option<UiTray> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    use freepalette_core::{ClipboardConfig, Config, ProviderConfig};

    #[test]
    fn close_policy_exits_when_no_background_lifecycle_exists() {
        assert_eq!(
            WindowClosePolicy::from_lifecycle(false, false),
            WindowClosePolicy::CloseProcess
        );
    }

    #[test]
    fn close_policy_hides_when_hotkey_lifecycle_exists() {
        assert_eq!(
            WindowClosePolicy::from_lifecycle(true, false),
            WindowClosePolicy::HideToBackground
        );
    }

    #[test]
    fn close_policy_hides_when_tray_lifecycle_exists() {
        assert_eq!(
            WindowClosePolicy::from_lifecycle(false, true),
            WindowClosePolicy::HideToBackground
        );
    }

    #[test]
    fn close_policy_hides_when_multiple_background_lifecycles_exist() {
        assert_eq!(
            WindowClosePolicy::from_lifecycle(true, true),
            WindowClosePolicy::HideToBackground
        );
    }

    #[test]
    fn lifecycle_without_hotkey_or_tray_is_inactive() {
        let lifecycle = UiLifecycle::new(UiHotkeyBridge::disabled(), None);

        assert!(!lifecycle.is_active());
    }

    #[test]
    fn selected_clipboard_text_reads_only_clipboard_results() {
        let mut palette = PaletteState::from_config(Config {
            providers: ProviderConfig {
                apps: false,
                calculator: true,
                shell: false,
                clipboard: true,
            },
            clipboard: ClipboardConfig {
                capture: true,
                max_entries: 10,
                max_entry_bytes: 4096,
            },
            ..Default::default()
        })
        .expect("test palette should register providers");

        palette
            .record_clipboard_text("private clipboard value")
            .expect("test clipboard text should be stored");
        palette.set_query("private");

        assert_eq!(
            selected_clipboard_text(&palette),
            Some("private clipboard value".to_string())
        );

        palette.set_query("calc 2+2");

        assert_eq!(selected_clipboard_text(&palette), None);
    }
}
