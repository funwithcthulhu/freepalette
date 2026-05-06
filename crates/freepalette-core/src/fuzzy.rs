use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};

pub fn fuzzy_score(query: &str, haystack: &str) -> Option<i64> {
    let query = query.trim();
    if query.is_empty() {
        return Some(0);
    }

    SkimMatcherV2::default().fuzzy_match(haystack, query)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_abbreviated_text() {
        let score = fuzzy_score("npd", "Notepad");
        assert!(score.is_some());
    }

    #[test]
    fn rejects_unrelated_text() {
        let score = fuzzy_score("zzzz", "Notepad");
        assert!(score.is_none());
    }
}
