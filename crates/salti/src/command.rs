use crate::{
    config::theme::ThemeChoice,
    session::DiffMode,
    ui::{notification::Notification, popups::minimap::MinimapView},
};

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Quit,
    OpenPalette,
    OpenMinimap,
    OpenKeybinds,
    ClosePopup,
    SetTheme(ThemeChoice),
    SetMinimapView(MinimapView),
    ShowNotification(Notification),
    LoadAlignment {
        input: String,
    },
    LoadGff {
        path: String,
    },
    CheckForUpdate,

    ScrollDown {
        amount: usize,
    },
    ScrollUp {
        amount: usize,
    },
    ScrollLeft {
        amount: usize,
    },
    ScrollRight {
        amount: usize,
    },
    ScrollNamesLeft {
        amount: usize,
    },
    ScrollNamesRight {
        amount: usize,
    },
    /// a position among the shown columns, after filters
    JumpToVisibleColumn(usize),
    /// a column in the alignment; jumps to the first shown column at or after it
    JumpToColumn(usize),
    /// an index into the loaded GFF's features
    JumpToFeature(usize),
    /// a row in the alignment
    JumpToSequence(usize),
    JumpToEnd,

    PinSequence(usize),
    UnpinSequence(usize),
    SetReference(Option<usize>),
    SetRowFilter(Option<String>),
    SetGapFilter(Option<f64>),
    SetConstantFilter(Option<f64>),
    ClearAllFilters,
    ToggleTranslationOverlay,
    ToggleProteinView,
    SetTranslationFrame(libmsa::ReadingFrame),
    SetActiveType(libmsa::AlignmentType),
    SetConsensusMethod(libmsa::ConsensusMethod),
    SetDiffMode(DiffMode),
    CycleDiffMode,
}
