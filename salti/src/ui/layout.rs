use ratatui::{
    layout::{Rect, Spacing},
    macros::{horizontal, vertical},
};

/// fixed height (rows) for the bottom consensus pane.
/// the remaining vertical space is used for the alignment pane.
const CONSENSUS_PANE_HEIGHT_ROWS: u16 = 5;
/// fixed height (rows) for the alignment ruler above sequence rows.
pub const RULER_HEIGHT_ROWS: u16 = 2;
/// width percentage for the left sequence ID pane (used in alignment and consensus panes).
/// the remaining horizontal space is used for sequence content.
const SEQUENCE_ID_PANE_WIDTH_PERCENT: u16 = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AlignmentHeaderLayout {
    pub local_feature_rows: u16,
    pub ruler_rows: u16,
}

impl AlignmentHeaderLayout {
    pub(crate) fn without_features() -> Self {
        Self {
            local_feature_rows: 0,
            ruler_rows: RULER_HEIGHT_ROWS,
        }
    }

    pub(crate) fn with_features(local_feature_rows: u16) -> Self {
        Self {
            local_feature_rows,
            ruler_rows: RULER_HEIGHT_ROWS,
        }
    }

    pub(crate) fn height(self) -> u16 {
        self.local_feature_rows.saturating_add(self.ruler_rows)
    }
}

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
    pub alignment_header: AlignmentHeaderLayout,
    pub consensus_sequence_id_pane: Rect,
    pub consensus_alignment_pane: Rect,
    pub gff_info_pane: Rect,
    pub gff_pane: Rect,
    pub gff_pane_rows: Rect,
}

impl AppLayout {
    pub fn new(
        content_area: Rect,
        gff_height: u16,
        alignment_header: AlignmentHeaderLayout,
    ) -> Self {
        let [gff_area, main_area] =
            content_area.layout(&vertical![==gff_height, *=1].spacing(Spacing::Overlap(1)));

        let [alignment_area, consensus_area] = main_area
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
        let inner_alignment_pane = ratatui::widgets::Block::bordered().inner(alignment_pane_area);
        let [_, _, sequence_rows_area] = inner_alignment_pane.layout(&vertical![
                ==alignment_header.local_feature_rows,
                ==alignment_header.ruler_rows,
                *=1
        ]);

        let [gff_info_pane_area, gff_pane_area] = gff_area.layout(
            &horizontal![==SEQUENCE_ID_PANE_WIDTH_PERCENT%, *=1].spacing(Spacing::Overlap(1)),
        );
        let gff_pane_rows = if gff_pane_area.width > 2 && gff_pane_area.height > 2 {
            ratatui::widgets::Block::bordered().inner(gff_pane_area)
        } else {
            Rect::default()
        };

        Self {
            sequence_id_pane: sequence_id_pane_area,
            alignment_pane: alignment_pane_area,
            alignment_pane_sequence_rows: sequence_rows_area,
            alignment_header,
            consensus_sequence_id_pane: consensus_sequence_id_pane_area,
            consensus_alignment_pane: consensus_alignment_pane_area,
            gff_info_pane: gff_info_pane_area,
            gff_pane: gff_pane_area,
            gff_pane_rows,
        }
    }
}

pub fn gff_pane_height(feature_row_count: usize) -> u16 {
    if feature_row_count == 0 {
        return 0;
    }
    let inner = u16::try_from(feature_row_count).unwrap_or(u16::MAX.saturating_sub(3));
    inner.saturating_add(3)
}
