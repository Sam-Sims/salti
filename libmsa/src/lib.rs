#![allow(clippy::must_use_candidate)]

mod alignment_type;
mod counts;
mod data;
pub mod detection;
mod error;
mod filter;
mod grid;
mod metrics;
mod model;
mod projection;
pub mod residue;
mod translation;

pub use alignment_type::AlignmentType;
pub use data::RawSequence;
pub use detection::DetectionOptions;
pub use error::AlignmentError;
pub use filter::ColumnFilter;
pub use grid::Grid;
pub use metrics::{ColumnSummary, ConsensusMethod};
pub use model::{Alignment, RowView};
pub use translation::{ReadingFrame, TranslationTable};
