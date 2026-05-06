//! Core search, ranking, config, and built-in provider plumbing for freepalette.

pub mod config;
pub mod error;
pub mod fuzzy;
pub mod providers;
pub mod ranking;
pub mod registry;

pub use config::{AppEntry, Config, GeneralConfig, ProviderConfig};
pub use error::CoreError;
pub use freepalette_plugin_api::{
    Action, ActionOutcome, Provider, ProviderId, Query, ResultKind, SearchContext, SearchResult,
};
pub use providers::{
    builtin_registry, AppIndexEntry, AppIndexEntrySource, AppIndexReport, AppIndexReportStatus,
    AppLauncherProvider, BuiltinProviderSet,
};
pub use ranking::{rank_results, RankedResult};
pub use registry::ProviderRegistry;

use std::path::Path;

pub fn registry_from_config_path(path: &Path) -> Result<ProviderRegistry, CoreError> {
    let config = Config::load_from_path(path)?;
    builtin_registry(&config)
}
