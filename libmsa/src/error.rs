use thiserror::Error;

/// Errors from building an [`Alignment`](crate::Alignment) or parsing an [`AlignmentType`](crate::AlignmentType)
#[derive(Debug, Clone, PartialEq, Error)]
pub enum AlignmentError {
    /// There are no sequences
    #[error("alignment contains no sequences")]
    Empty,
    /// The first sequence has no residues
    #[error("sequence '{id}' is empty")]
    EmptySequence { id: String },
    /// A sequence has a byte outside ASCII
    #[error("sequence '{id}' contains a non-ASCII byte")]
    NonAsciiSequence { id: String },
    /// A sequence's length differs from the first sequence's
    #[error("sequence '{id}' has width {actual}, expected {expected}")]
    LengthMismatch {
        expected: usize,
        actual: usize,
        id: String,
    },
    /// The string is not `dna`, `protein` or `generic`
    #[error("invalid alignment type: expected `dna`, `protein`, or `generic`")]
    InvalidAlignmentType,
}
