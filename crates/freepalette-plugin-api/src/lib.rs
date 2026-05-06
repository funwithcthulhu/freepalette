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
pub struct Query(String);

impl Query {
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

impl From<&str> for Query {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl std::fmt::Display for Query {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchContext {
    pub query: Query,
    pub limit: usize,
}

impl SearchContext {
    pub fn new(query: impl Into<String>, limit: usize) -> Self {
        Self {
            query: Query::new(query),
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
    LaunchApp { command: String, args: Vec<String> },
    RunShell { command: String },
    CopyText { text: String },
    Noop { message: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchResult {
    pub id: String,
    pub provider: ProviderId,
    pub title: String,
    pub subtitle: Option<String>,
    pub kind: ResultKind,
    pub action: Action,
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
        Self {
            id: id.into(),
            provider,
            title: title.into(),
            subtitle: None,
            kind,
            action,
            keywords: Vec::new(),
            score_hint: 0,
        }
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
