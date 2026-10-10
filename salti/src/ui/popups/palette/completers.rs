use std::{fs, path::Path};

use super::names;
use crate::{config::theme::ThemeId, core::session::DiffMode, ui::popups::Context};

pub(super) fn sequences(ctx: &Context, _: &str) -> Vec<String> {
    ctx.session
        .map(|session| names(session, session.layout().rows()))
        .unwrap_or_default()
}

pub(super) fn main_sequences(ctx: &Context, _: &str) -> Vec<String> {
    ctx.session
        .map(|session| names(session, session.layout().main()))
        .unwrap_or_default()
}

pub(super) fn pinned_sequences(ctx: &Context, _: &str) -> Vec<String> {
    ctx.session
        .map(|session| {
            let layout = session.layout();
            names(session, &layout.rows()[..layout.pinned()])
        })
        .unwrap_or_default()
}

pub(super) fn features(ctx: &Context, _: &str) -> Vec<String> {
    ctx.gff
        .map(|gff| {
            gff.features
                .iter()
                .map(|feature| feature.name.clone())
                .collect()
        })
        .unwrap_or_default()
}

pub(super) fn gap_percentages(_: &Context, _: &str) -> Vec<String> {
    ["0", "5", "10", "25", "50"].map(str::to_string).to_vec()
}

pub(super) fn constant_percentages(_: &Context, _: &str) -> Vec<String> {
    ["0", "70", "90", "95", "100"].map(str::to_string).to_vec()
}

pub(super) fn frames(_: &Context, _: &str) -> Vec<String> {
    libmsa::ReadingFrame::all()
        .map(|frame| frame.name().to_string())
        .to_vec()
}

pub(super) fn consensus_methods(_: &Context, _: &str) -> Vec<String> {
    libmsa::ConsensusMethod::all()
        .map(|method| method.name().to_string())
        .to_vec()
}

pub(super) fn diff_modes(_: &Context, _: &str) -> Vec<String> {
    DiffMode::all().map(|mode| mode.name().to_string()).to_vec()
}

pub(super) fn themes(_: &Context, _: &str) -> Vec<String> {
    ThemeId::ALL
        .iter()
        .map(|theme| theme.name().to_string())
        .collect()
}

pub(super) fn sequence_types(_: &Context, _: &str) -> Vec<String> {
    ["dna", "protein", "generic"].map(str::to_string).to_vec()
}

pub(super) fn filter_matches(ctx: &Context, argument: &str) -> Vec<String> {
    let names = sequences(ctx, argument);
    let pattern = argument.trim();
    if pattern.is_empty() {
        return names;
    }
    let Ok(regex) = regex::Regex::new(pattern) else {
        return Vec::new();
    };
    names
        .into_iter()
        .filter(|name| regex.is_match(name))
        .collect()
}

pub(super) fn filename(_: &Context, arguments: &str) -> Vec<String> {
    let (dir_prefix, name_prefix) = split_dir_and_prefix(arguments);
    let base_dir = if dir_prefix.is_empty() {
        Path::new(".")
    } else {
        Path::new(dir_prefix)
    };

    let Ok(entries) = fs::read_dir(base_dir) else {
        return Vec::new();
    };

    let mut matches = Vec::new();

    for entry in entries.flatten() {
        let entry_name = entry.file_name();
        let entry_name = entry_name.to_string_lossy();

        if !entry_name.starts_with(name_prefix) {
            continue;
        }

        let is_dir = entry.file_type().is_ok_and(|kind| kind.is_dir());
        let mut label = join_display_path(dir_prefix, &entry_name);
        if is_dir {
            label.push('/');
        }
        matches.push((!is_dir, label));
    }

    matches.sort();
    matches.into_iter().map(|(_, label)| label).collect()
}

fn split_dir_and_prefix(query: &str) -> (&str, &str) {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return ("", "");
    }

    if let Some(index) = trimmed.rfind('/') {
        let (dir, rest) = trimmed.split_at(index + 1);
        return (dir, rest);
    }

    ("", trimmed)
}

fn join_display_path(dir_prefix: &str, name: &str) -> String {
    if dir_prefix.is_empty() {
        return name.to_string();
    }

    let mut joined = String::with_capacity(dir_prefix.len() + name.len());
    joined.push_str(dir_prefix);
    joined.push_str(name);
    joined
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;
    use crate::test_utils::{genes, pinned_session, with_context};

    type CompleterFn = fn(&Context, &str) -> Vec<String>;

    fn complete(completer: CompleterFn, argument: &str) -> Vec<String> {
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
    fn completer_works(
        #[case] completer: CompleterFn,
        #[case] argument: &str,
        #[case] expected: &[&str],
    ) {
        assert_eq!(complete(completer, argument), expected);
    }

    #[rstest]
    #[case::filter_matches_invalid_regex(filter_matches, "(")]
    fn completer_is_empty(#[case] completer: CompleterFn, #[case] argument: &str) {
        assert_eq!(complete(completer, argument), Vec::<String>::new());
    }

    #[rstest]
    #[case::sequences(sequences)]
    #[case::main_sequences(main_sequences)]
    #[case::pinned_sequences(pinned_sequences)]
    #[case::features(features)]
    fn completer_is_empty_without_document(#[case] completer: CompleterFn) {
        assert_eq!(
            with_context(None, None, |ctx| completer(ctx, "")),
            Vec::<String>::new()
        );
    }
}
