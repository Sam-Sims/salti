use super::{
    command_runners::{
        run_check_update, run_clear_all_filters, run_clear_reference, run_consensus_method,
        run_diff_mode, run_filter_constant, run_filter_gaps, run_filter_rows, run_jump_feature,
        run_jump_position, run_jump_sequence, run_load_alignment, run_load_gff, run_pin_sequence,
        run_quit, run_set_active_type, run_set_reference, run_theme, run_toggle_protein_view,
        run_toggle_translation_overlay, run_translation_frame, run_unpin_sequence,
    },
    command_spec::{PaletteCommand, StaticCommand, TypableCommand},
    completers,
};

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
///   `ui/layers/palette/command_runners`
pub(super) const COMMAND_SPECS: &[PaletteCommand] = &[
    PaletteCommand::Typable(TypableCommand {
        name: "jump-position",
        help_text: "Jump to an alignment position (1 based), skipping forward to the next visible column if needed.",
        aliases: &["jp"],
        completer: None,
        static_candidates: &[],
        run: run_jump_position,
    }),
    PaletteCommand::Typable(TypableCommand {
        name: "jump-sequence",
        help_text: "Jump to a sequence by name.",
        aliases: &["js"],
        completer: Some(completers::main_sequences),
        static_candidates: &[],
        run: run_jump_sequence,
    }),
    PaletteCommand::Typable(TypableCommand {
        name: "jump-feature",
        help_text: "Jump to a GFF feature by name.",
        aliases: &["jf"],
        completer: Some(completers::features),
        static_candidates: &[],
        run: run_jump_feature,
    }),
    PaletteCommand::Typable(TypableCommand {
        name: "pin-sequence",
        help_text: "Pin a sequence to the top of the alignment pane.",
        aliases: &[],
        completer: Some(completers::main_sequences),
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
        name: "filter-rows",
        help_text: "Filter sequences by regular expression.",
        aliases: &[],
        completer: Some(completers::filter_matches),
        static_candidates: &[],
        run: run_filter_rows,
    }),
    PaletteCommand::Static(StaticCommand {
        name: "clear-all-filters",
        help_text: "Clear the row filter and both column filters.",
        aliases: &[],
        run: run_clear_all_filters,
    }),
    PaletteCommand::Typable(TypableCommand {
        name: "filter-gaps",
        help_text: "Hide columns with a gap percentage above the given threshold. Use 0 to disable it.",
        aliases: &[],
        completer: None,
        static_candidates: &["0", "5", "10", "25", "50"],
        run: run_filter_gaps,
    }),
    PaletteCommand::Typable(TypableCommand {
        name: "filter-constant",
        help_text: "Hide columns when a counted position reaches the given percentage. Gaps and unknowns are ignored. Use 0 to disable it.",
        aliases: &[],
        completer: None,
        static_candidates: &["0", "70", "90", "95", "100"],
        run: run_filter_constant,
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
        name: "toggle-translation-overlay",
        help_text: "Toggle the translation overlay, showing amino acids over their codons.",
        aliases: &[],
        run: run_toggle_translation_overlay,
    }),
    PaletteCommand::Static(StaticCommand {
        name: "toggle-protein-view",
        help_text: "Toggle the protein view, the DNA alignment translated in the active frame.",
        aliases: &[],
        run: run_toggle_protein_view,
    }),
    PaletteCommand::Static(StaticCommand {
        name: "check-update",
        help_text: "Check crates.io for a newer salti version.",
        aliases: &[],
        run: run_check_update,
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
        completer: Some(completers::diff_modes),
        static_candidates: &[],
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
        completer: Some(completers::consensus_methods),
        static_candidates: &[],
        run: run_consensus_method,
    }),
    PaletteCommand::Typable(TypableCommand {
        name: "set-translation-frame",
        help_text: "Set the reading frame used by the translation overlay and the protein view.",
        aliases: &[],
        completer: Some(completers::frames),
        static_candidates: &[],
        run: run_translation_frame,
    }),
    PaletteCommand::Typable(TypableCommand {
        name: "set-theme",
        help_text: "Set the active theme.",
        aliases: &[],
        completer: Some(completers::themes),
        static_candidates: &[],
        run: run_theme,
    }),
    PaletteCommand::Typable(TypableCommand {
        name: "set-sequence-type",
        help_text: "Override sequence type detection for rendering.",
        aliases: &[],
        completer: None,
        static_candidates: &["dna", "protein", "generic"],
        run: run_set_active_type,
    }),
    PaletteCommand::Typable(TypableCommand {
        name: "load-gff",
        help_text: "Load a GFF annotation file to display features above the alignment.",
        aliases: &[],
        completer: Some(completers::filename),
        static_candidates: &[],
        run: run_load_gff,
    }),
];
