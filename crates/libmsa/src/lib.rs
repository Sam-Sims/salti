#![allow(clippy::must_use_candidate)]

mod alignment_type;
mod counts;
mod error;
mod grid;
mod metrics;
mod model;
pub mod residue;
mod translation;

pub use alignment_type::AlignmentType;
pub use error::AlignmentError;
pub use grid::Grid;
pub use metrics::{ColumnFilter, ColumnSummary, ConsensusMethod};
pub use model::{Alignment, Sequence};
pub use translation::ReadingFrame;
