use crate::core::column_stats::ConsensusMethod;
use crate::core::parser::SequenceType;
use regex::Regex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffMode {
    Off,
    Reference,
    Consensus,
}

/// Represents an action that can be performed as part of the application.
#[derive(Debug)]
pub enum CoreAction {
    ScrollDown { amount: usize },
    ScrollUp { amount: usize },
    ScrollLeft { amount: usize },
    ScrollRight { amount: usize },
    ScrollNamesLeft { amount: usize },
    ScrollNamesRight { amount: usize },
    ClearFilter,
    SetFilter { pattern: String, regex: Regex },
    JumpToSequence(usize),
    JumpToPosition(usize),
    PinSequence(usize),
    UnpinSequence(usize),
    ClearReference,
    SetReference(usize),
    SetConsensusMethod(ConsensusMethod),
    SetSequenceType(SequenceType),
    SetTranslationFrame(u8),
    SetDiffMode(DiffMode),
    ToggleTranslationView,
}
