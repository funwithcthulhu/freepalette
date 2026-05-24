use std::sync::{Mutex, MutexGuard};

use freepalette_core::RankedResult;
use freepalette_ui::{PaletteExecution, PaletteState, PaletteStatus, SelectionDirection};
use serde::Serialize;
use tauri::{State, Window};

struct AppState {
    palette: Mutex<PaletteState>,
}

impl AppState {
    fn new() -> Result<Self, freepalette_ui::UiError> {
        Ok(Self {
            palette: Mutex::new(PaletteState::from_default_config()?),
        })
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
    Completed { hide_palette: bool },
    Failed,
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_target(false).init();

    tauri::Builder::default()
        .manage(AppState::new()?)
        .invoke_handler(tauri::generate_handler![
            palette_snapshot,
            search_palette,
            move_selection,
            execute_selected,
            reload_config,
            reset_palette,
            close_palette_window
        ])
        .run(tauri::generate_context!())
        .map_err(|error| anyhow::anyhow!("failed to run freepalette UI: {error}"))?;

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
    let execution = execution_state(palette.execute_selected());
    Ok(ExecutionSnapshot {
        execution,
        palette: snapshot(&palette),
    })
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
fn close_palette_window(window: Window) -> Result<(), String> {
    window.close().map_err(|error| error.to_string())
}

fn lock_palette<'a>(
    state: &'a State<'_, AppState>,
) -> Result<MutexGuard<'a, PaletteState>, String> {
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
        PaletteExecution::Completed { hide_palette } => ExecutionState::Completed { hide_palette },
        PaletteExecution::Failed => ExecutionState::Failed,
    }
}
