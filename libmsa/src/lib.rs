#![allow(clippy::must_use_candidate)]

mod alignment_type;
mod counts;
mod data;
mod detection;
mod error;
mod filter;
mod grid;
mod metrics;
mod model;
pub mod residue;
mod translation;

pub use alignment_type::AlignmentType;
pub use data::RawSequence;
pub use error::AlignmentError;
pub use filter::ColumnFilter;
pub use grid::Grid;
pub use metrics::{ColumnSummary, ConsensusMethod};
pub use model::Alignment;
pub use translation::ReadingFrame;
