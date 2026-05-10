use std::{
    fs,
    path::{Path, PathBuf},
};

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

use crate::error::CoreError;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub general: GeneralConfig,
    pub providers: ProviderConfig,
    pub clipboard: ClipboardConfig,
    pub hotkey: HotkeyConfig,
    pub apps: Vec<AppEntry>,
}

impl Config {
    pub fn load_from_path(path: &Path) -> Result<Self, CoreError> {
        let contents = fs::read_to_string(path).map_err(|source| CoreError::ConfigRead {
            path: path.to_path_buf(),
            source,
        })?;

        toml::from_str(&contents).map_err(|source| CoreError::ConfigParse {
            path: path.to_path_buf(),
            source: Box::new(source),
        })
    }

    pub fn load_default_or_default() -> Result<Self, CoreError> {
        match Self::default_path() {
            Some(path) => Self::load_path_or_default(&path),
            _ => Ok(Self::default()),
        }
    }

    pub fn default_path() -> Option<PathBuf> {
        ProjectDirs::from("org", "freepalette", "freepalette")
            .map(|dirs| dirs.config_dir().join("freepalette.toml"))
    }

    fn load_path_or_default(path: &Path) -> Result<Self, CoreError> {
        if path.exists() {
            Self::load_from_path(path)
        } else {
            Ok(Self::default())
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct GeneralConfig {
    pub max_results: usize,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self { max_results: 10 }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProviderConfig {
    pub apps: bool,
    pub calculator: bool,
    pub shell: bool,
    pub clipboard: bool,
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            apps: true,
            calculator: true,
            shell: true,
            clipboard: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ClipboardConfig {
    pub capture: bool,
    pub max_entries: usize,
    pub max_entry_bytes: usize,
}

impl Default for ClipboardConfig {
    fn default() -> Self {
        Self {
            capture: false,
            max_entries: 50,
            max_entry_bytes: 4096,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct HotkeyConfig {
    pub enabled: bool,
    pub key: String,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub meta: bool,
}

impl Default for HotkeyConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            key: "Space".to_string(),
            ctrl: true,
            alt: true,
            shift: false,
            meta: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppEntry {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub keywords: Vec<String>,
}

impl AppEntry {
    pub fn new(name: impl Into<String>, command: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            command: command.into(),
            args: Vec::new(),
            keywords: Vec::new(),
        }
    }
}

impl Default for AppEntry {
    fn default() -> Self {
        Self::new("", "")
    }
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    #[test]
    fn parses_config_with_apps() {
        let input = r#"
            [general]
            max_results = 7

            [providers]
            shell = false

            [[apps]]
            name = "Editor"
            command = "editor"
            args = ["--new-window"]
            keywords = ["text"]
        "#;

        let config: Config = toml::from_str(input).expect("valid test config");

        assert_eq!(config.general.max_results, 7);
        assert!(config.providers.apps);
        assert!(!config.providers.shell);
        assert_eq!(config.apps.len(), 1);
        assert_eq!(config.apps[0].name, "Editor");
        assert_eq!(config.apps[0].args, ["--new-window"]);
    }

    #[test]
    fn missing_sections_use_defaults() {
        let config: Config = toml::from_str("").expect("empty config should use defaults");

        assert_eq!(config.general.max_results, 10);
        assert!(config.providers.apps);
        assert!(config.providers.calculator);
        assert!(config.providers.shell);
        assert!(config.providers.clipboard);
        assert!(!config.clipboard.capture);
        assert_eq!(config.clipboard.max_entries, 50);
        assert_eq!(config.clipboard.max_entry_bytes, 4096);
        assert!(!config.hotkey.enabled);
        assert_eq!(config.hotkey.key, "Space");
        assert!(config.apps.is_empty());
    }

    #[test]
    fn loads_config_from_path() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after Unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("freepalette-config-{unique}.toml"));
        fs::write(
            &path,
            r#"
                [general]
                max_results = 3
            "#,
        )
        .expect("test config should be writable");

        let config = Config::load_from_path(&path).expect("config should load from temp path");
        fs::remove_file(&path).expect("test config should be removable");

        assert_eq!(config.general.max_results, 3);
    }

    #[test]
    fn missing_config_path_uses_defaults() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after Unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("freepalette-missing-{unique}.toml"));

        let config =
            Config::load_path_or_default(&path).expect("missing config path should use defaults");

        assert_eq!(config, Config::default());
    }

    #[test]
    fn minimal_valid_config_uses_defaults_for_missing_fields() {
        let input = r#"
            [providers]
            apps = false
        "#;

        let config: Config = toml::from_str(input).expect("minimal config should parse");

        assert_eq!(config.general.max_results, 10);
        assert!(!config.providers.apps);
        assert!(config.providers.calculator);
        assert!(config.providers.shell);
        assert!(config.providers.clipboard);
        assert!(!config.clipboard.capture);
        assert_eq!(config.clipboard.max_entries, 50);
        assert_eq!(config.hotkey.key, "Space");
        assert!(config.apps.is_empty());
    }

    #[test]
    fn parses_clipboard_and_hotkey_config() {
        let input = r#"
            [clipboard]
            capture = true
            max_entries = 12
            max_entry_bytes = 128

            [hotkey]
            enabled = true
            key = "K"
            ctrl = true
            alt = false
            shift = true
            meta = false
        "#;

        let config: Config = toml::from_str(input).expect("clipboard config should parse");

        assert!(config.clipboard.capture);
        assert_eq!(config.clipboard.max_entries, 12);
        assert_eq!(config.clipboard.max_entry_bytes, 128);
        assert!(config.hotkey.enabled);
        assert_eq!(config.hotkey.key, "K");
        assert!(config.hotkey.ctrl);
        assert!(!config.hotkey.alt);
        assert!(config.hotkey.shift);
        assert!(!config.hotkey.meta);
    }

    #[test]
    fn invalid_toml_reports_config_parse_error() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after Unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("freepalette-invalid-{unique}.toml"));
        fs::write(&path, "[providers\nshell = true")
            .expect("invalid test config should be writable");

        let error = Config::load_from_path(&path).expect_err("invalid TOML should fail to parse");
        fs::remove_file(&path).expect("test config should be removable");

        assert!(matches!(error, CoreError::ConfigParse { .. }));
        assert!(
            error.to_string().contains("failed to parse config at"),
            "parse error should include config path context"
        );
    }

    #[test]
    fn unknown_config_keys_are_ignored() {
        let input = r#"
            unknown_root_key = "kept out of the typed config"

            [providers]
            calculator = false
            unknown_provider_key = true
        "#;

        let config: Config = toml::from_str(input).expect("unknown keys are currently ignored");

        assert!(config.providers.apps);
        assert!(!config.providers.calculator);
        assert!(config.providers.shell);
        assert!(config.providers.clipboard);
    }
}
