mod app;
mod calculator;
mod clipboard;
mod shell;

pub use app::AppLauncherProvider;
pub use calculator::CalculatorProvider;
pub use clipboard::ClipboardHistoryProvider;
pub use shell::ShellCommandProvider;

use crate::{config::Config, error::CoreError, registry::ProviderRegistry};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuiltinProviderSet {
    pub apps: bool,
    pub calculator: bool,
    pub shell: bool,
    pub clipboard: bool,
}

pub fn builtin_registry(config: &Config) -> Result<ProviderRegistry, CoreError> {
    let mut registry = ProviderRegistry::new();

    if config.providers.apps {
        registry.register(AppLauncherProvider::from_config(config))?;
    }
    if config.providers.calculator {
        registry.register(CalculatorProvider)?;
    }
    if config.providers.shell {
        registry.register(ShellCommandProvider)?;
    }
    if config.providers.clipboard {
        registry.register(ClipboardHistoryProvider::empty())?;
    }

    Ok(registry)
}
