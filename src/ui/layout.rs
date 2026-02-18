use ratatui::layout::{Rect, Spacing};
use ratatui::macros::{horizontal, vertical};
use ratatui::widgets::Block;

/// fixed height (rows) for the bottom consensus pane.
/// the remaining vertical space is used for the alignment pane.
const CONSENSUS_PANE_HEIGHT_ROWS: u16 = 6;
/// width percentage for the left sequence ID pane (used in alignment and consensus panes).
/// the remaining horizontal space is used for sequence content.
const SEQUENCE_ID_PANE_WIDTH_PERCENT: u16 = 20;

#[derive(Debug, Clone, Copy)]
pub struct FrameLayout {
    pub top_status_area: Rect,
    pub overlay_area: Rect,
    pub content_area: Rect,
    pub bottom_status_area: Rect,
    pub input_area: Rect,
}

impl FrameLayout {
    #[must_use]
    pub fn new(terminal_area: Rect) -> Self {
        let [non_input_area, input_area] = terminal_area.layout(&vertical![*=1, ==1]);
        let [top_status_area, overlay_area] = non_input_area.layout(&vertical![==1, *=1]);
        let [content_area, bottom_status_area] = overlay_area.layout(&vertical![*=1, ==1]);

        Self {
            top_status_area,
            overlay_area,
            content_area,
            bottom_status_area,
            input_area,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AppLayout {
    pub sequence_id_pane_area: Rect,
    pub alignment_pane_area: Rect,
    pub consensus_sequence_id_pane_area: Rect,
    pub consensus_alignment_pane_area: Rect,
}

impl AppLayout {
    #[must_use]
    pub fn new(area: Rect) -> Self {
        let content_area = area;

        let [alignment_area, consensus_area] = content_area
            .layout(&vertical![*=1, ==CONSENSUS_PANE_HEIGHT_ROWS].spacing(Spacing::Overlap(1)));

        let [sequence_id_pane_area, alignment_pane_area] = alignment_area.layout(
            &horizontal![==SEQUENCE_ID_PANE_WIDTH_PERCENT%, *=1].spacing(Spacing::Overlap(1)),
        );
        let [
            consensus_sequence_id_pane_area,
            consensus_alignment_pane_area,
        ] = consensus_area.layout(
            &horizontal![==SEQUENCE_ID_PANE_WIDTH_PERCENT%, *=1].spacing(Spacing::Overlap(1)),
        );

        Self {
            sequence_id_pane_area,
            alignment_pane_area,
            consensus_sequence_id_pane_area,
            consensus_alignment_pane_area,
        }
    }

    #[must_use]
    pub fn sequence_id_pane_inner_area(&self) -> Rect {
        Block::bordered().inner(self.sequence_id_pane_area)
    }

    #[must_use]
    pub fn alignment_pane_sequence_rows_area(&self) -> Rect {
        let [_, sequence_rows_area] = Block::bordered()
            .inner(self.alignment_pane_area)
            .layout(&vertical![==2, *=1]);
        sequence_rows_area
    }
}
