use std::path::PathBuf;

use freepalette_plugin_api::PluginError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("failed to read config at {path}: {source}")]
    ConfigRead {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse config at {path}: {source}")]
    ConfigParse {
        path: PathBuf,
        #[source]
        source: Box<toml::de::Error>,
    },
    #[error("provider '{0}' is already registered")]
    ProviderAlreadyRegistered(String),
    #[error("provider '{0}' is not registered")]
    ProviderNotFound(String),
    #[error("provider '{provider}' failed: {source}")]
    ProviderFailed {
        provider: String,
        #[source]
        source: PluginError,
    },
}
