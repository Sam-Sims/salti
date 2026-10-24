use std::ops::Range;

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    macros::vertical,
    text::Line,
    widgets::{Paragraph, Widget},
};

use crate::{
    config::theme::Theme,
    gff::{Feature, Gff},
    session::Session,
    ui::{
        layout::Window,
        panes::{
            self,
            features::{self, DisplayFeature, PlacedFeature},
        },
    },
};

pub struct GffPane<'frame> {
    pub gff: &'frame Gff,
    pub session: &'frame Session,
    pub window: &'frame Window,
    pub theme: &'frame Theme,
}

impl Widget for GffPane<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let inner_area = panes::render_pane(self.theme, None, area, buf);

        let track =
            FeatureTrack::for_alignment(self.gff, self.session, usize::from(inner_area.width));
        let content_rows = content_area(inner_area, track.total_columns);
        if content_rows.is_empty() {
            return;
        }
        let [feature_rows, navigation_row] = split_navigation_row(content_rows);

        features::render(&track.placed_features, feature_rows, self.theme, buf);

        panes::render_column_scrollbar(
            self.theme,
            track.total_columns,
            &self.window.columns,
            navigation_row,
            buf,
        );
    }
}

pub struct GffInfoPane<'frame> {
    pub tooltip: Option<&'frame str>,
    pub theme: &'frame Theme,
}

impl Widget for GffInfoPane<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        use ratatui::style::Styled;

        let inner_area = panes::render_pane(
            self.theme,
            Some(Line::from("Feature Info".set_style(self.theme.text_muted))),
            area,
            buf,
        );
        if inner_area.is_empty() {
            return;
        }

        let lines: Vec<Line<'_>> = match self.tooltip {
            Some(tooltip) => tooltip
                .lines()
                .map(|line| Line::from(line.set_style(self.theme.text)))
                .collect(),
            None => vec![Line::from(
                "Hover over a feature".set_style(self.theme.text_muted),
            )],
        };

        Paragraph::new(lines)
            .style(self.theme.base)
            .render(inner_area, buf);
    }
}

pub fn tooltip_at(
    gff: &Gff,
    session: &Session,
    inner_area: Rect,
    mouse_x: u16,
    mouse_y: u16,
) -> Option<String> {
    let track = FeatureTrack::for_alignment(gff, session, usize::from(inner_area.width));
    let [feature_rows, _] = split_navigation_row(inner_area);
    if !feature_rows.contains((mouse_x, mouse_y).into()) {
        return None;
    }

    let row = usize::from(mouse_y - feature_rows.y);
    let x = usize::from(mouse_x - feature_rows.x);

    track.feature_at(x, row).map(format_tooltip)
}

/// how many rows the features need when the whole alignment is squeezed into `width` cells
pub fn feature_row_count(gff: &Gff, session: &Session, width: usize) -> usize {
    FeatureTrack::for_alignment(gff, session, width).row_count()
}

/// the part of `area` the features are drawn in
///
/// it's narrower than `area` when the alignment has fewer columns than cells, so features are never
/// stretched
pub fn content_area(area: Rect, total_columns: usize) -> Rect {
    let width = area
        .width
        .min(u16::try_from(total_columns).unwrap_or(u16::MAX));
    Rect { width, ..area }
}

// the features squeezed into the GFF pane, which always shows the whole alignment
#[derive(Debug, Clone)]
struct FeatureTrack<'gff> {
    placed_features: Vec<PlacedFeature<'gff>>,
    total_columns: usize,
}

impl<'gff> FeatureTrack<'gff> {
    fn for_alignment(gff: &'gff Gff, session: &Session, available_width: usize) -> Self {
        let total_columns = session.layout().columns().len();
        let width = available_width.min(total_columns);
        let placed_features =
            placed_features(&features::display(gff, session), width, total_columns);
        Self {
            placed_features,
            total_columns,
        }
    }

    fn row_count(&self) -> usize {
        self.placed_features
            .iter()
            .map(|placed_feature| placed_feature.row + 1)
            .max()
            .unwrap_or(0)
    }

    fn feature_at(&self, x: usize, row: usize) -> Option<&'gff Feature> {
        self.placed_features
            .iter()
            .find(|placed_feature| placed_feature.row == row && placed_feature.span.contains(&x))
            .map(|placed_feature| placed_feature.feature)
    }
}

fn format_tooltip(feature: &Feature) -> String {
    format!(
        "{} ({}) — {}\n{}-{} • {} nt",
        feature.name,
        feature.kind,
        feature.strand,
        feature.range.start + 1,
        feature.range.end,
        feature.range.len(),
    )
}

// the features sit above the navigation row along the bottom
fn split_navigation_row(area: Rect) -> [Rect; 2] {
    area.layout(&vertical![*=1, ==1])
}

fn placed_features<'gff>(
    display_features: &[DisplayFeature<'gff>],
    width: usize,
    total_columns: usize,
) -> Vec<PlacedFeature<'gff>> {
    let mut placed: Vec<PlacedFeature<'gff>> = Vec::new();
    let mut alternating_row = 0;
    let mut stack_offset = 0;

    for display_feature in display_features {
        let Some(span) = feature_span(display_feature, width, total_columns) else {
            continue;
        };

        let same_as_previous = placed
            .last()
            .is_some_and(|placed_feature| placed_feature.span == span);
        let row = if same_as_previous {
            // features with the same span stack below each other
            stack_offset += 1;
            alternating_row + stack_offset
        } else {
            // the rest alternate rows so neighbours don't touch
            alternating_row = placed.len() % 2;
            stack_offset = 0;
            alternating_row
        };
        placed.push(PlacedFeature {
            feature: display_feature.feature,
            span,
            row,
            colour_idx: display_feature.colour_idx,
        });
    }

    placed
}

fn feature_span(
    display_feature: &DisplayFeature<'_>,
    width: usize,
    total_columns: usize,
) -> Option<Range<usize>> {
    if width == 0 {
        return None;
    }

    let start = display_feature.columns.start.saturating_mul(width) / total_columns;
    let end = display_feature
        .columns
        .end
        .saturating_mul(width)
        .div_ceil(total_columns)
        .min(width);
    // at least one cell, leaving a gap before the next feature
    Some(start..start + (end - start).saturating_sub(1).max(1))
}

#[cfg(test)]
#[allow(clippy::single_range_in_vec_init)]
mod tests {
    use rstest::rstest;

    use super::*;
    use crate::{
        gff::{FeatureType, Strand},
        test_utils::{buffer_text, genes, session},
    };

    fn feature(name: &str, range: Range<usize>, strand: Strand) -> Feature {
        Feature {
            name: name.to_string(),
            kind: FeatureType::Gene,
            range,
            strand,
        }
    }

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
    #[case::same_scale_leaves_gap(2..5, 10, 10, 2..4)]
    #[case::one_column(3..4, 10, 10, 3..4)]
    #[case::scaled_down(0..50, 10, 100, 0..4)]
    #[case::short_feature_keeps_one_cell(50..51, 10, 100, 5..6)]
    #[case::at_end(95..100, 10, 100, 9..10)]
    fn feature_span_works(
        #[case] columns: Range<usize>,
        #[case] width: usize,
        #[case] total_columns: usize,
        #[case] expected: Range<usize>,
    ) {
        let gff = genes(&[columns]);

        assert_eq!(
            feature_span(&display(&gff)[0], width, total_columns),
            Some(expected)
        );
    }

    #[test]
    fn feature_span_rejects_zero_width() {
        let gff = genes(&[0..5]);

        assert_eq!(feature_span(&display(&gff)[0], 0, 10), None);
    }

    #[rstest]
    #[case::alternating(&[0..3, 4..7, 8..11], &[0, 1, 0])]
    #[case::same_span_stacks(&[0..3, 0..3, 0..3, 5..8], &[0, 1, 2, 1])]
    #[case::stacks_below_second_row(&[0..3, 5..8, 5..8], &[0, 1, 2])]
    fn placed_features_works(#[case] ranges: &[Range<usize>], #[case] expected: &[usize]) {
        let gff = genes(ranges);

        let rows: Vec<usize> = placed_features(&display(&gff), 20, 20)
            .into_iter()
            .map(|placed| placed.row)
            .collect();

        assert_eq!(rows, expected);
    }

    #[rstest]
    #[case::no_features_shown(&[30..40], 0)]
    #[case::alternating(&[0..3, 4..7, 8..11], 2)]
    fn feature_row_count_works(#[case] ranges: &[Range<usize>], #[case] expected: usize) {
        let session = session(&[&[b'A'; 20]]);

        assert_eq!(feature_row_count(&genes(ranges), &session, 20), expected);
    }

    #[rstest]
    #[case::first_row(10, 5, Some(0))]
    #[case::stacked_row(10, 6, Some(1))]
    #[case::empty_cell(12, 5, None)]
    #[case::navigation_row(10, 7, None)]
    #[case::left_of_area(9, 5, None)]
    fn tooltip_at_works(#[case] x: u16, #[case] y: u16, #[case] expected: Option<usize>) {
        let session = session(&[&[b'A'; 100]]);
        let gff = genes(&[0..1, 1..2, 2..3]);

        assert_eq!(
            tooltip_at(&gff, &session, Rect::new(10, 5, 4, 3), x, y),
            expected.map(|i| format_tooltip(&gff.features[i]))
        );
    }

    #[test]
    fn pane() {
        let session = session(&[&[b'A'; 40]]);
        let gff = Gff {
            features: vec![
                feature("alpha", 0..12, Strand::Forward),
                feature("beta", 14..30, Strand::Reverse),
                feature("gamma", 32..40, Strand::Unknown),
                feature("delta", 32..40, Strand::Forward),
            ],
        };
        let window = Window {
            columns: 0..10,
            ..Window::default()
        };
        let theme = Theme::default();
        let area = Rect::new(0, 0, 42, 6);
        let mut buf = Buffer::empty(area);

        GffPane {
            gff: &gff,
            session: &session,
            window: &window,
            theme: &theme,
        }
        .render(area, &mut buf);

        insta::assert_snapshot!(buffer_text(&buf, area));
    }
}
