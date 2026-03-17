use crate::alignment_type::AlignmentType;
use thiserror::Error;

use crate::translation::ReadingFrame;

#[derive(Debug, Clone, PartialEq, Error)]
pub enum AlignmentError {
    /// The alignment contains no sequences.
    #[error("alignment contains no sequences")]
    Empty,
    /// A sequence in the alignment is empty.
    #[error("sequence '{id}' is empty")]
    EmptySequence { id: String },
    /// A sequence has a different length from the rest of the alignment.
    #[error("sequence '{id}' has width {actual}, expected {expected}")]
    LengthMismatch {
        expected: usize,
        actual: usize,
        id: String,
    },
    /// A requested column index lies outside the alignment.
    #[error("column index {index} is out of bounds for alignment length {length}")]
    ColumnOutOfBounds { index: usize, length: usize },
    /// A requested row index lies outside the alignment.
    #[error("row index {index} is out of bounds for alignment row count {row_count}")]
    RowOutOfBounds { index: usize, row_count: usize },
    /// A requested row index appears more than once in a row subset.
    #[error("row index {index} appears more than once in the row subset")]
    DuplicateRowIndex { index: usize },
    /// Conservation scoring is not defined for this alignment type.
    #[error("conservation is not defined for this alignment type")]
    ConservationUndefined,
    /// A requested range contains no columns.
    #[error("range is empty")]
    EmptyRange,
    /// A requested row subset contains no rows.
    #[error("row subset is empty")]
    EmptyRowSubset,
    /// Translation produced no residues for the requested reading frame.
    #[error("translation in {frame:?} is empty for alignment length {length}")]
    TranslationEmpty { frame: ReadingFrame, length: usize },
    /// The requested operation is not available for the active alignment kind.
    #[error("operation '{operation}' is not supported for alignment kind {kind:?}")]
    UnsupportedOperation {
        operation: &'static str,
        kind: AlignmentType,
    },
    /// An alignment kind string was not one of the supported values.
    #[error("invalid alignment type: expected `dna`, `protein`, or `generic`")]
    InvalidAlignmentType,
    /// Detection options used a threshold outside the supported finite range.
    #[error("invalid classification threshold: {0} (expected a finite value in 0.0..=1.0)")]
    InvalidClassificationThreshold(f32),
    /// Parsing alignment data failed with the attached message.
    #[error("failed to parse alignment: {0}")]
    Parse(String),
    /// A gap-filter threshold was outside the supported finite range.
    #[error("invalid gap fraction: {0} (expected a finite value in 0.0..=1.0)")]
    InvalidGapFraction(f32),
    /// A regex row-name filter could not be compiled.
    #[error("invalid regex '{pattern}'")]
    InvalidRegex {
        pattern: String,
        #[source]
        source: regex::Error,
    },
}
