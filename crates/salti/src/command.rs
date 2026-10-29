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
    LoadAlignment { input: String },
    LoadGff { path: String },
    CheckForUpdate,

    Scroll(Direction, Distance),
    ScrollNamesLeft { amount: usize },
    ScrollNamesRight { amount: usize },
    JumpToVisibleColumn(usize),
    JumpToColumn(usize),
    JumpToFeature(usize),
    JumpToSequence(usize),

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

#[derive(Debug, Clone, PartialEq)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

/// how far a scroll moves, in columns or rows
#[derive(Debug, Clone, PartialEq)]
pub enum Distance {
    Amount(usize),
    Step,
    Page,
    Edge,
}
