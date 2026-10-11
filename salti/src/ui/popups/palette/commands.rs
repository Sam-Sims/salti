use anyhow::Result;
use libmsa::{AlignmentType, ConsensusMethod, ReadingFrame};

use super::{completers, runners};
use crate::{
    command::Command, config::theme::ThemeId, core::session::DiffMode, ui::popups::Context,
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
            run: runners::jump_position,
            completer: None,
        },
    },
    PaletteCommand {
        name: "jump-sequence",
        aliases: &["js"],
        doc: "Jump to a sequence by name.",
        action: Action::Required {
            run: runners::jump_sequence,
            completer: Some(completers::main_sequences),
        },
    },
    PaletteCommand {
        name: "jump-feature",
        aliases: &["jf"],
        doc: "Jump to a GFF feature by name.",
        action: Action::Required {
            run: runners::jump_feature,
            completer: Some(completers::features),
        },
    },
    PaletteCommand {
        name: "pin-sequence",
        aliases: &[],
        doc: "Pin a sequence to the top of the alignment pane.",
        action: Action::Required {
            run: runners::pin_sequence,
            completer: Some(completers::main_sequences),
        },
    },
    PaletteCommand {
        name: "unpin-sequence",
        aliases: &[],
        doc: "Remove a pinned sequence from the pinned group.",
        action: Action::Required {
            run: runners::unpin_sequence,
            completer: Some(completers::pinned_sequences),
        },
    },
    PaletteCommand {
        name: "filter-rows",
        aliases: &[],
        doc: "Filter sequences by regular expression.",
        action: Action::Optional {
            run: runners::filter_rows,
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
            run: runners::filter_gaps,
            completer: Some(completers::gap_percentages),
        },
    },
    PaletteCommand {
        name: "filter-constant",
        aliases: &[],
        doc: "Hide columns when a counted position reaches the given percentage. Gaps and unknowns are ignored. Use 0 to disable it.",
        action: Action::Required {
            run: runners::filter_constant,
            completer: Some(completers::constant_percentages),
        },
    },
    PaletteCommand {
        name: "set-reference",
        aliases: &[],
        doc: "Set the reference sequence used for diffs.",
        action: Action::Required {
            run: runners::set_reference,
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
            run: runners::diff_mode,
            completer: Some(completers::choices::<DiffMode>),
        },
    },
    PaletteCommand {
        name: "load-alignment",
        aliases: &["load"],
        doc: "Load an alignment file using a file path argument.",
        action: Action::Required {
            run: runners::load_alignment,
            completer: Some(completers::filename),
        },
    },
    PaletteCommand {
        name: "set-consensus-method",
        aliases: &[],
        doc: "Set the consensus method used for the consensus row.",
        action: Action::Required {
            run: runners::consensus_method,
            completer: Some(completers::choices::<ConsensusMethod>),
        },
    },
    PaletteCommand {
        name: "set-translation-frame",
        aliases: &[],
        doc: "Set the reading frame used by the translation overlay and the protein view.",
        action: Action::Required {
            run: runners::translation_frame,
            completer: Some(completers::choices::<ReadingFrame>),
        },
    },
    PaletteCommand {
        name: "set-theme",
        aliases: &[],
        doc: "Set the active theme.",
        action: Action::Required {
            run: runners::theme,
            completer: Some(completers::choices::<ThemeId>),
        },
    },
    PaletteCommand {
        name: "set-sequence-type",
        aliases: &[],
        doc: "Override sequence type detection for rendering.",
        action: Action::Required {
            run: runners::sequence_type,
            completer: Some(completers::choices::<AlignmentType>),
        },
    },
    PaletteCommand {
        name: "load-gff",
        aliases: &[],
        doc: "Load a GFF annotation file to display features above the alignment.",
        action: Action::Required {
            run: runners::load_gff,
            completer: Some(completers::filename),
        },
    },
];
