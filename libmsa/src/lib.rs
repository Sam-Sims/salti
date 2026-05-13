mod alignment_type;
mod data;
pub mod detection;
mod error;
mod filter;
mod metrics;
mod model;
mod projection;
mod translation;

pub use alignment_type::AlignmentType;
pub use data::RawSequence;
pub use detection::DetectionOptions;
pub use error::AlignmentError;
pub use filter::FilterBuilder;
pub use metrics::{ColumnSummary, ConsensusMethod};
pub use model::{Alignment, RowView};
pub use translation::{
    ReadingFrame, TranslatedAlignment, TranslatedSequenceView, TranslationTable,
};
