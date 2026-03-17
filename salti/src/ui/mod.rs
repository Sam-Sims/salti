pub mod alignment_pane;
pub mod consensus_pane;
pub mod frame;
pub mod layout;
pub mod render;
pub mod rows;
pub mod selection;
pub mod sequence_id_pane;
pub mod ui_state;
pub mod utils;

use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisibleSequence {
    pub sequence_id: usize,
    pub sequence_name: Arc<str>,
}

pub use alignment_pane::render_alignment_pane;
pub use consensus_pane::render_consensus_pane;
pub use frame::render_frame;
pub use render::render;
pub use sequence_id_pane::render_sequence_id_pane;
pub use ui_state::{MouseSelection, MouseState, UiAction, UiState};
