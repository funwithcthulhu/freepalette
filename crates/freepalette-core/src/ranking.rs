use freepalette_plugin_api::{ResultKind, SearchResult};
use serde::Serialize;

use crate::fuzzy::fuzzy_score;

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
        300
    } else if !query_lower.is_empty() && title.starts_with(&query_lower) {
        120
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
        ResultKind::App => 30,
        ResultKind::Calculator => 25,
        ResultKind::Shell => 20,
        ResultKind::Clipboard => 10,
        ResultKind::Plugin | ResultKind::System => 0,
    }
}

#[cfg(test)]
mod tests {
    use freepalette_plugin_api::{Action, ProviderId};

    use super::*;

    fn result(title: &str, kind: ResultKind, score_hint: i64) -> SearchResult {
        SearchResult::new(
            ProviderId::from("test"),
            title.to_ascii_lowercase(),
            title,
            kind,
            Action::Noop {
                message: "noop".to_string(),
            },
        )
        .with_score_hint(score_hint)
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
    fn score_hint_keeps_dynamic_provider_result() {
        let ranked = rank_results(
            "calc 2+2",
            vec![result("2 + 2 = 4", ResultKind::Calculator, 900)],
        );

        assert_eq!(ranked.len(), 1);
        assert_eq!(ranked[0].result.kind, ResultKind::Calculator);
    }
}
