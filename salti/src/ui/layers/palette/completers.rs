use std::fs;
use std::path::PathBuf;

use super::input::CommandPaletteState;

fn sequence_names_from(sequences: &[super::input::VisibleSequence]) -> Vec<String> {
    sequences
        .iter()
        .map(|sequence| sequence.sequence_name.to_string())
        .collect()
}

pub(super) fn sequences(state: &CommandPaletteState, _: &str) -> Vec<String> {
    sequence_names_from(&state.selectable_sequences)
}

pub(super) fn pinned_sequences(state: &CommandPaletteState, _: &str) -> Vec<String> {
    sequence_names_from(&state.pinned_sequences)
}

pub(super) fn features(state: &CommandPaletteState, _: &str) -> Vec<String> {
    state
        .gff_feature_targets
        .as_deref()
        .map_or_else(Vec::new, |gff_feature_targets| {
            gff_feature_targets
                .iter()
                .map(|gff_feature_target| gff_feature_target.feature_name.to_string())
                .collect()
        })
}

pub(super) fn filter_matches(state: &CommandPaletteState, arguments: &str) -> Vec<String> {
    let regex_text = arguments.trim();
    if regex_text.is_empty() {
        return sequence_names_from(&state.selectable_sequences);
    }

    let Ok(regex) = regex::Regex::new(regex_text) else {
        return Vec::new();
    };

    state
        .selectable_sequences
        .iter()
        .map(|sequence| sequence.sequence_name.as_ref())
        .filter(|sequence_name| regex.is_match(sequence_name))
        .map(std::string::ToString::to_string)
        .collect()
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

pub(super) fn filename(_: &CommandPaletteState, arguments: &str) -> Vec<String> {
    let (dir_prefix, name_prefix) = split_dir_and_prefix(arguments);
    let base_dir = if dir_prefix.is_empty() {
        PathBuf::from(".")
    } else {
        PathBuf::from(dir_prefix)
    };

    let Ok(entries) = fs::read_dir(base_dir.as_path()) else {
        return Vec::new();
    };

    let mut matches = Vec::new();

    for entry in entries.flatten() {
        let entry_name = entry.file_name();
        let Some(entry_name) = entry_name.to_str() else {
            continue;
        };

        if !entry_name.starts_with(name_prefix) {
            continue;
        }

        let is_dir = entry.path().is_dir();
        let mut label = join_display_path(dir_prefix, entry_name);
        if is_dir {
            label.push('/');
        }
        matches.push((!is_dir, label));
    }

    matches.sort();
    matches.into_iter().map(|(_, label)| label).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::ui::layers::palette::input::{CommandPaletteSnapshot, GffFeatureTarget};

    fn palette_state_with_gff_features(
        gff_feature_targets: Option<Vec<GffFeatureTarget>>,
    ) -> CommandPaletteState {
        CommandPaletteState::new(CommandPaletteSnapshot {
            selectable_sequences: Vec::new(),
            pinned_sequences: Vec::new(),
            active_type: libmsa::AlignmentType::Dna,
            is_reloaded_as_protein: false,
            visible_columns: vec![0, 1, 2],
            gff_feature_targets,
        })
    }

    #[test]
    fn features_returns_gff_feature_names_in_gff_order() {
        let state = palette_state_with_gff_features(Some(vec![
            GffFeatureTarget {
                feature_name: "gene-b".into(),
                target_col: Some(1),
            },
            GffFeatureTarget {
                feature_name: "gene-a".into(),
                target_col: Some(2),
            },
        ]));

        assert_eq!(features(&state, ""), vec!["gene-b", "gene-a"]);
    }

    #[test]
    fn features_returns_an_empty_list_without_a_gff() {
        let state = palette_state_with_gff_features(None);

        assert!(features(&state, "").is_empty());
    }
}
