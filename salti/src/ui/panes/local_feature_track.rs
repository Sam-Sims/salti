use std::ops::Range;

use ratatui::{buffer::Buffer, layout::Rect, widgets::Widget};

use crate::{
    core::{gff::Gff, session::Session},
    ui::{
        features::{DisplayFeature, PlacedFeature, display_features, render_features},
        layout::Window,
        ui_state::ThemeState,
    },
};

const MAX_LOCAL_FEATURE_ROWS: usize = 5;

pub(crate) struct LocalFeatureTrack<'a> {
    pub(crate) gff: &'a Gff,
    pub(crate) session: &'a Session,
    pub(crate) window: &'a Window,
    pub(crate) theme: &'a ThemeState,
}

impl Widget for LocalFeatureTrack<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() {
            return;
        }

        let display_features = display_features(self.gff, self.session);
        let placed_features = place_visible_features(&display_features, &self.window.columns);
        render_features(&placed_features, area, self.theme, buf);
    }
}

pub(crate) fn local_feature_row_count(
    gff: &Gff,
    session: &Session,
    columns: &Range<usize>,
) -> usize {
    place_visible_features(&display_features(gff, session), columns)
        .iter()
        .map(|placed| placed.row + 1)
        .max()
        .unwrap_or(1)
}

fn place_visible_features<'a>(
    display_features: &[DisplayFeature<'a>],
    col_range: &Range<usize>,
) -> Vec<PlacedFeature<'a>> {
    let mut res: Vec<PlacedFeature<'a>> = Vec::new();
    let mut row_ends = [0; MAX_LOCAL_FEATURE_ROWS];

    for display_feature in display_features {
        let Some(visible_range) = intersect(&display_feature.columns, col_range) else {
            continue;
        };
        let span = visible_range.start - col_range.start..visible_range.end - col_range.start;
        let Some(row) = row_ends.iter().position(|&row_end| row_end <= span.start) else {
            continue;
        };
        row_ends[row] = span.end;
        res.push(PlacedFeature {
            feature: display_feature.feature,
            span,
            row,
            colour_idx: display_feature.colour_idx,
        });
    }

    res
}

fn intersect(left: &Range<usize>, right: &Range<usize>) -> Option<Range<usize>> {
    let start = left.start.max(right.start);
    let end = left.end.min(right.end);
    (start < end).then_some(start..end)
}

#[cfg(test)]
#[allow(clippy::single_range_in_vec_init)]
mod tests {
    use rstest::rstest;

    use super::*;
    use crate::test_utils::{genes, session};

    fn display(gff: &Gff) -> Vec<DisplayFeature<'_>> {
        gff.features
            .iter()
            .enumerate()
            .map(|(colour_idx, feature)| DisplayFeature {
                feature,
                columns: feature.range.clone(),
                colour_idx,
            })
            .collect()
    }

    #[rstest]
    #[case::shifted_to_window(&[5..10], 3..13, &[(2..7, 0)])]
    #[case::clipped_to_window(&[0..10], 5..8, &[(0..3, 0)])]
    #[case::outside_window_dropped(&[0..5, 6..8], 5..10, &[(1..3, 0)])]
    #[case::overlaps_stack(&[0..10, 2..8, 10..12], 0..12, &[(0..10, 0), (2..8, 1), (10..12, 0)])]
    #[case::capped_at_five_rows(
        &[0..4, 0..4, 0..4, 0..4, 0..4, 0..4, 4..6],
        0..10,
        &[(0..4, 0), (0..4, 1), (0..4, 2), (0..4, 3), (0..4, 4), (4..6, 0)],
    )]
    fn place_visible_features_works(
        #[case] ranges: &[Range<usize>],
        #[case] window: Range<usize>,
        #[case] expected: &[(Range<usize>, usize)],
    ) {
        let gff = genes(ranges);

        let placed: Vec<(Range<usize>, usize)> = place_visible_features(&display(&gff), &window)
            .into_iter()
            .map(|placed| (placed.span, placed.row))
            .collect();

        assert_eq!(placed, expected);
    }

    #[rstest]
    #[case::none_visible_keeps_one_row(&[0..5], 10..15, 1)]
    #[case::stacked(&[0..10, 2..8], 0..12, 2)]
    fn local_feature_row_count_works(
        #[case] ranges: &[Range<usize>],
        #[case] columns: Range<usize>,
        #[case] expected: usize,
    ) {
        let session = session(&[&[b'A'; 20]]);

        assert_eq!(
            local_feature_row_count(&genes(ranges), &session, &columns),
            expected
        );
    }
}
