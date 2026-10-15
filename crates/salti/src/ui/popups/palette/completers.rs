use std::fs;

use strum::VariantArray;

use crate::{
    config::theme::ThemeChoice,
    session::Session,
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

pub(super) fn theme(_: &Context, word: &str) -> Vec<String> {
    fuzzy_match(word, ThemeChoice::names().map(str::to_string))
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
    use std::fs;

    use rstest::rstest;

    use super::*;
    use crate::{
        session::DiffMode,
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

    fn filenames(entries: &[&str], word: &str) -> Vec<String> {
        let dir = tempfile::tempdir().unwrap();
        for entry in entries {
            let path = dir.path().join(entry);
            if entry.ends_with('/') {
                fs::create_dir_all(path).unwrap();
            } else {
                fs::create_dir_all(path.parent().unwrap()).unwrap();
                fs::write(path, "").unwrap();
            }
        }
        let root = format!("{}/", dir.path().display());
        with_context(None, None, |ctx| filename(ctx, &format!("{root}{word}")))
            .into_iter()
            .map(|label| label.strip_prefix(&root).unwrap().to_string())
            .collect()
    }

    #[rstest]
    #[case::every_entry_for_empty_word(&["b.fa", "a.fa"], "", &["a.fa", "b.fa"])]
    #[case::directories_first(&["a.fa", "z/"], "", &["z/", "a.fa"])]
    #[case::sorted_by_byte(&["b.fa", "C.fa"], "", &["C.fa", "b.fa"])]
    #[case::prefix_filters(&["ab.fa", "ac/", "b.fa"], "a", &["ac/", "ab.fa"])]
    #[case::whole_name_as_prefix(&["a.fa", "a.fasta"], "a.fa", &["a.fa", "a.fasta"])]
    #[case::nested_directory(&["inner/x.fa", "inner/y/"], "inner/", &["inner/y/", "inner/x.fa"])]
    #[case::nested_prefix(&["inner/x.fa", "inner/z.fa"], "inner/x", &["inner/x.fa"])]
    #[case::parent_directory(&["inner/", "a.fa"], "inner/../", &["inner/../inner/", "inner/../a.fa"])]
    #[case::hidden_entries_listed(&[".hidden", "a.fa"], "", &[".hidden", "a.fa"])]
    #[case::dot_prefix_matches_hidden(&[".hidden", "a.fa"], ".", &[".hidden"])]
    #[case::space_inside_name(&["my file.fa", "my.fa"], "my ", &["my file.fa"])]
    #[case::leading_space_in_name(&[" a.fa", "a.fa"], " ", &[" a.fa"])]
    #[case::quote_in_name(&["it's.fa"], "it'", &["it's.fa"])]
    #[case::multibyte_name(&["é.fa", "e.fa"], "é", &["é.fa"])]
    fn filename_works(#[case] entries: &[&str], #[case] word: &str, #[case] expected: &[&str]) {
        assert_eq!(filenames(entries, word), expected);
    }

    #[test]
    #[cfg(not(windows))]
    fn filename_keeps_trailing_space_in_name() {
        assert_eq!(filenames(&["a.fa ", "a.fasta"], "a.fa "), ["a.fa "]);
    }

    #[rstest]
    #[case::no_match(&["a.fa"], "z")]
    #[case::prefix_is_case_sensitive(&["A.fa"], "a")]
    #[case::missing_directory(&["a.fa"], "nope/")]
    #[case::file_used_as_directory(&["a.fa"], "a.fa/")]
    #[case::whitespace_only_word(&["a.fa"], "  ")]
    #[case::empty_directory(&["empty/"], "empty/")]
    fn filename_is_empty(#[case] entries: &[&str], #[case] word: &str) {
        assert_eq!(filenames(entries, word), Vec::<String>::new());
    }
}
