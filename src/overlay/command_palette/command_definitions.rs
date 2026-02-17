use super::command_runners::{
    run_clear_filter, run_clear_reference, run_consensus_method, run_diff_mode, run_filter,
    run_jump_position, run_jump_sequence, run_load_alignment, run_pin_sequence, run_quit,
    run_sequence_type, run_set_reference, run_theme, run_toggle_translation, run_translation_frame,
    run_unpin_sequence,
};
use super::command_spec::{PaletteCommand, StaticCommand, TypableCommand};
use super::completers;

/// Defines all commands available in the command palette.
///
/// Commands are split into two categories: static commands that run immediately when selected,
/// and typable commands that require the user to enter an argument before running.
///
/// Each command has a:
/// - `name`: the name used to invoke the command in the palette
/// - `help_text`: a description of what the command does, shown in the palette help box
/// - `aliases`: alternative names that can also be used to invoke the command
/// - `completer`: a function that provides autocompletion suggestions for the command's argument
///   These are only used for typable commands.
/// - `run`: the function that is called to execute the command. These are defined in
///   `overlay/command_palette/command_runners`
pub(super) const COMMAND_SPECS: &[PaletteCommand] = &[
    PaletteCommand::Typable(TypableCommand {
        name: "jump-position",
        help_text: "Jump to a position in the alignment (1 based).",
        aliases: &["jp"],
        completer: None,
        static_candidates: &[],
        run: run_jump_position,
    }),
    PaletteCommand::Typable(TypableCommand {
        name: "jump-sequence",
        help_text: "Jump to a sequence by name.",
        aliases: &["js"],
        completer: Some(completers::sequences),
        static_candidates: &[],
        run: run_jump_sequence,
    }),
    PaletteCommand::Typable(TypableCommand {
        name: "pin-sequence",
        help_text: "Pin a sequence to the top of the alignment pane.",
        aliases: &[],
        completer: Some(completers::sequences),
        static_candidates: &[],
        run: run_pin_sequence,
    }),
    PaletteCommand::Typable(TypableCommand {
        name: "unpin-sequence",
        help_text: "Remove a pinned sequence from the pinned group.",
        aliases: &[],
        completer: Some(completers::pinned_sequences),
        static_candidates: &[],
        run: run_unpin_sequence,
    }),
    PaletteCommand::Typable(TypableCommand {
        name: "set-filter",
        help_text: "Filter sequences by regular expression.",
        aliases: &[],
        completer: Some(completers::filter_matches),
        static_candidates: &[],
        run: run_filter,
    }),
    PaletteCommand::Static(StaticCommand {
        name: "clear-filter",
        help_text: "Clear the active filter.",
        aliases: &[],
        run: run_clear_filter,
    }),
    PaletteCommand::Typable(TypableCommand {
        name: "set-reference",
        help_text: "Set the reference sequence used for diffs.",
        aliases: &[],
        completer: Some(completers::sequences),
        static_candidates: &[],
        run: run_set_reference,
    }),
    PaletteCommand::Static(StaticCommand {
        name: "clear-reference",
        help_text: "Clear the active reference sequence.",
        aliases: &[],
        run: run_clear_reference,
    }),
    PaletteCommand::Static(StaticCommand {
        name: "toggle-translate",
        help_text: "Toggle the translation view.",
        aliases: &[],
        run: run_toggle_translation,
    }),
    PaletteCommand::Static(StaticCommand {
        name: "quit",
        help_text: "Exit the application.",
        aliases: &["q"],
        run: run_quit,
    }),
    PaletteCommand::Typable(TypableCommand {
        name: "set-diff-mode",
        help_text: "Set diff highlighting mode.",
        aliases: &[],
        completer: None,
        static_candidates: &["off", "reference", "consensus"],
        run: run_diff_mode,
    }),
    PaletteCommand::Typable(TypableCommand {
        name: "load-alignment",
        help_text: "Load an alignment file using a file path argument.",
        aliases: &["load"],
        completer: Some(completers::filename),
        static_candidates: &[],
        run: run_load_alignment,
    }),
    PaletteCommand::Typable(TypableCommand {
        name: "set-consensus-method",
        help_text: "Set the consensus method used for the consensus row.",
        aliases: &[],
        completer: None,
        static_candidates: &["majority", "majority-non-gap"],
        run: run_consensus_method,
    }),
    PaletteCommand::Typable(TypableCommand {
        name: "set-translation-frame",
        help_text: "Set the translation frame used for translation view.",
        aliases: &[],
        completer: None,
        static_candidates: &["1", "2", "3"],
        run: run_translation_frame,
    }),
    PaletteCommand::Typable(TypableCommand {
        name: "set-theme",
        help_text: "Set the active theme.",
        aliases: &[],
        completer: None,
        static_candidates: &["everforest-dark", "solarized-light", "tokyo-night", "terminal-default"],
        run: run_theme,
    }),
    PaletteCommand::Typable(TypableCommand {
        name: "set-sequence-type",
        help_text: "Override sequence type detection for rendering.",
        aliases: &[],
        completer: None,
        static_candidates: &["dna", "aa"],
        run: run_sequence_type,
    }),
];
