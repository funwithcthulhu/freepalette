#[cfg(any(windows, test))]
use std::path::Path;
use std::path::PathBuf;

use thiserror::Error;

#[cfg(any(windows, test))]
const STARTUP_SHORTCUT_NAME: &str = "freepalette.lnk";

#[derive(Debug, Error)]
pub enum UiAutostartError {
    #[error("UI autostart is only implemented on Windows")]
    UnsupportedPlatform,
    #[error("APPDATA is not set; cannot locate the per-user Windows Startup folder")]
    MissingRoamingAppData,
    #[error("failed to resolve current freepalette-ui executable: {0}")]
    CurrentExecutable(#[source] std::io::Error),
    #[error("current executable path has no parent directory: {path}")]
    ExecutableHasNoParent { path: PathBuf },
    #[error("autostart shortcut path has no parent directory: {path}")]
    ShortcutHasNoParent { path: PathBuf },
    #[error("failed to create Windows Startup folder at {path}: {source}")]
    CreateStartupFolder {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to remove autostart shortcut at {path}: {source}")]
    RemoveShortcut {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[cfg(windows)]
    #[error("failed to create autostart shortcut at {path}: {source}")]
    CreateShortcut {
        path: PathBuf,
        #[source]
        source: win_desktop_utils::Error,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UiAutostartStatus {
    Enabled { shortcut: PathBuf },
    Disabled { shortcut: PathBuf },
    Unsupported,
}

impl UiAutostartStatus {
    pub fn is_enabled(&self) -> bool {
        matches!(self, Self::Enabled { .. })
    }
}

pub struct UiAutostart;

impl UiAutostart {
    pub fn status() -> Result<UiAutostartStatus, UiAutostartError> {
        autostart_status()
    }

    pub fn enable() -> Result<PathBuf, UiAutostartError> {
        enable_autostart()
    }

    pub fn disable() -> Result<PathBuf, UiAutostartError> {
        disable_autostart()
    }
}

#[cfg(any(windows, test))]
fn startup_shortcut_path_from_roaming_app_data(roaming_app_data: &Path) -> PathBuf {
    roaming_app_data
        .join("Microsoft")
        .join("Windows")
        .join("Start Menu")
        .join("Programs")
        .join("Startup")
        .join(STARTUP_SHORTCUT_NAME)
}

#[cfg(any(windows, test))]
fn autostart_status_for_shortcut(shortcut: PathBuf) -> UiAutostartStatus {
    if shortcut.exists() {
        UiAutostartStatus::Enabled { shortcut }
    } else {
        UiAutostartStatus::Disabled { shortcut }
    }
}

#[cfg(windows)]
fn autostart_status() -> Result<UiAutostartStatus, UiAutostartError> {
    let shortcut = startup_shortcut_path()?;
    Ok(autostart_status_for_shortcut(shortcut))
}

#[cfg(not(windows))]
fn autostart_status() -> Result<UiAutostartStatus, UiAutostartError> {
    Ok(UiAutostartStatus::Unsupported)
}

#[cfg(windows)]
fn enable_autostart() -> Result<PathBuf, UiAutostartError> {
    let shortcut = startup_shortcut_path()?;
    let startup_dir = shortcut
        .parent()
        .ok_or_else(|| UiAutostartError::ShortcutHasNoParent {
            path: shortcut.clone(),
        })?;

    std::fs::create_dir_all(startup_dir).map_err(|source| {
        UiAutostartError::CreateStartupFolder {
            path: startup_dir.to_path_buf(),
            source,
        }
    })?;

    let executable = current_executable()?;
    let options = shortcut_options_for_executable(&executable)?;
    win_desktop_utils::create_shortcut(&shortcut, &executable, &options).map_err(|source| {
        UiAutostartError::CreateShortcut {
            path: shortcut.clone(),
            source,
        }
    })?;

    Ok(shortcut)
}

#[cfg(not(windows))]
fn enable_autostart() -> Result<PathBuf, UiAutostartError> {
    Err(UiAutostartError::UnsupportedPlatform)
}

#[cfg(windows)]
fn disable_autostart() -> Result<PathBuf, UiAutostartError> {
    let shortcut = startup_shortcut_path()?;
    if shortcut.exists() {
        std::fs::remove_file(&shortcut).map_err(|source| UiAutostartError::RemoveShortcut {
            path: shortcut.clone(),
            source,
        })?;
    }

    Ok(shortcut)
}

#[cfg(not(windows))]
fn disable_autostart() -> Result<PathBuf, UiAutostartError> {
    Err(UiAutostartError::UnsupportedPlatform)
}

#[cfg(windows)]
fn startup_shortcut_path() -> Result<PathBuf, UiAutostartError> {
    let roaming_app_data =
        std::env::var_os("APPDATA").ok_or(UiAutostartError::MissingRoamingAppData)?;
    Ok(startup_shortcut_path_from_roaming_app_data(Path::new(
        &roaming_app_data,
    )))
}

#[cfg(windows)]
fn current_executable() -> Result<PathBuf, UiAutostartError> {
    std::env::current_exe().map_err(UiAutostartError::CurrentExecutable)
}

#[cfg(windows)]
fn shortcut_options_for_executable(
    executable: &Path,
) -> Result<win_desktop_utils::ShortcutOptions, UiAutostartError> {
    let working_directory =
        executable
            .parent()
            .ok_or_else(|| UiAutostartError::ExecutableHasNoParent {
                path: executable.to_path_buf(),
            })?;

    Ok(win_desktop_utils::ShortcutOptions::new()
        .working_directory(working_directory)
        .icon(executable, 0)
        .description("Start freepalette UI at sign-in"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn startup_shortcut_path_uses_per_user_startup_folder() {
        let base = Path::new(r"C:\Users\Example\AppData\Roaming");
        let path = startup_shortcut_path_from_roaming_app_data(base);
        let expected_tail: PathBuf = [
            "Microsoft",
            "Windows",
            "Start Menu",
            "Programs",
            "Startup",
            STARTUP_SHORTCUT_NAME,
        ]
        .iter()
        .collect();

        assert!(path.ends_with(expected_tail));
    }

    #[test]
    fn missing_autostart_shortcut_reports_disabled() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after Unix epoch")
            .as_nanos();
        let shortcut = std::env::temp_dir().join(format!(
            "freepalette-missing-autostart-{}-{}.lnk",
            std::process::id(),
            unique
        ));
        let _ = std::fs::remove_file(&shortcut);

        let status = autostart_status_for_shortcut(shortcut.clone());

        assert_eq!(status, UiAutostartStatus::Disabled { shortcut });
    }
}
