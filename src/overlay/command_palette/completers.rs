use std::fs;
use std::path::PathBuf;

use super::input::CommandPaletteState;

fn sequence_names_from(sequences: &[crate::core::VisibleSequence]) -> Vec<String> {
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
