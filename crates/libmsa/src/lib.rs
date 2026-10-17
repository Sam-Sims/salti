#![allow(clippy::must_use_candidate)]

mod alignment_type;
mod column_counts;
mod error;
mod grid;
mod metrics;
mod model;
pub mod residue;
mod translation;

pub use alignment_type::AlignmentType;
pub use column_counts::{ColumnCounts, ConsensusMethod};
pub use error::AlignmentError;
pub use grid::Grid;
pub use metrics::ColumnFilter;
pub use model::{Alignment, Sequence};
pub use translation::ReadingFrame;
