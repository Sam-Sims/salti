use nucleo_matcher::{
    Matcher, Utf32Str,
    pattern::{AtomKind, CaseMatching, Normalization, Pattern},
};

pub(super) fn fuzzy_match<T: AsRef<str>>(
    query: &str,
    items: impl IntoIterator<Item = T>,
) -> Vec<T> {
    if query.is_empty() {
        return items.into_iter().collect();
    }

    let pattern = Pattern::new(
        query,
        CaseMatching::Ignore,
        Normalization::Smart,
        AtomKind::Fuzzy,
    );
    let mut matcher = Matcher::default();
    let mut buffer = Vec::new();
    let mut scored: Vec<(u32, T)> = items
        .into_iter()
        .filter_map(|item| {
            let score = pattern.score(Utf32Str::new(item.as_ref(), &mut buffer), &mut matcher)?;
            Some((score, item))
        })
        .collect();
    scored.sort_by(|(score_a, _), (score_b, _)| score_b.cmp(score_a));
    scored.into_iter().map(|(_, item)| item).collect()
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::empty_query_keeps_order("", &["b", "a", "c"], &["b", "a", "c"])]
    #[case::best_match_first("ab", &["xaxb", "ab"], &["ab", "xaxb"])]
    #[case::ties_keep_order("ab", &["ab1", "ab2"], &["ab1", "ab2"])]
    #[case::drops_non_matching("ab", &["ab", "cd"], &["ab"])]
    #[case::ignores_case("AB", &["ab"], &["ab"])]
    fn fuzzy_match_works(#[case] query: &str, #[case] items: &[&str], #[case] expected: &[&str]) {
        assert_eq!(fuzzy_match(query, items.iter().copied()), expected);
    }
}
