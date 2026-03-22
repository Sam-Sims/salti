use crate::command::Command;
use anyhow::format_err;
use tracing::warn;

use super::input::CommandPaletteState;
use super::input::VisibleSequence;
use super::utils::parse_argument;

fn ensure_no_argument(arguments: &str) -> anyhow::Result<()> {
    if parse_argument(arguments).is_some() {
        return Err(format_err!("Expected 0 arguments, got 1"));
    }
    Ok(())
}

fn run_command(
    command: &'static str,
    arguments: &str,
    run: impl FnOnce() -> anyhow::Result<Command>,
) -> anyhow::Result<Command> {
    let result = run();
    if let Err(error) = &result {
        warn!(
            command,
            arguments = %arguments,
            error = %error,
            "Command runner rejected command input"
        );
    }
    result
}

pub(super) fn run_clear_filter(
    _: &CommandPaletteState,
    arguments: &str,
) -> anyhow::Result<Command> {
    run_command("clear-filter", arguments, || {
        ensure_no_argument(arguments)?;
        Ok(Command::ClearFilter)
    })
}

pub(super) fn run_clear_reference(
    _: &CommandPaletteState,
    arguments: &str,
) -> anyhow::Result<Command> {
    run_command("clear-reference", arguments, || {
        ensure_no_argument(arguments)?;
        Ok(Command::ClearReference)
    })
}

pub(super) fn run_toggle_translation(
    state: &CommandPaletteState,
    arguments: &str,
) -> anyhow::Result<Command> {
    run_command("toggle-translate", arguments, || {
        ensure_no_argument(arguments)?;
        if state.active_type != libmsa::AlignmentType::Dna {
            return Err(format_err!(
                "toggle-translate is only available for DNA sequences",
            ));
        }
        Ok(Command::ToggleTranslationView)
    })
}

fn next_visible_column_index(visible_columns: &[usize], absolute_target: usize) -> Option<usize> {
    match visible_columns.binary_search(&absolute_target) {
        Ok(visible_index) => Some(visible_index),
        Err(next_visible_index) => {
            (next_visible_index < visible_columns.len()).then_some(next_visible_index)
        }
    }
}

pub(super) fn run_filter_gaps(_: &CommandPaletteState, arguments: &str) -> anyhow::Result<Command> {
    run_command("filter-gaps", arguments, || {
        let value = require_argument(arguments)?;
        let Ok(percent) = value.parse::<f32>() else {
            return Err(format_err!(
                "Invalid argument: expected a percentage in 0..=100",
            ));
        };
        if !percent.is_finite() || !(0.0..=100.0).contains(&percent) {
            return Err(format_err!(
                "Invalid argument: expected a percentage in 0..=100",
            ));
        }

        let max_gap_fraction = if percent == 0.0 {
            None
        } else {
            Some(percent / 100.0)
        };

        Ok(Command::SetGapFilter(max_gap_fraction))
    })
}

pub(super) fn run_jump_position(
    state: &CommandPaletteState,
    arguments: &str,
) -> anyhow::Result<Command> {
    run_command("jump-position", arguments, || {
        let value = require_argument(arguments)?;

        let Ok(position) = value.parse::<usize>() else {
            return Err(format_err!("Invalid argument: expected a positive integer",));
        };
        if position == 0 {
            return Err(format_err!("Invalid argument: expected a positive integer",));
        }

        let absolute_target = position - 1;
        let Some(visible_col) = next_visible_column_index(&state.visible_columns, absolute_target)
        else {
            return Err(format_err!(
                "No visible column at or after the requested position",
            ));
        };

        Ok(Command::JumpToPosition(visible_col))
    })
}

// this searches through the visible sequences to get the seq id - in the future might want to
// consider a hashmap? would changes behaviour to last seq wins rather than first
fn lookup_sequence_id(sequences: &[VisibleSequence], sequence_name: &str) -> Option<usize> {
    sequences
        .iter()
        .find(|sequence| sequence.sequence_name.as_ref() == sequence_name)
        .map(|sequence| sequence.sequence_id)
}

fn require_argument(arguments: &str) -> anyhow::Result<String> {
    parse_argument(arguments)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format_err!("Expected 1 argument, got 0"))
}

fn resolve_argument_to_sequence_id(
    sequences: &[VisibleSequence],
    arguments: &str,
) -> anyhow::Result<usize> {
    let sequence_name = require_argument(arguments)?;
    lookup_sequence_id(sequences, sequence_name.as_str())
        .ok_or_else(|| format_err!("Sequence not found: {sequence_name}"))
}

pub(super) fn run_jump_sequence(
    state: &CommandPaletteState,
    arguments: &str,
) -> anyhow::Result<Command> {
    run_command("jump-sequence", arguments, || {
        let sequence_id = resolve_argument_to_sequence_id(&state.selectable_sequences, arguments)?;
        Ok(Command::JumpToSequence(sequence_id))
    })
}

pub(super) fn run_pin_sequence(
    state: &CommandPaletteState,
    arguments: &str,
) -> anyhow::Result<Command> {
    run_command("pin-sequence", arguments, || {
        let sequence_id = resolve_argument_to_sequence_id(&state.selectable_sequences, arguments)?;
        Ok(Command::PinSequence(sequence_id))
    })
}

pub(super) fn run_unpin_sequence(
    state: &CommandPaletteState,
    arguments: &str,
) -> anyhow::Result<Command> {
    run_command("unpin-sequence", arguments, || {
        let sequence_id = resolve_argument_to_sequence_id(&state.pinned_sequences, arguments)?;
        Ok(Command::UnpinSequence(sequence_id))
    })
}

pub(super) fn run_filter_rows(_: &CommandPaletteState, arguments: &str) -> anyhow::Result<Command> {
    run_command("filter-rows", arguments, || {
        if arguments.is_empty() {
            Ok(Command::ClearFilter)
        } else {
            Ok(Command::SetFilter(arguments.to_string()))
        }
    })
}

pub(super) fn run_set_reference(
    state: &CommandPaletteState,
    arguments: &str,
) -> anyhow::Result<Command> {
    run_command("set-reference", arguments, || {
        let arg = require_argument(arguments)?;

        let sequence_id = lookup_sequence_id(&state.selectable_sequences, arg.as_str())
            .ok_or_else(|| format_err!("Sequence not found: {arg}"))?;
        Ok(Command::SetReference(sequence_id))
    })
}

pub(super) fn run_load_alignment(
    _: &CommandPaletteState,
    arguments: &str,
) -> anyhow::Result<Command> {
    run_command("load-alignment", arguments, || {
        let path = require_argument(arguments)?;

        Ok(Command::LoadFile { input: path })
    })
}

pub(super) fn run_diff_mode(_: &CommandPaletteState, arguments: &str) -> anyhow::Result<Command> {
    run_command("set-diff-mode", arguments, || {
        let arg = require_argument(arguments)?;
        let mode = arg.parse()?;
        Ok(Command::SetDiffMode(mode))
    })
}

pub(super) fn run_consensus_method(
    _: &CommandPaletteState,
    arguments: &str,
) -> anyhow::Result<Command> {
    run_command("set-consensus-method", arguments, || {
        let arg = require_argument(arguments)?;
        let method = arg
            .parse()
            .ok()
            .ok_or_else(|| format_err!("Invalid argument for set-consensus-method: {arg}"))?;
        Ok(Command::SetConsensusMethod(method))
    })
}

pub(super) fn run_translation_frame(
    _: &CommandPaletteState,
    arguments: &str,
) -> anyhow::Result<Command> {
    run_command("set-translation-frame", arguments, || {
        let arg = require_argument(arguments)?;
        let frame = arg
            .parse()
            .ok()
            .ok_or_else(|| format_err!("Invalid argument for set-translation-frame: {arg}"))?;
        Ok(Command::SetTranslationFrame(frame))
    })
}

pub(super) fn run_theme(_: &CommandPaletteState, arguments: &str) -> anyhow::Result<Command> {
    run_command("set-theme", arguments, || {
        let arg = require_argument(arguments)?;
        let theme = arg
            .parse()
            .ok()
            .ok_or_else(|| format_err!("Invalid argument for set-theme: {arg}"))?;
        Ok(Command::SetTheme(theme))
    })
}

pub(super) fn run_set_active_type(
    _: &CommandPaletteState,
    arguments: &str,
) -> anyhow::Result<Command> {
    run_command("set-sequence-type", arguments, || {
        let arg = require_argument(arguments)?;
        let kind = arg
            .parse::<libmsa::AlignmentType>()
            .map_err(|_| format_err!("Invalid argument for set-sequence-type: {arg}"))?;
        Ok(Command::SetActiveType(kind))
    })
}

pub(super) fn run_check_update(
    _: &CommandPaletteState,
    arguments: &str,
) -> anyhow::Result<Command> {
    run_command("check-update", arguments, || {
        ensure_no_argument(arguments)?;
        Ok(Command::CheckForUpdate {
            show_success_message: true,
        })
    })
}

pub(super) fn run_quit(_: &CommandPaletteState, arguments: &str) -> anyhow::Result<Command> {
    run_command("quit", arguments, || {
        ensure_no_argument(arguments)?;
        Ok(Command::Quit)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn palette_state_with_columns(visible_columns: Vec<usize>) -> CommandPaletteState {
        CommandPaletteState::new(
            Vec::new(),
            Vec::new(),
            libmsa::AlignmentType::Dna,
            visible_columns,
        )
    }

    #[test]
    fn jump_position_uses_next_visible_column_when_target_hidden() {
        let state = palette_state_with_columns(vec![0, 3, 4]);

        let action = run_jump_position(&state, "2")
            .expect("jump-position should resolve to the next visible column");

        assert_eq!(action, Command::JumpToPosition(1));
    }

    #[test]
    fn jump_position_errors_when_target_is_after_last_visible_column() {
        let state = palette_state_with_columns(vec![0, 3, 4]);

        let error = run_jump_position(&state, "10")
            .expect_err("jump-position should reject targets after the last visible column");

        assert_eq!(
            error.to_string(),
            "No visible column at or after the requested position"
        );
    }

    #[test]
    fn jump_position_rejects_zero() {
        let state = palette_state_with_columns(vec![0, 1, 2]);

        let error =
            run_jump_position(&state, "0").expect_err("zero should be rejected as invalid input");

        assert_eq!(
            error.to_string(),
            "Invalid argument: expected a positive integer"
        );
    }

    #[test]
    fn filter_gaps_parses_percentage_into_gap_fraction() {
        let state = palette_state_with_columns(Vec::new());

        let action = run_filter_gaps(&state, "25").expect("percentage should parse");

        assert!(matches!(
            action,
            Command::SetGapFilter(Some(value))
            if (value - 0.25).abs() < f32::EPSILON
        ));
    }

    #[test]
    fn filter_gaps_zero_clears_the_gap_filter() {
        let state = palette_state_with_columns(Vec::new());

        let action = run_filter_gaps(&state, "0").expect("zero should disable the gap filter");

        assert_eq!(action, Command::SetGapFilter(None));
    }

    #[test]
    fn filter_gaps_rejects_out_of_range_values() {
        let state = palette_state_with_columns(Vec::new());

        let error =
            run_filter_gaps(&state, "120").expect_err("percentages above 100 should be rejected");

        assert_eq!(
            error.to_string(),
            "Invalid argument: expected a percentage in 0..=100"
        );
    }

    #[test]
    fn set_active_type_accepts_alignment_type_name() {
        let state = palette_state_with_columns(Vec::new());

        let action = run_set_active_type(&state, "protein")
            .expect("canonical alignment type should be accepted");

        assert!(matches!(
            action,
            Command::SetActiveType(libmsa::AlignmentType::Protein)
        ));
    }

    #[test]
    fn set_active_type_rejects_unknown_argument() {
        let state = palette_state_with_columns(Vec::new());

        let error = run_set_active_type(&state, "rna")
            .expect_err("unknown sequence type should be rejected");

        assert_eq!(
            error.to_string(),
            "Invalid argument for set-sequence-type: rna"
        );
    }
}
