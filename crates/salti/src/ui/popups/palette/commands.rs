use std::{num::NonZeroUsize, str::FromStr};

use anyhow::{Context as _, Result, format_err};
use libmsa::{AlignmentType, ConsensusMethod, ReadingFrame};

use super::completers;
use crate::{
    command::Command,
    session::{DiffMode, Session},
    ui::popups::Context,
};

pub(super) struct PaletteCommand {
    pub(super) name: &'static str,
    pub(super) aliases: &'static [&'static str],
    pub(super) doc: &'static str,
    pub(super) action: Action,
}

pub(super) enum Action {
    Fixed(Command),
    Optional {
        run: fn(&Context, Option<&str>) -> Result<Command>,
        completer: Option<Completer>,
    },
    Required {
        run: fn(&Context, &str) -> Result<Command>,
        completer: Option<Completer>,
    },
}

impl Action {
    pub(super) fn completer(&self) -> Option<Completer> {
        match self {
            Self::Fixed(_) => None,
            Self::Optional { completer, .. } | Self::Required { completer, .. } => *completer,
        }
    }
}

pub(super) type Completer = fn(&Context, &str) -> Vec<String>;

pub(super) const COMMANDS: &[PaletteCommand] = &[
    PaletteCommand {
        name: "jump-position",
        aliases: &["jp"],
        doc: "Jump to an alignment position (1 based), skipping forward to the next visible column if needed.",
        action: Action::Required {
            run: |_, argument| {
                let position = argument
                    .parse::<NonZeroUsize>()
                    .map_err(|_| format_err!("Expected a positive integer, got '{argument}'"))?;
                Ok(Command::JumpToColumn(position.get() - 1))
            },
            completer: None,
        },
    },
    PaletteCommand {
        name: "jump-sequence",
        aliases: &["js"],
        doc: "Jump to a sequence by name.",
        action: Action::Required {
            run: |ctx, argument| {
                let row = row_named(ctx, |session| session.layout().main(), argument)?;
                Ok(Command::JumpToSequence(row))
            },
            completer: Some(completers::main_sequences),
        },
    },
    PaletteCommand {
        name: "jump-feature",
        aliases: &["jf"],
        doc: "Jump to a GFF feature by name.",
        action: Action::Required {
            run: |ctx, argument| {
                let gff = ctx
                    .gff
                    .context("No GFF file is loaded. Load one with load-gff")?;
                let index = gff
                    .features
                    .iter()
                    .position(|feature| feature.name == argument)
                    .with_context(|| format!("No feature named '{argument}'"))?;
                Ok(Command::JumpToFeature(index))
            },
            completer: Some(completers::features),
        },
    },
    PaletteCommand {
        name: "pin-sequence",
        aliases: &[],
        doc: "Pin a sequence to the top of the alignment pane.",
        action: Action::Required {
            run: |ctx, argument| {
                let row = row_named(ctx, |session| session.layout().main(), argument)?;
                Ok(Command::PinSequence(row))
            },
            completer: Some(completers::main_sequences),
        },
    },
    PaletteCommand {
        name: "unpin-sequence",
        aliases: &[],
        doc: "Remove a pinned sequence from the pinned group.",
        action: Action::Required {
            run: |ctx, argument| {
                let row = row_named(ctx, |session| session.state().pinned(), argument)?;
                Ok(Command::UnpinSequence(row))
            },
            completer: Some(completers::pinned_sequences),
        },
    },
    PaletteCommand {
        name: "filter-rows",
        aliases: &[],
        doc: "Filter sequences by regular expression.",
        action: Action::Optional {
            run: |_, argument| Ok(Command::SetRowFilter(argument.map(str::to_string))),
            completer: Some(completers::filter_matches),
        },
    },
    PaletteCommand {
        name: "clear-all-filters",
        aliases: &[],
        doc: "Clear the row filter and both column filters.",
        action: Action::Fixed(Command::ClearAllFilters),
    },
    PaletteCommand {
        name: "filter-gaps",
        aliases: &[],
        doc: "Hide columns with a gap percentage above the given threshold. Use 0 to disable it.",
        action: Action::Required {
            run: |_, argument| Ok(Command::SetGapFilter(parse_percentage(argument)?)),
            completer: Some(completers::gap_percentages),
        },
    },
    PaletteCommand {
        name: "filter-constant",
        aliases: &[],
        doc: "Hide columns when a counted position reaches the given percentage. DNA and protein ignore gaps and unknowns. Use 0 to disable it.",
        action: Action::Required {
            run: |_, argument| Ok(Command::SetConstantFilter(parse_percentage(argument)?)),
            completer: Some(completers::constant_percentages),
        },
    },
    PaletteCommand {
        name: "set-reference",
        aliases: &[],
        doc: "Set the reference sequence used for diffs.",
        action: Action::Required {
            run: |ctx, argument| {
                let row = row_named(ctx, |session| session.layout().rows(), argument)?;
                Ok(Command::SetReference(Some(row)))
            },
            completer: Some(completers::sequences),
        },
    },
    PaletteCommand {
        name: "clear-reference",
        aliases: &[],
        doc: "Clear the active reference sequence.",
        action: Action::Fixed(Command::SetReference(None)),
    },
    PaletteCommand {
        name: "toggle-translation-overlay",
        aliases: &[],
        doc: "Toggle the translation overlay, showing amino acids over their codons.",
        action: Action::Fixed(Command::ToggleTranslationOverlay),
    },
    PaletteCommand {
        name: "toggle-protein-view",
        aliases: &[],
        doc: "Toggle the protein view, the DNA alignment translated in the active frame.",
        action: Action::Fixed(Command::ToggleProteinView),
    },
    PaletteCommand {
        name: "check-update",
        aliases: &[],
        doc: "Check crates.io for a newer salti version.",
        action: Action::Fixed(Command::CheckForUpdate),
    },
    PaletteCommand {
        name: "keybinds",
        aliases: &[],
        doc: "Show the key bindings, one tab per category.",
        action: Action::Fixed(Command::OpenKeybinds),
    },
    PaletteCommand {
        name: "quit",
        aliases: &["q"],
        doc: "Exit the application.",
        action: Action::Fixed(Command::Quit),
    },
    PaletteCommand {
        name: "set-diff-mode",
        aliases: &[],
        doc: "Set diff highlighting mode.",
        action: Action::Required {
            run: |_, argument| Ok(Command::SetDiffMode(parse_choice(argument, "diff mode")?)),
            completer: Some(completers::choices::<DiffMode>),
        },
    },
    PaletteCommand {
        name: "load-alignment",
        aliases: &["load"],
        doc: "Load an alignment file using a file path argument.",
        action: Action::Required {
            run: |_, argument| {
                Ok(Command::LoadAlignment {
                    input: argument.to_string(),
                })
            },
            completer: Some(completers::filename),
        },
    },
    PaletteCommand {
        name: "set-consensus-method",
        aliases: &[],
        doc: "Set the consensus method used for the consensus row.",
        action: Action::Required {
            run: |_, argument| {
                Ok(Command::SetConsensusMethod(parse_choice(
                    argument,
                    "consensus method",
                )?))
            },
            completer: Some(completers::choices::<ConsensusMethod>),
        },
    },
    PaletteCommand {
        name: "set-translation-frame",
        aliases: &[],
        doc: "Set the reading frame used by the translation overlay and the protein view.",
        action: Action::Required {
            run: |_, argument| {
                Ok(Command::SetTranslationFrame(parse_choice(
                    argument,
                    "reading frame",
                )?))
            },
            completer: Some(completers::choices::<ReadingFrame>),
        },
    },
    PaletteCommand {
        name: "set-theme",
        aliases: &[],
        doc: "Set the active theme. `auto` picks Everforest light or dark to match the terminal.",
        action: Action::Required {
            run: |_, argument| Ok(Command::SetTheme(parse_choice(argument, "theme")?)),
            completer: Some(completers::theme),
        },
    },
    PaletteCommand {
        name: "set-sequence-type",
        aliases: &[],
        doc: "Override sequence type detection for rendering.",
        action: Action::Required {
            run: |_, argument| {
                Ok(Command::SetActiveType(parse_choice(
                    argument,
                    "sequence type",
                )?))
            },
            completer: Some(completers::choices::<AlignmentType>),
        },
    },
    PaletteCommand {
        name: "load-gff",
        aliases: &[],
        doc: "Load a GFF annotation file to display features above the alignment.",
        action: Action::Required {
            run: |_, argument| {
                Ok(Command::LoadGff {
                    path: argument.to_string(),
                })
            },
            completer: Some(completers::filename),
        },
    },
];

fn row_named(ctx: &Context, rows: fn(&Session) -> &[usize], name: &str) -> Result<usize> {
    let session = ctx.session()?;
    rows(session)
        .iter()
        .copied()
        .find(|&row| session.base_alignment().id(row) == name)
        .with_context(|| format!("No sequence named '{name}'"))
}

fn parse_choice<T: FromStr>(argument: &str, what: &str) -> Result<T> {
    argument
        .parse()
        .map_err(|_| format_err!("Unknown {what} '{argument}'"))
}

fn parse_percentage(argument: &str) -> Result<Option<f64>> {
    let percent = argument
        .parse::<f64>()
        .ok()
        .filter(|percent| (0.0..=100.0).contains(percent))
        .with_context(|| format!("Expected a percentage from 0 to 100, got '{argument}'"))?;
    Ok((percent != 0.0).then_some(percent / 100.0))
}
