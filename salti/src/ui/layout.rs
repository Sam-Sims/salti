use std::ops::Range;

use ratatui::{
    layout::{Rect, Spacing},
    macros::{horizontal, vertical},
    widgets::Block,
};
use tracing::debug;

use crate::{
    core::{
        gff::Gff,
        layout::Layout,
        session::{Position, Session},
    },
    ui::panes::{gff::feature_row_count, local_feature_track::local_feature_row_count},
};

/// fixed height (rows) for the bottom consensus pane.
/// the remaining vertical space is used for the alignment pane.
const CONSENSUS_PANE_HEIGHT_ROWS: u16 = 5;
/// fixed height (rows) for the alignment ruler above sequence rows.
pub(crate) const RULER_HEIGHT_ROWS: u16 = 2;
/// width percentage for the left sequence ID pane (used in alignment and consensus panes).
/// the remaining horizontal space is used for sequence content.
const SEQUENCE_ID_PANE_WIDTH_PERCENT: u16 = 20;

#[derive(Debug)]
pub struct ScreenLayout {
    pub area: Rect,
    pub frame: FrameLayout,
    pub app: AppLayout,
    pub window: Window,
}

impl ScreenLayout {
    pub fn new(
        area: Rect,
        session: Option<&Session>,
        gff: Option<&Gff>,
        position: &mut Position,
    ) -> Self {
        let frame = FrameLayout::new(area);
        let [_, alignment_pane] = split_id_pane(frame.content_area);
        // also the GFF pane's inner width
        let visible_width = usize::from(Block::bordered().inner(alignment_pane).width);
        let total_columns = session.map_or(0, |session| session.layout().columns().len());
        let columns = fit(&mut position.column, visible_width, total_columns);

        let (gff_height, local_feature_rows) = match (gff, session) {
            (Some(gff), Some(session)) => {
                let gff_rows = feature_row_count(gff, session, visible_width).max(1);
                let local_rows = local_feature_row_count(gff, session, &columns);
                (
                    gff_pane_height(gff_rows),
                    u16::try_from(local_rows).unwrap_or(u16::MAX),
                )
            }
            _ => (0, 0),
        };
        let app = AppLayout::new(
            frame.content_area,
            gff_height,
            AlignmentHeaderLayout::new(local_feature_rows),
        );

        let available_sequence_rows = usize::from(app.alignment_pane_sequence_rows.height);
        let pinned = session.map_or(0, |session| session.layout().pinned());
        let shown_pinned = pinned.min(available_sequence_rows.saturating_sub(1));
        let divider = usize::from(shown_pinned > 0);
        let scrollable_height = available_sequence_rows - shown_pinned - divider;
        let main_rows = session.map_or(0, |session| session.layout().main().len());
        let rows = fit(&mut position.row, scrollable_height, main_rows);

        let row_count = session.map_or(0, |session| session.base_alignment().row_count());
        let number_prefix_width = row_count.max(1).ilog10() as usize + 2;
        let name_visible_width = usize::from(Block::bordered().inner(app.sequence_id_pane).width)
            .saturating_sub(number_prefix_width);
        let max_id_len = session.map_or(0, |session| session.base_alignment().max_id_len());
        let names = fit(&mut position.name, name_visible_width, max_id_len);

        debug!(
            terminal_width = area.width,
            terminal_height = area.height,
            visible_width,
            available_sequence_rows,
            scrollable_height,
            name_visible_width,
            "Screen layout rebuilt"
        );

        Self {
            area,
            frame,
            app,
            window: Window {
                pinned: 0..shown_pinned,
                rows,
                columns,
                names,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AlignmentHeaderLayout {
    pub local_feature_rows: u16,
}

impl AlignmentHeaderLayout {
    pub(crate) fn new(local_feature_rows: u16) -> Self {
        Self { local_feature_rows }
    }

    pub(crate) fn height(self) -> u16 {
        self.local_feature_rows.saturating_add(RULER_HEIGHT_ROWS)
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

#[derive(Debug, Clone, Copy)]
#[allow(clippy::struct_field_names)]
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

        let [sequence_id_pane_area, alignment_pane_area] = split_id_pane(alignment_area);
        let [
            consensus_sequence_id_pane_area,
            consensus_alignment_pane_area,
        ] = split_id_pane(consensus_area);
        let inner_alignment_pane = Block::bordered().inner(alignment_pane_area);
        let [_, _, sequence_rows_area] = inner_alignment_pane.layout(&vertical![
                ==alignment_header.local_feature_rows,
                ==RULER_HEIGHT_ROWS,
                *=1
        ]);

        let [gff_info_pane_area, gff_pane_area] = split_id_pane(gff_area);
        let gff_pane_rows = Block::bordered().inner(gff_pane_area);

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

fn split_id_pane(area: Rect) -> [Rect; 2] {
    area.layout(&horizontal![==SEQUENCE_ID_PANE_WIDTH_PERCENT%, *=1].spacing(Spacing::Overlap(1)))
}

fn fit(offset: &mut usize, visible: usize, total: usize) -> Range<usize> {
    *offset = (*offset).min(total.saturating_sub(visible));
    *offset..(*offset + visible).min(total)
}

fn gff_pane_height(feature_row_count: usize) -> u16 {
    if feature_row_count == 0 {
        return 0;
    }
    let inner = u16::try_from(feature_row_count).unwrap_or(u16::MAX.saturating_sub(3));
    inner.saturating_add(3)
}

#[cfg(test)]
#[allow(clippy::single_range_in_vec_init)]
mod tests {
    use rstest::rstest;

    use super::*;
    use crate::test_utils::{genes, pinned_session, session};

    const AREA: Rect = Rect::new(0, 0, 80, 24);

    #[test]
    fn new_clamps_offsets_past_end() {
        let session = session(&vec![[b'A'; 200].as_slice(); 50]);
        let mut position = Position {
            row: 500,
            column: 500,
            name: 500,
        };

        let window = ScreenLayout::new(AREA, Some(&session), None, &mut position).window;

        assert_eq!(window.rows.end, 50);
        assert_eq!(window.columns.end, 200);
        assert_eq!(window.names, 0..3);
        assert_eq!(
            (position.row, position.column, position.name),
            (window.rows.start, window.columns.start, window.names.start)
        );
    }

    #[rstest]
    #[case::no_features(&[], 4)]
    #[case::one_feature(&[0..10], 4)]
    #[case::alternating(&[0..10, 20..30], 5)]
    #[case::same_span_only_at_pane_width(&[0..10, 40..43, 40..42], 6)]
    fn new_sizes_gff_pane_to_feature_rows(#[case] ranges: &[Range<usize>], #[case] expected: u16) {
        let session = session(&[&[b'A'; 200]]);

        let screen = ScreenLayout::new(
            AREA,
            Some(&session),
            Some(&genes(ranges)),
            &mut Position::default(),
        );

        assert_eq!(screen.app.gff_pane.height, expected);
    }

    #[rstest]
    #[case::stacked_in_window(0, 2)]
    #[case::scrolled_past_stack(140, 1)]
    fn new_sizes_local_features_to_window(#[case] column: usize, #[case] expected: u16) {
        let session = session(&[&[b'A'; 200]]);
        let gff = genes(&[0..10, 2..8, 150..160]);
        let mut position = Position {
            column,
            ..Position::default()
        };

        let screen = ScreenLayout::new(AREA, Some(&session), Some(&gff), &mut position);

        assert_eq!(screen.app.alignment_header.local_feature_rows, expected);
    }

    #[rstest]
    #[case::fewer_than_space(2, 0..2)]
    #[case::capped_below_height(20, 0..12)]
    fn new_caps_pinned_rows(#[case] pinned: usize, #[case] expected: Range<usize>) {
        let pinned: Vec<usize> = (0..pinned).collect();
        let session = pinned_session(30, &pinned);

        let screen = ScreenLayout::new(AREA, Some(&session), None, &mut Position::default());

        assert_eq!(screen.window.pinned, expected);
    }

    #[test]
    fn new_without_session_is_empty() {
        let mut position = Position {
            row: 5,
            column: 5,
            name: 5,
        };

        let window = ScreenLayout::new(AREA, None, None, &mut position).window;

        assert!(window.pinned.is_empty());
        assert!(window.rows.is_empty());
        assert!(window.columns.is_empty());
        assert!(window.names.is_empty());
        assert_eq!((position.row, position.column, position.name), (0, 0, 0));
    }

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
