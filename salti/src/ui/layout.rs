use std::ops::Range;

use ratatui::{
    layout::{Rect, Spacing},
    macros::{horizontal, vertical},
};

use crate::core::layout::Layout;

/// fixed height (rows) for the bottom consensus pane.
/// the remaining vertical space is used for the alignment pane.
const CONSENSUS_PANE_HEIGHT_ROWS: u16 = 5;
/// fixed height (rows) for the alignment ruler above sequence rows.
const RULER_HEIGHT_ROWS: u16 = 2;
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

pub fn screen_rows(
    layout: &Layout,
    window: &Window,
) -> impl Iterator<Item = Option<usize>> + use<> {
    let offset = layout.pinned();
    let divider = (!window.pinned.is_empty()).then_some(None);
    let main = window.rows.clone().map(move |i| Some(offset + i));
    window.pinned.clone().map(Some).chain(divider).chain(main)
}

#[derive(Debug, Clone, Default)]
pub struct Window {
    pub pinned: Range<usize>,
    pub rows: Range<usize>,
    pub columns: Range<usize>,
    pub names: Range<usize>,
}

pub fn fit(offset: &mut usize, visible: usize, total: usize) -> Range<usize> {
    *offset = (*offset).min(total.saturating_sub(visible));
    *offset..(*offset + visible).min(total)
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
        let gff_pane_rows = ratatui::widgets::Block::bordered().inner(gff_pane_area);

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

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;
    use crate::test_utils::pinned_session;

    #[rstest]
    #[case::no_pins(&[], 0..0, 1..3, &[Some(1), Some(2)])]
    #[case::pinned_then_divider_then_main(&[5, 4], 0..2, 0..2, &[Some(0), Some(1), None, Some(2), Some(3)])]
    #[case::scrolled(&[5, 4], 1..2, 1..3, &[Some(1), None, Some(3), Some(4)])]
    #[case::no_pinned_space_no_divider(&[5, 4], 0..0, 0..2, &[Some(2), Some(3)])]
    fn screen_rows_works(
        #[case] pinned: &[usize],
        #[case] window_pinned: Range<usize>,
        #[case] window_rows: Range<usize>,
        #[case] expected: &[Option<usize>],
    ) {
        let session = pinned_session(6, pinned);
        let window = Window {
            pinned: window_pinned,
            rows: window_rows,
            ..Window::default()
        };

        assert_eq!(
            screen_rows(session.layout(), &window).collect::<Vec<_>>(),
            expected
        );
    }

    #[rstest]
    #[case::from_start(0, 5, 10, 0..5)]
    #[case::scrolled(3, 5, 10, 3..8)]
    #[case::clamped_to_last_page(8, 5, 10, 5..10)]
    #[case::fewer_than_visible(2, 5, 3, 0..3)]
    fn fit_works(
        #[case] offset: usize,
        #[case] visible: usize,
        #[case] total: usize,
        #[case] expected: Range<usize>,
    ) {
        let mut offset = offset;

        assert_eq!(fit(&mut offset, visible, total), expected);
        assert_eq!(offset, expected.start);
    }

    #[rstest]
    #[case::no_items(4, 5, 0)]
    #[case::no_space(4, 0, 10)]
    fn fit_is_empty(#[case] offset: usize, #[case] visible: usize, #[case] total: usize) {
        let mut offset = offset;

        assert!(fit(&mut offset, visible, total).is_empty());
    }

    #[rstest]
    #[case::no_features(0, 0)]
    #[case::adds_borders_and_navigation_row(2, 5)]
    fn gff_pane_height_works(#[case] feature_rows: usize, #[case] expected: u16) {
        assert_eq!(gff_pane_height(feature_rows), expected);
    }
}
