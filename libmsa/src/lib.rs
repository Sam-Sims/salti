pub mod alignment_type;
mod data;
pub mod detection;
pub mod error;
mod filter;
mod metrics;
mod model;
mod projection;
pub mod translation;

pub use alignment_type::AlignmentType;
pub use data::{RawSequence, Sequence};
pub use detection::DetectionOptions;
pub use error::AlignmentError;
pub use filter::FilterBuilder;
pub use metrics::{ColumnSummary, ConsensusMethod};
pub use model::{Alignment, SequenceView};
pub use translation::{
    ReadingFrame, TranslatedAlignment, TranslatedSequenceView, TranslationTable,
};
