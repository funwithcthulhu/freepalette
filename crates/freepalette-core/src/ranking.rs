use freepalette_plugin_api::{ResultKind, SearchResult};
use serde::Serialize;

use crate::fuzzy::fuzzy_score;

const EXACT_TITLE_MATCH_BONUS: i64 = 300;
const PREFIX_TITLE_MATCH_BONUS: i64 = 120;
const APP_RESULT_BIAS: i64 = 30;
const CALCULATOR_RESULT_BIAS: i64 = 25;
const SHELL_RESULT_BIAS: i64 = 20;
const CLIPBOARD_RESULT_BIAS: i64 = 10;

/// A result after applying the core ranking model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RankedResult {
    pub result: SearchResult,
    pub score: i64,
}

/// Rank provider results using a small, documented scoring model:
///
/// - fuzzy title/subtitle/keyword match is the largest generic signal;
/// - providers may add a modest score hint for exact command-style results;
/// - exact and prefix title matches receive a small bonus;
/// - result kind has a tiny bias so app/calculator/shell stay visible in MVP.
pub fn rank_results(query: &str, results: Vec<SearchResult>) -> Vec<RankedResult> {
    let mut ranked = results
        .into_iter()
        .filter_map(|result| rank_result(query, result))
        .collect::<Vec<_>>();

    ranked.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| left.result.title.cmp(&right.result.title))
            .then_with(|| left.result.id.cmp(&right.result.id))
    });

    ranked
}

fn rank_result(query: &str, result: SearchResult) -> Option<RankedResult> {
    let trimmed_query = query.trim();
    let haystack = result_haystack(&result);
    let fuzzy = fuzzy_score(trimmed_query, &haystack);

    if !trimmed_query.is_empty() && fuzzy.is_none() && result.score_hint <= 0 {
        return None;
    }

    let title = result.title.to_ascii_lowercase();
    let query_lower = trimmed_query.to_ascii_lowercase();
    let exact_bonus = if !query_lower.is_empty() && title == query_lower {
        EXACT_TITLE_MATCH_BONUS
    } else if !query_lower.is_empty() && title.starts_with(&query_lower) {
        PREFIX_TITLE_MATCH_BONUS
    } else {
        0
    };

    Some(RankedResult {
        score: fuzzy.unwrap_or(0) + result.score_hint + exact_bonus + kind_bias(result.kind),
        result,
    })
}

fn result_haystack(result: &SearchResult) -> String {
    let mut parts = vec![result.title.as_str()];
    if let Some(subtitle) = &result.subtitle {
        parts.push(subtitle);
    }
    parts.extend(result.keywords.iter().map(String::as_str));
    parts.join(" ")
}

fn kind_bias(kind: ResultKind) -> i64 {
    match kind {
        ResultKind::App => APP_RESULT_BIAS,
        ResultKind::Calculator => CALCULATOR_RESULT_BIAS,
        ResultKind::Shell => SHELL_RESULT_BIAS,
        ResultKind::Clipboard => CLIPBOARD_RESULT_BIAS,
        ResultKind::Plugin | ResultKind::System => 0,
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    use freepalette_plugin_api::{Action, Provider, ProviderId, SearchContext};

    use crate::providers::{CalculatorProvider, ShellCommandProvider};

    use super::*;

    fn result(title: &str, kind: ResultKind, score_hint: i64) -> SearchResult {
        result_from_provider("test", &title.to_ascii_lowercase(), title, kind, score_hint)
    }

    fn result_from_provider(
        provider: &str,
        id: &str,
        title: &str,
        kind: ResultKind,
        score_hint: i64,
    ) -> SearchResult {
        SearchResult::new(
            ProviderId::from(provider),
            id,
            title,
            kind,
            Action::Noop {
                message: "noop".to_string(),
            },
        )
        .with_score_hint(score_hint)
    }

    struct TempMarker {
        path: PathBuf,
    }

    impl TempMarker {
        fn new(name: &str) -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be after Unix epoch")
                .as_nanos();
            let path = std::env::temp_dir().join(format!("freepalette-ranking-{name}-{unique}"));
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempMarker {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.path);
        }
    }

    #[cfg(target_os = "windows")]
    fn shell_write_marker_command(path: &Path) -> String {
        format!("echo ranking-shell-ran > \"{}\"", path.display())
    }

    #[cfg(not(target_os = "windows"))]
    fn shell_write_marker_command(path: &Path) -> String {
        let quoted = path.display().to_string().replace('\'', "'\\''");
        format!("printf ranking-shell-ran > '{quoted}'")
    }

    #[test]
    fn exact_title_match_beats_weaker_match() {
        let ranked = rank_results(
            "notepad",
            vec![
                result("Notes archive", ResultKind::App, 0),
                result("Notepad", ResultKind::App, 0),
            ],
        );

        assert_eq!(ranked[0].result.title, "Notepad");
    }

    #[test]
    fn exact_app_name_match_orders_before_partial_app_name_match() {
        let ranked = rank_results(
            "notepad",
            vec![
                result_from_provider(
                    "apps",
                    "notepad-helper",
                    "Notepad Helper",
                    ResultKind::App,
                    0,
                ),
                result_from_provider("apps", "notepad", "Notepad", ResultKind::App, 0),
            ],
        );

        let titles = ranked
            .iter()
            .map(|ranked| ranked.result.title.as_str())
            .collect::<Vec<_>>();
        assert_eq!(titles, vec!["Notepad", "Notepad Helper"]);
    }

    #[test]
    fn score_hint_keeps_dynamic_provider_result() {
        let ranked = rank_results(
            "calc 2+2",
            vec![result("2 + 2 = 4", ResultKind::Calculator, 900)],
        );

        assert_eq!(ranked.len(), 1);
        assert_eq!(ranked[0].result.kind, ResultKind::Calculator);
    }

    #[test]
    fn calculator_query_keeps_calculator_result_before_unrelated_results() {
        let calculator = CalculatorProvider;
        let mut results = vec![
            result_from_provider("apps", "notepad", "Notepad", ResultKind::App, 0),
            result_from_provider("plugins", "calendar", "Calendar", ResultKind::Plugin, 0),
        ];
        results.extend(
            calculator
                .search(&SearchContext::new("calc 2+2", 10))
                .expect("calculator search should succeed"),
        );

        let ranked = rank_results("calc 2+2", results);

        assert_eq!(ranked[0].result.provider, ProviderId::from("calculator"));
        assert_eq!(ranked[0].result.kind, ResultKind::Calculator);
        assert_eq!(ranked[0].result.title, "2+2 = 4");
    }

    #[test]
    fn shell_query_keeps_shell_result_visible_without_executing_action() {
        let marker = TempMarker::new("shell-visible");
        let command = shell_write_marker_command(marker.path());
        let query = format!("> {command}");
        let shell = ShellCommandProvider;
        let mut results = vec![result_from_provider(
            "apps",
            "echo-helper",
            "Echo Helper",
            ResultKind::App,
            0,
        )];
        results.extend(
            shell
                .search(&SearchContext::new(query.as_str(), 10))
                .expect("shell search should succeed"),
        );

        let ranked = rank_results(&query, results);

        assert!(
            !marker.path().exists(),
            "ranking/search must not execute shell actions"
        );
        let shell_result = ranked
            .iter()
            .find(|ranked| ranked.result.kind == ResultKind::Shell)
            .expect("shell result should remain visible");
        assert!(matches!(
            &shell_result.result.action,
            Action::RunShell { command: actual } if actual == &command
        ));
    }

    #[test]
    fn equal_scores_sort_by_title_then_id() {
        let mut alpha_second = result("Alpha", ResultKind::System, 0);
        alpha_second.id = "b".to_string();
        let mut alpha_first = result("Alpha", ResultKind::System, 0);
        alpha_first.id = "a".to_string();

        let ranked = rank_results(
            "",
            vec![
                result("Beta", ResultKind::System, 0),
                alpha_second,
                alpha_first,
            ],
        );

        assert_eq!(ranked[0].result.title, "Alpha");
        assert_eq!(ranked[0].result.id, "a");
        assert_eq!(ranked[1].result.id, "b");
        assert_eq!(ranked[2].result.title, "Beta");
    }
}
