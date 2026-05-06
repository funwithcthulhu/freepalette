use std::path::{Path, PathBuf};

use freepalette_core::{builtin_registry, Config, CoreError, ProviderRegistry, RankedResult};

pub struct DaemonState {
    config_path: Option<PathBuf>,
    config: Config,
    registry: ProviderRegistry,
    clipboard_history: Vec<String>,
}

impl DaemonState {
    pub fn new(config: Config) -> Result<Self, CoreError> {
        let registry = builtin_registry(&config)?;
        Ok(Self {
            config_path: None,
            config,
            registry,
            clipboard_history: Vec::new(),
        })
    }

    pub fn load_from_path(path: &Path) -> Result<Self, CoreError> {
        let config = Config::load_from_path(path)?;
        let registry = builtin_registry(&config)?;
        Ok(Self {
            config_path: Some(path.to_path_buf()),
            config,
            registry,
            clipboard_history: Vec::new(),
        })
    }

    pub fn reload_config(&mut self) -> Result<(), CoreError> {
        let Some(path) = &self.config_path else {
            return Ok(());
        };

        let config = Config::load_from_path(path)?;
        let registry = builtin_registry(&config)?;
        self.config = config;
        self.registry = registry;
        Ok(())
    }

    pub fn search(&self, query: &str) -> Result<Vec<RankedResult>, CoreError> {
        self.registry.search(query, self.config.general.max_results)
    }

    pub fn provider_ids(&self) -> Vec<String> {
        self.registry
            .provider_ids()
            .into_iter()
            .map(|id| id.to_string())
            .collect()
    }

    pub fn clipboard_history_len(&self) -> usize {
        self.clipboard_history.len()
    }
}
