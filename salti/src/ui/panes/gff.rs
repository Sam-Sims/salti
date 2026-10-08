use std::ops::Range;

use crossterm::event::MouseEvent;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Styled,
    text::Line,
    widgets::{Paragraph, Widget},
};

use crate::{
    command::Command,
    core::{
        gff::{Feature, Gff},
        session::Session,
    },
    input::movement::HorizontalDrag,
    ui::{
        features::{DisplayFeature, PlacedFeature, display_features, render_features},
        layout::Window,
        rows::render_column_scrollbar,
        ui_state::ThemeState,
        utils::render_pane,
    },
};

pub(crate) struct GffPane<'a> {
    pub(crate) gff: &'a Gff,
    pub(crate) session: &'a Session,
    pub(crate) window: &'a Window,
    pub(crate) theme: &'a ThemeState,
}

impl Widget for GffPane<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let inner_area = render_pane(&self.theme.styles, None, area, buf);

        let track =
            FeatureTrack::for_alignment(self.gff, self.session, usize::from(inner_area.width));
        let content_rows = Rect {
            width: u16::try_from(track.width).unwrap_or(u16::MAX),
            ..inner_area
        };
        if content_rows.is_empty() {
            return;
        }

        let feature_rows = Rect {
            height: content_rows.height.saturating_sub(1),
            ..content_rows
        };
        let navigation_row = Rect::new(
            content_rows.x,
            content_rows.y + feature_rows.height,
            content_rows.width,
            1,
        );

        render_features(&track.placed_features, feature_rows, self.theme, buf);

        render_column_scrollbar(
            "▁",
            self.theme.styles.accent,
            track.total_columns,
            &self.window.columns,
            navigation_row,
            buf,
        );
    }
}

pub(crate) struct GffInfoPane<'a> {
    pub(crate) tooltip: Option<&'a str>,
    pub(crate) theme: &'a ThemeState,
}

impl Widget for GffInfoPane<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let inner_area = render_pane(
            &self.theme.styles,
            Some(Line::from(
                "Feature Info".set_style(self.theme.styles.text_muted),
            )),
            area,
            buf,
        );
        if inner_area.is_empty() {
            return;
        }

        let lines: Vec<Line<'_>> = match self.tooltip {
            Some(tooltip) => tooltip
                .lines()
                .map(|line| Line::from(line.set_style(self.theme.styles.text)))
                .collect(),
            None => vec![Line::from(
                "Hover over a feature".set_style(self.theme.styles.text_muted),
            )],
        };

        Paragraph::new(lines)
            .style(self.theme.styles.base_block)
            .render(inner_area, buf);
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct GffPaneState {
    pan_drag: HorizontalDrag,
}

impl GffPaneState {
    pub(crate) fn handle_mouse(
        &mut self,
        mouse: MouseEvent,
        gff_inner: Rect,
        window_columns: &Range<usize>,
        total_columns: usize,
    ) -> Option<Command> {
        self.pan_drag.handle_mouse(
            mouse,
            gff_content_area(gff_inner, total_columns),
            window_columns,
            total_columns,
        )
    }

    pub(crate) fn is_dragging(&self) -> bool {
        self.pan_drag.is_dragging()
    }
}

pub(crate) fn tooltip_at(
    gff: &Gff,
    session: &Session,
    gff_inner: Rect,
    mouse_x: u16,
    mouse_y: u16,
) -> Option<String> {
    let track = FeatureTrack::for_alignment(gff, session, usize::from(gff_inner.width));
    if gff_inner.is_empty() {
        return None;
    }
    let content_rows = Rect {
        width: u16::try_from(track.width).unwrap_or(u16::MAX),
        ..gff_inner
    };
    if !content_rows.contains((mouse_x, mouse_y).into()) {
        return None;
    }

    let feature_rows = Rect {
        height: content_rows.height.saturating_sub(1),
        ..content_rows
    };
    if !feature_rows.contains((mouse_x, mouse_y).into()) {
        return None;
    }

    let row = usize::from(mouse_y - feature_rows.y);
    let x = usize::from(mouse_x - feature_rows.x);

    track.feature_at(x, row).map(format_tooltip)
}

pub(crate) fn feature_row_count(gff: &Gff, session: &Session, width: usize) -> usize {
    FeatureTrack::for_alignment(gff, session, width).row_count()
}

fn gff_content_area(area: Rect, total_columns: usize) -> Rect {
    let width = area
        .width
        .min(u16::try_from(total_columns).unwrap_or(u16::MAX));
    Rect { width, ..area }
}

fn format_tooltip(feature: &Feature) -> String {
    let length = feature.range.len();
    format!(
        "{} ({}) — {}\n{}-{} • {} nt",
        feature.name,
        feature.kind,
        feature.strand,
        feature.range.start + 1,
        feature.range.end,
        length,
    )
}

#[derive(Debug, Clone)]
struct FeatureTrack<'a> {
    placed_features: Vec<PlacedFeature<'a>>,
    total_columns: usize,
    width: usize,
}

impl<'a> FeatureTrack<'a> {
    fn for_alignment(gff: &'a Gff, session: &Session, available_width: usize) -> Self {
        let total_columns = session.layout().columns().len();
        let width = available_width.min(total_columns);
        let placed_features =
            placed_features(&display_features(gff, session), width, total_columns);
        Self {
            placed_features,
            total_columns,
            width,
        }
    }

    fn row_count(&self) -> usize {
        self.placed_features
            .iter()
            .map(|placed_feature| placed_feature.row + 1)
            .max()
            .unwrap_or(0)
    }

    fn feature_at(&self, x: usize, row: usize) -> Option<&'a Feature> {
        self.placed_features
            .iter()
            .find(|placed_feature| placed_feature.row == row && placed_feature.span.contains(&x))
            .map(|placed_feature| placed_feature.feature)
    }
}

fn placed_features<'a>(
    display_features: &[DisplayFeature<'a>],
    width: usize,
    total_columns: usize,
) -> Vec<PlacedFeature<'a>> {
    let mut res = Vec::new();
    let mut alternating_row = 0;
    let mut stack_offset = 0;

    for display_feature in display_features {
        let Some(span) = feature_span(display_feature, width, total_columns) else {
            continue;
        };

        let same_as_previous = res
            .last()
            .is_some_and(|placed_feature: &PlacedFeature<'_>| placed_feature.span == span);
        let row = if same_as_previous {
            stack_offset += 1;
            alternating_row + stack_offset
        } else {
            alternating_row = res.len() % 2;
            stack_offset = 0;
            alternating_row
        };
        res.push(PlacedFeature {
            feature: display_feature.feature,
            span,
            row,
            colour_idx: display_feature.colour_idx,
        });
    }

    res
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
        .max(start + 1)
        .min(width);
    Some(start..start + (end - start).saturating_sub(1).max(1))
}

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use crate::core::gff::Strand;

//     fn feature(start: usize, end: usize) -> Feature {
//         feature_with_name("gene", start, end)
//     }

//     fn feature_with_name(name: &str, start: usize, end: usize) -> Feature {
//         Feature {
//             name: name.to_string(),
//             kind: crate::core::gff::FeatureType::Gene,
//             range: start..end + 1,
//             strand: Strand::Forward,
//         }
//     }

//     fn raw(sequence: &[u8]) -> libmsa::Sequence {
//         libmsa::Sequence {
//             id: "seq".to_string(),
//             residues: sequence.to_vec(),
//         }
//     }

//     fn model_with_sequence(sequence: &[u8]) -> AlignmentModel {
//         let alignment = libmsa::Alignment::new(vec![raw(sequence)]).unwrap();
//         AlignmentModel::new(alignment).unwrap()
//     }

//     fn model_with_len(len: usize) -> AlignmentModel {
//         model_with_sequence(&vec![b'A'; len])
//     }

//     #[test]
//     fn placed_features_alternate_rows() {
//         let gff = Gff {
//             features: vec![feature(0, 1), feature(2, 3), feature(4, 5)],
//         };
//         let model = model_with_len(6);
//         let display_features = display_features(&gff, &model);
//         let rows: Vec<usize> = placed_features(&display_features, 6, model.view().column_count())
//             .into_iter()
//             .map(|placed_feature| placed_feature.row)
//             .collect();

//         assert_eq!(rows, vec![0, 1, 0]);
//         assert_eq!(feature_row_count(&gff, &model, 6), 2);
//     }

//     #[test]
//     fn tooltip_uses_stacked_feature_rows() {
//         let gff = Gff {
//             features: vec![
//                 feature_with_name("gene1", 0, 0),
//                 feature_with_name("gene2", 1, 1),
//             ],
//         };
//         let model = model_with_len(100);
//         let rows = Rect::new(10, 5, 1, 3);

//         let tooltip = tooltip_at(&gff, &model, rows, 10, 6).unwrap();

//         assert!(tooltip.starts_with("gene2 "));
//     }

//     #[test]
//     fn content_area_shrinks_to_visible_columns() {
//         let area = Rect::new(10, 5, 100, 3);

//         assert_eq!(gff_content_area(area, 12), Rect::new(10, 5, 12, 3));
//         assert_eq!(gff_content_area(area, 120), area);
//     }
// }
