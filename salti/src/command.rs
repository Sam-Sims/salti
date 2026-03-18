use crate::config::theme::ThemeId;
use crate::core::model::DiffMode;
use crate::ui::notification::Notification;

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Quit,
    OpenCommandPalette,
    CloseOverlay,
    ToggleMinimap,
    SetTheme(ThemeId),
    ShowNotification(Notification),
    LoadFile { input: String },
    CheckForUpdate { show_success_message: bool },
    ScrollDown { amount: usize },
    ScrollUp { amount: usize },
    ScrollLeft { amount: usize },
    ScrollRight { amount: usize },
    ScrollNamesLeft { amount: usize },
    ScrollNamesRight { amount: usize },
    JumpToPosition(usize),
    JumpToSequence(usize),
    JumpToStart,
    JumpToEnd,
    SetFilter(String),
    SetGapFilter(Option<f32>),
    ClearFilter,
    PinSequence(usize),
    UnpinSequence(usize),
    SetReference(usize),
    ClearReference,
    SetConsensusMethod(libmsa::ConsensusMethod),
    SetActiveType(libmsa::AlignmentType),
    SetTranslationFrame(libmsa::ReadingFrame),
    SetDiffMode(DiffMode),
    ToggleTranslationView,
}
