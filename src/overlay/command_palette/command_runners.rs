use crate::app::Action;
use crate::config::theme::ThemeId;
use crate::core::VisibleSequence;
use crate::core::column_stats::ConsensusMethod;
use crate::core::command::{CoreAction, DiffMode};
use crate::core::parser::SequenceType;
use crate::ui::UiAction;
use tracing::{debug, trace};

use super::command_error::CommandError;
use super::input::CommandPaletteState;
use super::utils::parse_argument;

fn ensure_no_argument(arguments: &str) -> Result<(), CommandError> {
    if parse_argument(arguments).is_some() {
        return Err(CommandError::new("Expected 0 arguments, got 1"));
    }
    Ok(())
}

fn run_command(
    command: &'static str,
    arguments: &str,
    run: impl FnOnce() -> Result<Action, CommandError>,
) -> Result<Action, CommandError> {
    let result = run();
    match &result {
        Ok(action) => {
            trace!(
                command,
                arguments = %arguments,
                action = ?action,
                "command runner produced action"
            );
        }
        Err(error) => {
            debug!(
                command,
                arguments = %arguments,
                error = %error.message,
                "command runner rejected command input"
            );
        }
    }
    result
}

pub(super) fn run_clear_filter(
    _: &CommandPaletteState,
    arguments: &str,
) -> Result<Action, CommandError> {
    run_command("clear-filter", arguments, || {
        ensure_no_argument(arguments)?;
        Ok(Action::Core(CoreAction::ClearFilter))
    })
}

pub(super) fn run_clear_reference(
    _: &CommandPaletteState,
    arguments: &str,
) -> Result<Action, CommandError> {
    run_command("clear-reference", arguments, || {
        ensure_no_argument(arguments)?;
        Ok(Action::Core(CoreAction::ClearReference))
    })
}

pub(super) fn run_toggle_translation(
    _: &CommandPaletteState,
    arguments: &str,
) -> Result<Action, CommandError> {
    run_command("toggle-translate", arguments, || {
        ensure_no_argument(arguments)?;
        Ok(Action::Core(CoreAction::ToggleTranslationView))
    })
}

pub(super) fn run_jump_position(
    _: &CommandPaletteState,
    arguments: &str,
) -> Result<Action, CommandError> {
    run_command("jump-position", arguments, || {
        let value = require_argument(arguments)?;

        let Ok(position) = value.parse::<usize>() else {
            return Err(CommandError::new(
                "Invalid argument: expected a positive integer",
            ));
        };

        Ok(Action::Core(CoreAction::JumpToPosition(
            position.saturating_sub(1),
        )))
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

fn require_argument(arguments: &str) -> Result<String, CommandError> {
    parse_argument(arguments)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| CommandError::new("Expected 1 argument, got 0"))
}

fn resolve_argument_to_sequence_id(
    sequences: &[VisibleSequence],
    arguments: &str,
) -> Result<usize, CommandError> {
    let sequence_name = require_argument(arguments)?;
    lookup_sequence_id(sequences, sequence_name.as_str())
        .ok_or_else(|| CommandError::new(format!("Sequence not found: {sequence_name}")))
}

pub(super) fn run_jump_sequence(
    state: &CommandPaletteState,
    arguments: &str,
) -> Result<Action, CommandError> {
    run_command("jump-sequence", arguments, || {
        let sequence_id = resolve_argument_to_sequence_id(&state.selectable_sequences, arguments)?;
        Ok(Action::Core(CoreAction::JumpToSequence(sequence_id)))
    })
}

pub(super) fn run_pin_sequence(
    state: &CommandPaletteState,
    arguments: &str,
) -> Result<Action, CommandError> {
    run_command("pin-sequence", arguments, || {
        let sequence_id = resolve_argument_to_sequence_id(&state.selectable_sequences, arguments)?;
        Ok(Action::Core(CoreAction::PinSequence(sequence_id)))
    })
}

pub(super) fn run_unpin_sequence(
    state: &CommandPaletteState,
    arguments: &str,
) -> Result<Action, CommandError> {
    run_command("unpin-sequence", arguments, || {
        let sequence_id = resolve_argument_to_sequence_id(&state.pinned_sequences, arguments)?;
        Ok(Action::Core(CoreAction::UnpinSequence(sequence_id)))
    })
}

pub(super) fn run_filter(_: &CommandPaletteState, arguments: &str) -> Result<Action, CommandError> {
    run_command("set-filter", arguments, || {
        if arguments.is_empty() {
            Ok(Action::Core(CoreAction::ClearFilter))
        } else {
            match regex::Regex::new(arguments) {
                Ok(regex) => Ok(Action::Core(CoreAction::SetFilter {
                    pattern: arguments.to_string(),
                    regex,
                })),
                Err(_) => Err(CommandError::new(
                    "Invalid argument: expected a valid regular expression",
                )),
            }
        }
    })
}

pub(super) fn run_set_reference(
    state: &CommandPaletteState,
    arguments: &str,
) -> Result<Action, CommandError> {
    run_command("set-reference", arguments, || {
        let arg = require_argument(arguments)?;

        let sequence_id = lookup_sequence_id(&state.selectable_sequences, arg.as_str())
            .ok_or_else(|| CommandError::new(format!("Sequence not found: {arg}")))?;
        Ok(Action::Core(CoreAction::SetReference(sequence_id)))
    })
}

pub(super) fn run_load_alignment(
    _: &CommandPaletteState,
    arguments: &str,
) -> Result<Action, CommandError> {
    run_command("load-alignment", arguments, || {
        let path = require_argument(arguments)?;

        Ok(Action::LoadFile { path: path.into() })
    })
}

fn parse_consensus_method(arg: &str) -> Option<ConsensusMethod> {
    match arg {
        "majority" => Some(ConsensusMethod::Majority),
        "majority-non-gap" => Some(ConsensusMethod::MajorityNonGap),
        _ => None,
    }
}

fn parse_diff_mode(arg: &str) -> Option<DiffMode> {
    match arg {
        "off" => Some(DiffMode::Off),
        "reference" => Some(DiffMode::Reference),
        "consensus" => Some(DiffMode::Consensus),
        _ => None,
    }
}

pub(super) fn run_diff_mode(
    _: &CommandPaletteState,
    arguments: &str,
) -> Result<Action, CommandError> {
    run_command("set-diff-mode", arguments, || {
        let arg = require_argument(arguments)?;
        let mode = parse_diff_mode(arg.as_str()).ok_or_else(|| {
            CommandError::new(format!("Invalid argument for set-diff-mode: {arg}"))
        })?;
        Ok(Action::Core(CoreAction::SetDiffMode(mode)))
    })
}

pub(super) fn run_consensus_method(
    _: &CommandPaletteState,
    arguments: &str,
) -> Result<Action, CommandError> {
    run_command("set-consensus-method", arguments, || {
        let arg = require_argument(arguments)?;
        let method = parse_consensus_method(arg.as_str()).ok_or_else(|| {
            CommandError::new(format!("Invalid argument for set-consensus-method: {arg}"))
        })?;
        Ok(Action::Core(CoreAction::SetConsensusMethod(method)))
    })
}

fn parse_translation_frame(arg: &str) -> Option<u8> {
    arg.parse::<u8>()
        .ok()
        .filter(|frame| (1..=3).contains(frame))
}

pub(super) fn run_translation_frame(
    _: &CommandPaletteState,
    arguments: &str,
) -> Result<Action, CommandError> {
    run_command("set-translation-frame", arguments, || {
        let arg = require_argument(arguments)?;
        let frame = parse_translation_frame(arg.as_str()).ok_or_else(|| {
            CommandError::new(format!("Invalid argument for set-translation-frame: {arg}"))
        })?;
        Ok(Action::Core(CoreAction::SetTranslationFrame(frame)))
    })
}

fn parse_theme(arg: &str) -> Option<ThemeId> {
    match arg {
        "everforest-dark" => Some(ThemeId::EverforestDark),
        _ => None,
    }
}

pub(super) fn run_theme(_: &CommandPaletteState, arguments: &str) -> Result<Action, CommandError> {
    run_command("set-theme", arguments, || {
        let arg = require_argument(arguments)?;
        let theme = parse_theme(arg.as_str())
            .ok_or_else(|| CommandError::new(format!("Invalid argument for set-theme: {arg}")))?;
        Ok(Action::Ui(UiAction::SetTheme(theme)))
    })
}

fn parse_sequence_type(arg: &str) -> Option<SequenceType> {
    match arg {
        "dna" => Some(SequenceType::Dna),
        "aa" => Some(SequenceType::AminoAcid),
        _ => None,
    }
}

pub(super) fn run_sequence_type(
    _: &CommandPaletteState,
    arguments: &str,
) -> Result<Action, CommandError> {
    run_command("set-sequence-type", arguments, || {
        let arg = require_argument(arguments)?;
        let sequence_type = parse_sequence_type(arg.as_str()).ok_or_else(|| {
            CommandError::new(format!("Invalid argument for set-sequence-type: {arg}"))
        })?;
        Ok(Action::Core(CoreAction::SetSequenceType(sequence_type)))
    })
}

pub(super) fn run_quit(_: &CommandPaletteState, arguments: &str) -> Result<Action, CommandError> {
    run_command("quit", arguments, || {
        ensure_no_argument(arguments)?;
        Ok(Action::Quit)
    })
}
