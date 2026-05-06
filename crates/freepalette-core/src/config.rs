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
            Some(path) if path.exists() => Self::load_from_path(&path),
            _ => Ok(Self::default()),
        }
    }

    pub fn default_path() -> Option<PathBuf> {
        ProjectDirs::from("org", "freepalette", "freepalette")
            .map(|dirs| dirs.config_dir().join("freepalette.toml"))
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
}
