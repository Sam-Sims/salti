use std::fs;

use strum::VariantArray;

use crate::{
    core::session::Session,
    ui::popups::{Context, fuzzy::fuzzy_match},
};

pub(super) fn sequences(ctx: &Context, word: &str) -> Vec<String> {
    fuzzy_match(word, names(ctx, |session| session.layout().rows()))
}

pub(super) fn main_sequences(ctx: &Context, word: &str) -> Vec<String> {
    fuzzy_match(word, names(ctx, |session| session.layout().main()))
}

pub(super) fn pinned_sequences(ctx: &Context, word: &str) -> Vec<String> {
    fuzzy_match(word, names(ctx, |session| session.state().pinned()))
}

pub(super) fn features(ctx: &Context, word: &str) -> Vec<String> {
    let names = ctx.gff.into_iter().flat_map(|gff| &gff.features);
    fuzzy_match(word, names.map(|feature| feature.name.clone()))
}

pub(super) fn gap_percentages(_: &Context, word: &str) -> Vec<String> {
    fuzzy_match(word, ["0", "5", "10", "25", "50"].map(str::to_string))
}

pub(super) fn constant_percentages(_: &Context, word: &str) -> Vec<String> {
    fuzzy_match(word, ["0", "70", "90", "95", "100"].map(str::to_string))
}

pub(super) fn choices<T: VariantArray + ToString>(_: &Context, word: &str) -> Vec<String> {
    fuzzy_match(word, T::VARIANTS.iter().map(ToString::to_string))
}

pub(super) fn filter_matches(ctx: &Context, word: &str) -> Vec<String> {
    let names = names(ctx, |session| session.layout().rows());
    if word.is_empty() {
        return names;
    }
    let Ok(regex) = regex::Regex::new(word) else {
        return Vec::new();
    };
    names
        .into_iter()
        .filter(|name| regex.is_match(name))
        .collect()
}

pub(super) fn filename(_: &Context, word: &str) -> Vec<String> {
    let (dir, prefix) = word
        .rfind('/')
        .map_or(("", word), |index| word.split_at(index + 1));
    let Ok(entries) = fs::read_dir(if dir.is_empty() { "." } else { dir }) else {
        return Vec::new();
    };
    let mut matches: Vec<(bool, String)> = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if !name.starts_with(prefix) {
                return None;
            }
            let is_dir = entry.file_type().is_ok_and(|kind| kind.is_dir());
            let slash = if is_dir { "/" } else { "" };
            Some((!is_dir, format!("{dir}{name}{slash}")))
        })
        .collect();
    matches.sort();
    matches.into_iter().map(|(_, label)| label).collect()
}

fn names(ctx: &Context, rows: fn(&Session) -> &[usize]) -> Vec<String> {
    ctx.session
        .map(|session| {
            rows(session)
                .iter()
                .map(|&row| session.base_alignment().id(row).to_string())
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;
    use crate::{
        core::session::DiffMode,
        test_utils::{genes, pinned_session, with_context},
        ui::popups::palette::commands::Completer,
    };

    fn complete(completer: Completer, argument: &str) -> Vec<String> {
        let session = pinned_session(3, &[1]);
        let gff = genes(&[0..3, 3..6]);
        with_context(Some(&session), Some(&gff), |ctx| completer(ctx, argument))
    }

    #[rstest]
    #[case::sequences_pinned_first(sequences, "", &["s1", "s0", "s2"])]
    #[case::main_sequences_skip_pinned(main_sequences, "", &["s0", "s2"])]
    #[case::pinned_sequences(pinned_sequences, "", &["s1"])]
    #[case::features_in_gff_order(features, "", &["gene0", "gene1"])]
    #[case::filter_matches_all_without_pattern(filter_matches, "", &["s1", "s0", "s2"])]
    #[case::filter_matches_regex(filter_matches, "s[02]", &["s0", "s2"])]
    #[case::main_sequences_fuzzy_match_word(main_sequences, "2", &["s2"])]
    #[case::choices_in_order(choices::<DiffMode>, "", &["off", "reference", "consensus"])]
    fn completer_works(
        #[case] completer: Completer,
        #[case] argument: &str,
        #[case] expected: &[&str],
    ) {
        assert_eq!(complete(completer, argument), expected);
    }

    #[rstest]
    #[case::filter_matches_invalid_regex(filter_matches, "(")]
    fn completer_is_empty(#[case] completer: Completer, #[case] argument: &str) {
        assert_eq!(complete(completer, argument), Vec::<String>::new());
    }

    #[rstest]
    #[case::sequences(sequences)]
    #[case::main_sequences(main_sequences)]
    #[case::pinned_sequences(pinned_sequences)]
    #[case::features(features)]
    fn completer_is_empty_without_document(#[case] completer: Completer) {
        assert_eq!(
            with_context(None, None, |ctx| completer(ctx, "")),
            Vec::<String>::new()
        );
    }
}
