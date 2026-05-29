use freepalette_plugin_api::{
    Action, ActionOutcome, Provider, ProviderId, SearchContext, SearchResult,
};
use tracing::debug;

use crate::{error::CoreError, ranking, RankedResult};

pub struct ProviderRegistry {
    providers: Vec<Box<dyn Provider>>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
        }
    }

    pub fn register<P>(&mut self, provider: P) -> Result<(), CoreError>
    where
        P: Provider + 'static,
    {
        let id = provider.id();
        if self.contains(id.as_str()) {
            return Err(CoreError::ProviderAlreadyRegistered(id.to_string()));
        }

        debug!(provider = %id, "registered provider");
        self.providers.push(Box::new(provider));
        Ok(())
    }

    pub fn contains(&self, provider_id: &str) -> bool {
        self.providers
            .iter()
            .any(|provider| provider.id().as_str() == provider_id)
    }

    pub fn provider_ids(&self) -> Vec<ProviderId> {
        self.providers
            .iter()
            .map(|provider| provider.id())
            .collect()
    }

    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<RankedResult>, CoreError> {
        let context = SearchContext::new(query, limit);
        let mut results = Vec::<SearchResult>::new();

        for provider in &self.providers {
            let provider_id = provider.id();
            let provider_results =
                provider
                    .search(&context)
                    .map_err(|source| CoreError::ProviderFailed {
                        provider: provider_id.to_string(),
                        source,
                    })?;
            results.extend(provider_results);
        }

        let mut ranked = ranking::rank_results(query, results);
        ranked.truncate(limit);
        Ok(ranked)
    }

    pub fn execute(&self, result: &SearchResult) -> Result<ActionOutcome, CoreError> {
        self.execute_action(&result.provider, &result.action)
    }

    pub fn execute_action(
        &self,
        provider_id: &ProviderId,
        action: &Action,
    ) -> Result<ActionOutcome, CoreError> {
        let provider = self
            .providers
            .iter()
            .find(|provider| provider.id().as_str() == provider_id.as_str())
            .ok_or_else(|| CoreError::ProviderNotFound(provider_id.to_string()))?;

        provider
            .execute(action)
            .map_err(|source| CoreError::ProviderFailed {
                provider: provider_id.to_string(),
                source,
            })
    }
}

impl Default for ProviderRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };

    use freepalette_plugin_api::{Action, PluginError, ResultKind};

    use super::*;

    struct StaticProvider;

    impl Provider for StaticProvider {
        fn id(&self) -> ProviderId {
            ProviderId::from("static")
        }

        fn search(&self, _context: &SearchContext) -> Result<Vec<SearchResult>, PluginError> {
            Ok(vec![SearchResult::new(
                self.id(),
                "notepad",
                "Notepad",
                ResultKind::App,
                Action::Noop {
                    message: "noop".to_string(),
                },
            )])
        }
    }

    #[test]
    fn registers_and_searches_provider() {
        let mut registry = ProviderRegistry::new();
        registry
            .register(StaticProvider)
            .expect("test provider should register");

        assert!(registry.contains("static"));

        let results = registry.search("note", 5).expect("search should succeed");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].result.title, "Notepad");
    }

    #[test]
    fn rejects_duplicate_provider_ids() {
        let mut registry = ProviderRegistry::new();
        registry
            .register(StaticProvider)
            .expect("first provider should register");

        let error = registry
            .register(StaticProvider)
            .expect_err("duplicate provider should be rejected");

        assert!(matches!(error, CoreError::ProviderAlreadyRegistered(_)));
    }

    struct ShellPreviewProvider {
        executions: Arc<AtomicUsize>,
    }

    impl Provider for ShellPreviewProvider {
        fn id(&self) -> ProviderId {
            ProviderId::from("shell-preview")
        }

        fn search(&self, context: &SearchContext) -> Result<Vec<SearchResult>, PluginError> {
            let command = context
                .query
                .raw()
                .trim_start()
                .strip_prefix('>')
                .map(str::trim)
                .unwrap_or_default();
            Ok(vec![SearchResult::new(
                self.id(),
                "shell-preview",
                format!("Run: {command}"),
                ResultKind::Shell,
                Action::RunShell {
                    command: command.to_string(),
                },
            )
            .with_keywords(vec![
                ">".to_string(),
                "shell".to_string(),
                "command".to_string(),
            ])
            .with_score_hint(900)])
        }

        fn execute(&self, _action: &Action) -> Result<ActionOutcome, PluginError> {
            self.executions.fetch_add(1, Ordering::SeqCst);
            Ok(ActionOutcome::new("fake shell executed"))
        }
    }

    #[test]
    fn search_does_not_execute_shell_actions() {
        let executions = Arc::new(AtomicUsize::new(0));
        let mut registry = ProviderRegistry::new();
        registry
            .register(ShellPreviewProvider {
                executions: Arc::clone(&executions),
            })
            .expect("fake shell provider should register");

        let results = registry
            .search("> echo hello", 10)
            .expect("shell-looking search should succeed");
        let result = results
            .first()
            .expect("shell-looking search should return a preview result");

        assert!(matches!(
            result.result.action,
            Action::RunShell { ref command } if command == "echo hello"
        ));
        assert_eq!(executions.load(Ordering::SeqCst), 0);
    }
}
