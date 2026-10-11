#![allow(clippy::unnecessary_wraps)]

use std::{num::NonZeroUsize, str::FromStr};

use anyhow::{Context as _, Result, format_err};

use super::row_named;
use crate::{command::Command, ui::popups::Context};

pub(super) fn jump_position(_: &Context, argument: &str) -> Result<Command> {
    let position = argument
        .parse::<NonZeroUsize>()
        .map_err(|_| format_err!("expected a positive integer, got '{argument}'"))?;
    Ok(Command::JumpToColumn(position.get() - 1))
}

pub(super) fn jump_sequence(ctx: &Context, argument: &str) -> Result<Command> {
    let session = ctx.session()?;
    let row = row_named(session, session.layout().main(), argument)?;
    Ok(Command::JumpToSequence(row))
}

pub(super) fn jump_feature(ctx: &Context, argument: &str) -> Result<Command> {
    let gff = ctx
        .gff
        .context("No GFF file is loaded. Load one with load-gff")?;
    let index = gff
        .features
        .iter()
        .position(|feature| feature.name == argument)
        .with_context(|| format!("no feature named '{argument}'"))?;
    Ok(Command::JumpToFeature(index))
}

pub(super) fn pin_sequence(ctx: &Context, argument: &str) -> Result<Command> {
    let session = ctx.session()?;
    let row = row_named(session, session.layout().main(), argument)?;
    Ok(Command::PinSequence(row))
}

pub(super) fn unpin_sequence(ctx: &Context, argument: &str) -> Result<Command> {
    let session = ctx.session()?;
    let row = row_named(session, session.state().pinned(), argument)?;
    Ok(Command::UnpinSequence(row))
}

pub(super) fn filter_rows(_: &Context, argument: Option<&str>) -> Result<Command> {
    Ok(Command::SetRowFilter(argument.map(str::to_string)))
}

pub(super) fn filter_gaps(_: &Context, argument: &str) -> Result<Command> {
    Ok(Command::SetGapFilter(parse_percentage(argument)?))
}

pub(super) fn filter_constant(_: &Context, argument: &str) -> Result<Command> {
    Ok(Command::SetConstantFilter(parse_percentage(argument)?))
}

pub(super) fn set_reference(ctx: &Context, argument: &str) -> Result<Command> {
    let session = ctx.session()?;
    let row = row_named(session, session.layout().rows(), argument)?;
    Ok(Command::SetReference(Some(row)))
}

pub(super) fn diff_mode(_: &Context, argument: &str) -> Result<Command> {
    Ok(Command::SetDiffMode(parse_choice(argument, "diff mode")?))
}

pub(super) fn consensus_method(_: &Context, argument: &str) -> Result<Command> {
    Ok(Command::SetConsensusMethod(parse_choice(
        argument,
        "consensus method",
    )?))
}

pub(super) fn translation_frame(_: &Context, argument: &str) -> Result<Command> {
    Ok(Command::SetTranslationFrame(parse_choice(
        argument,
        "reading frame",
    )?))
}

pub(super) fn theme(_: &Context, argument: &str) -> Result<Command> {
    Ok(Command::SetTheme(parse_choice(argument, "theme")?))
}

pub(super) fn sequence_type(_: &Context, argument: &str) -> Result<Command> {
    Ok(Command::SetActiveType(parse_choice(
        argument,
        "sequence type",
    )?))
}

pub(super) fn load_alignment(_: &Context, argument: &str) -> Result<Command> {
    Ok(Command::LoadAlignment(argument.to_string()))
}

pub(super) fn load_gff(_: &Context, argument: &str) -> Result<Command> {
    Ok(Command::LoadGff {
        path: argument.to_string(),
    })
}

fn parse_choice<T: FromStr>(argument: &str, what: &str) -> Result<T> {
    argument
        .parse()
        .map_err(|_| format_err!("unknown {what} '{argument}'"))
}

fn parse_percentage(argument: &str) -> Result<Option<f64>> {
    let percent = argument
        .parse::<f64>()
        .ok()
        .filter(|percent| (0.0..=100.0).contains(percent))
        .with_context(|| format!("expected a percentage from 0 to 100, got '{argument}'"))?;
    Ok((percent != 0.0).then_some(percent / 100.0))
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;
    use crate::{
        config::theme::ThemeId,
        core::session::DiffMode,
        test_utils::{genes, pinned_session, with_context},
    };

    type Runner = fn(&Context, &str) -> Result<Command>;

    fn run(runner: Runner, argument: &str) -> Result<Command> {
        let session = pinned_session(4, &[1]);
        let gff = genes(&[0..3, 3..6]);
        with_context(Some(&session), Some(&gff), |ctx| runner(ctx, argument))
    }

    #[rstest]
    #[case::jump_position(jump_position, "1", Command::JumpToColumn(0))]
    #[case::jump_sequence(jump_sequence, "s2", Command::JumpToSequence(2))]
    #[case::jump_feature(jump_feature, "gene1", Command::JumpToFeature(1))]
    #[case::pin_sequence(pin_sequence, "s0", Command::PinSequence(0))]
    #[case::unpin_sequence(unpin_sequence, "s1", Command::UnpinSequence(1))]
    #[case::set_reference_main(set_reference, "s3", Command::SetReference(Some(3)))]
    #[case::set_reference_pinned(set_reference, "s1", Command::SetReference(Some(1)))]
    #[case::filter_gaps(filter_gaps, "25", Command::SetGapFilter(Some(0.25)))]
    #[case::filter_gaps_zero_clears(filter_gaps, "0", Command::SetGapFilter(None))]
    #[case::filter_constant(filter_constant, "100", Command::SetConstantFilter(Some(1.0)))]
    #[case::diff_mode(diff_mode, "reference", Command::SetDiffMode(DiffMode::Reference))]
    #[case::consensus_method(
        consensus_method,
        "majority",
        Command::SetConsensusMethod(libmsa::ConsensusMethod::Majority)
    )]
    #[case::translation_frame(
        translation_frame,
        "2",
        Command::SetTranslationFrame(libmsa::ReadingFrame::Frame2)
    )]
    #[case::theme(theme, "tokyo-night", Command::SetTheme(ThemeId::TokyoNight))]
    #[case::sequence_type(
        sequence_type,
        "protein",
        Command::SetActiveType(libmsa::AlignmentType::Protein)
    )]
    fn runner_works(#[case] runner: Runner, #[case] argument: &str, #[case] expected: Command) {
        assert_eq!(run(runner, argument).unwrap(), expected);
    }

    #[rstest]
    #[case::jump_position_zero(jump_position, "0")]
    #[case::jump_position_not_a_number(jump_position, "x")]
    #[case::jump_sequence_pinned(jump_sequence, "s1")]
    #[case::jump_sequence_unknown(jump_sequence, "nope")]
    #[case::jump_feature_unknown(jump_feature, "nope")]
    #[case::pin_sequence_already_pinned(pin_sequence, "s1")]
    #[case::unpin_sequence_not_pinned(unpin_sequence, "s0")]
    #[case::filter_gaps_above_range(filter_gaps, "101")]
    #[case::filter_gaps_below_range(filter_gaps, "-1")]
    #[case::filter_gaps_not_a_number(filter_gaps, "NaN")]
    #[case::unknown_choice(diff_mode, "sideways")]
    fn runner_rejects(#[case] runner: Runner, #[case] argument: &str) {
        assert!(run(runner, argument).is_err());
    }

    #[rstest]
    #[case::jump_sequence(jump_sequence)]
    #[case::pin_sequence(pin_sequence)]
    #[case::unpin_sequence(unpin_sequence)]
    #[case::set_reference(set_reference)]
    fn runner_rejects_without_alignment(#[case] runner: Runner) {
        let result = with_context(None, None, |ctx| runner(ctx, "s0"));

        assert!(result.is_err());
    }

    #[test]
    fn jump_feature_rejects_without_gff() {
        let session = pinned_session(2, &[]);
        let result = with_context(Some(&session), None, |ctx| jump_feature(ctx, "gene0"));

        assert!(result.is_err());
    }
}
