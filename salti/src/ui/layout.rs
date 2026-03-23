use ratatui::layout::{Rect, Spacing};
use ratatui::macros::{horizontal, vertical};

/// fixed height (rows) for the bottom consensus pane.
/// the remaining vertical space is used for the alignment pane.
const CONSENSUS_PANE_HEIGHT_ROWS: u16 = 5;
/// fixed height (rows) for the alignment ruler above sequence rows.
pub const RULER_HEIGHT_ROWS: u16 = 2;
/// width percentage for the left sequence ID pane (used in alignment and consensus panes).
/// the remaining horizontal space is used for sequence content.
const SEQUENCE_ID_PANE_WIDTH_PERCENT: u16 = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PinnedSectionLayout {
    pub pinned_rendered: usize,
    pub divider_height: usize,
    pub scrollable_height: usize,
}

pub fn pinned_section_layout(pinned_count: usize, available_height: usize) -> PinnedSectionLayout {
    if available_height == 0 {
        return PinnedSectionLayout {
            pinned_rendered: 0,
            divider_height: 0,
            scrollable_height: 0,
        };
    }

    let pinned_rendered = pinned_count.min(available_height.saturating_sub(1));
    let divider_height = usize::from(pinned_rendered > 0);
    let scrollable_height = available_height.saturating_sub(pinned_rendered + divider_height);

    PinnedSectionLayout {
        pinned_rendered,
        divider_height,
        scrollable_height,
    }
}
#[derive(Debug, Clone, Copy)]
pub struct FrameLayout {
    pub top_status_area: Rect,
    pub overlay_area: Rect,
    pub content_area: Rect,
    pub bottom_status_area: Rect,
    pub input_area: Rect,
}

impl FrameLayout {
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

#[derive(Debug, Clone, Copy)]
pub struct AppLayout {
    pub sequence_id_pane: Rect,
    pub alignment_pane: Rect,
    pub alignment_pane_sequence_rows: Rect,
    pub consensus_sequence_id_pane: Rect,
    pub consensus_alignment_pane: Rect,
}

impl AppLayout {
    pub fn new(content_area: Rect) -> Self {
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
        let [_, sequence_rows_area] = ratatui::widgets::Block::bordered()
            .inner(alignment_pane_area)
            .layout(&vertical![==RULER_HEIGHT_ROWS, *=1]);

        Self {
            sequence_id_pane: sequence_id_pane_area,
            alignment_pane: alignment_pane_area,
            alignment_pane_sequence_rows: sequence_rows_area,
            consensus_sequence_id_pane: consensus_sequence_id_pane_area,
            consensus_alignment_pane: consensus_alignment_pane_area,
        }
    }
}
