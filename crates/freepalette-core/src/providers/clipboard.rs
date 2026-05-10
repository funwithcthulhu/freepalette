use freepalette_plugin_api::{
    Action, ActionOutcome, PluginError, Provider, ProviderId, ResultKind, SearchContext,
    SearchResult,
};

pub struct ClipboardHistoryProvider {
    items: Vec<String>,
}

impl ClipboardHistoryProvider {
    pub fn empty() -> Self {
        Self { items: Vec::new() }
    }

    pub fn with_items(items: Vec<String>) -> Self {
        Self { items }
    }
}

impl Provider for ClipboardHistoryProvider {
    fn id(&self) -> ProviderId {
        ProviderId::from("clipboard")
    }

    fn search(&self, _context: &SearchContext) -> Result<Vec<SearchResult>, PluginError> {
        Ok(self
            .items
            .iter()
            .enumerate()
            .map(|(index, item)| {
                SearchResult::new(
                    self.id(),
                    format!("clipboard:{index}"),
                    preview(item),
                    ResultKind::Clipboard,
                    Action::CopyText { text: item.clone() },
                )
                .with_subtitle("Clipboard history")
                .with_keywords(vec!["clipboard".to_string(), "history".to_string()])
            })
            .collect())
    }

    fn execute(&self, action: &Action) -> Result<ActionOutcome, PluginError> {
        match action {
            Action::CopyText { text } => Ok(ActionOutcome::new(format!(
                "clipboard write is stubbed; selected {} bytes",
                text.len()
            ))),
            _ => Err(PluginError::UnsupportedAction),
        }
    }
}

fn preview(value: &str) -> String {
    let mut output = value.lines().next().unwrap_or("").trim().to_string();
    if output.chars().count() > 80 {
        output = output.chars().take(77).collect::<String>();
        output.push_str("...");
    }
    if output.is_empty() {
        "<empty clipboard item>".to_string()
    } else {
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_seeded_clipboard_items() {
        let provider = ClipboardHistoryProvider::with_items(vec!["hello".to_string()]);
        let results = provider
            .search(&SearchContext::new("hello", 10))
            .expect("clipboard search should succeed");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "hello");
    }

    #[test]
    fn preview_uses_first_line_only() {
        let provider =
            ClipboardHistoryProvider::with_items(vec!["first line\nsecond line".to_string()]);
        let results = provider
            .search(&SearchContext::new("clipboard", 10))
            .expect("clipboard search should succeed");

        assert_eq!(results[0].title, "first line");
        assert!(!results[0].title.contains("second line"));
    }

    #[test]
    fn preview_truncates_long_items() {
        let provider = ClipboardHistoryProvider::with_items(vec!["x".repeat(120)]);
        let results = provider
            .search(&SearchContext::new("clipboard", 10))
            .expect("clipboard search should succeed");

        assert_eq!(results[0].title.chars().count(), 80);
        assert!(results[0].title.ends_with("..."));
    }

    #[test]
    fn execute_message_does_not_echo_clipboard_contents() {
        let provider = ClipboardHistoryProvider::empty();
        let outcome = provider
            .execute(&Action::CopyText {
                text: "private-token-value".to_string(),
            })
            .expect("clipboard copy action should return stub outcome");

        assert_eq!(
            outcome.message,
            "clipboard write is stubbed; selected 19 bytes"
        );
        assert!(!outcome.message.contains("private-token-value"));
    }
}
