//! Public provider and action types for freepalette.
//!
//! This crate is intentionally data-oriented. Built-in providers use the Rust
//! trait in this crate, while future external plugins should communicate with
//! serialized request and response messages rather than relying on Rust ABI
//! stability.

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProviderId(String);

impl ProviderId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for ProviderId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl std::fmt::Display for ProviderId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SearchQuery(String);

impl SearchQuery {
    pub fn new(raw: impl Into<String>) -> Self {
        Self(raw.into())
    }

    pub fn raw(&self) -> &str {
        &self.0
    }

    pub fn trimmed(&self) -> &str {
        self.0.trim()
    }

    pub fn is_empty(&self) -> bool {
        self.trimmed().is_empty()
    }
}

impl From<&str> for SearchQuery {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl std::fmt::Display for SearchQuery {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchContext {
    pub query: SearchQuery,
    pub limit: usize,
}

impl SearchContext {
    pub fn new(query: impl Into<String>, limit: usize) -> Self {
        Self {
            query: SearchQuery::new(query),
            limit,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ResultKind {
    App,
    Calculator,
    Shell,
    Clipboard,
    Plugin,
    System,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum Action {
    /// Spawn an executable or command directly with optional arguments.
    LaunchApp { command: String, args: Vec<String> },
    /// Open a local path through the platform shell/default application.
    OpenPath { path: String },
    /// Run a shell command after explicit caller approval.
    RunShell { command: String },
    /// Make text available to copy.
    CopyText { text: String },
    /// Return a message without taking an external action.
    Noop { message: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionDescriptor {
    pub id: String,
    pub label: String,
    pub action: Action,
    pub primary: bool,
}

impl ActionDescriptor {
    pub fn primary(label: impl Into<String>, action: Action) -> Self {
        Self {
            id: "primary".to_string(),
            label: label.into(),
            action,
            primary: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchResult {
    pub id: String,
    pub provider: ProviderId,
    pub title: String,
    pub subtitle: Option<String>,
    pub kind: ResultKind,
    pub action: Action,
    #[serde(default)]
    pub actions: Vec<ActionDescriptor>,
    pub keywords: Vec<String>,
    pub score_hint: i64,
}

impl SearchResult {
    pub fn new(
        provider: ProviderId,
        id: impl Into<String>,
        title: impl Into<String>,
        kind: ResultKind,
        action: Action,
    ) -> Self {
        let label = primary_action_label(&action).to_string();
        Self {
            id: id.into(),
            provider,
            title: title.into(),
            subtitle: None,
            kind,
            action: action.clone(),
            actions: vec![ActionDescriptor::primary(label, action)],
            keywords: Vec::new(),
            score_hint: 0,
        }
    }

    pub fn primary_action(&self) -> &Action {
        self.actions
            .iter()
            .find(|descriptor| descriptor.primary)
            .map(|descriptor| &descriptor.action)
            .unwrap_or(&self.action)
    }

    pub fn with_subtitle(mut self, subtitle: impl Into<String>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    pub fn with_keywords(mut self, keywords: Vec<String>) -> Self {
        self.keywords = keywords;
        self
    }

    pub fn with_score_hint(mut self, score_hint: i64) -> Self {
        self.score_hint = score_hint;
        self
    }
}

fn primary_action_label(action: &Action) -> &'static str {
    match action {
        Action::LaunchApp { .. } => "Launch",
        Action::OpenPath { .. } => "Open",
        Action::RunShell { .. } => "Run",
        Action::CopyText { .. } => "Copy",
        Action::Noop { .. } => "Show",
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionOutcome {
    pub message: String,
}

impl ActionOutcome {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

#[derive(Debug, Error)]
pub enum PluginError {
    #[error("action failed: {0}")]
    Action(String),
    #[error("invalid query: {0}")]
    InvalidQuery(String),
    #[error("provider failed: {0}")]
    Provider(String),
    #[error("provider does not support this action")]
    UnsupportedAction,
}

pub trait Provider: Send + Sync {
    fn id(&self) -> ProviderId;

    fn search(&self, context: &SearchContext) -> Result<Vec<SearchResult>, PluginError>;

    fn execute(&self, action: &Action) -> Result<ActionOutcome, PluginError> {
        match action {
            Action::Noop { message } => Ok(ActionOutcome::new(message.clone())),
            _ => Err(PluginError::UnsupportedAction),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_result_keeps_action_field_as_primary_action() {
        let action = Action::CopyText {
            text: "4".to_string(),
        };

        let result = SearchResult::new(
            ProviderId::from("calculator"),
            "calc",
            "2+2 = 4",
            ResultKind::Calculator,
            action.clone(),
        );

        assert_eq!(result.action, action);
        assert_eq!(result.primary_action(), &action);
        assert_eq!(result.actions.len(), 1);
        assert_eq!(result.actions[0].id, "primary");
        assert_eq!(result.actions[0].label, "Copy");
        assert!(result.actions[0].primary);
    }

    #[test]
    fn primary_action_falls_back_to_action_field_when_descriptors_are_absent() {
        let result = SearchResult {
            id: "legacy".to_string(),
            provider: ProviderId::from("legacy"),
            title: "Legacy Result".to_string(),
            subtitle: None,
            kind: ResultKind::System,
            action: Action::Noop {
                message: "legacy action".to_string(),
            },
            actions: Vec::new(),
            keywords: Vec::new(),
            score_hint: 0,
        };

        assert!(matches!(
            result.primary_action(),
            Action::Noop { message } if message == "legacy action"
        ));
    }
}
