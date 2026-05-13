use std::ops::Range;

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    text::{Line, Span},
    widgets::{Paragraph, Widget},
};

use crate::{
    core::{
        gff::{Feature, Gff, Strand},
        model::AlignmentModel,
        viewport::ViewportWindow,
    },
    ui::{
        features::{DisplayFeature, FeatureMap, display_features, feature_style},
        ui_state::ThemeState,
    },
};

const MAX_LOCAL_FEATURE_ROWS: usize = 5;
const MIN_LABEL_WIDTH: usize = 1;

pub(crate) struct LocalFeatureTrack<'a> {
    pub(crate) gff: &'a Gff,
    pub(crate) alignment: &'a AlignmentModel,
    pub(crate) window: &'a ViewportWindow,
    pub(crate) theme: &'a ThemeState,
}

impl Widget for LocalFeatureTrack<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let display_features = display_features(self.gff, self.alignment);
        let placed_features = place_visible_features(&display_features, &self.window.col_range);
        render_features(&placed_features, area, self.theme, buf);
    }
}

pub(crate) fn local_feature_row_count(
    gff: &Gff,
    alignment: &AlignmentModel,
    col_range: &Range<usize>,
) -> usize {
    let mapping = FeatureMap::for_alignment(alignment);
    let mut row_ends = [0; MAX_LOCAL_FEATURE_ROWS];
    let mut row_count = 0;

    for feature in &gff.features {
        let Some(relative_col_range) = mapping.map_feature(alignment.view(), feature) else {
            continue;
        };
        let Some(visible_range) = intersect(&relative_col_range, col_range) else {
            continue;
        };
        let span = visible_range.start - col_range.start..visible_range.end - col_range.start;
        let Some(row) = row_ends.iter().position(|&row_end| row_end <= span.start) else {
            continue;
        };
        row_ends[row] = span.end;
        row_count = row_count.max(row + 1);
    }

    row_count.max(1)
}

#[derive(Debug, Clone)]
struct LocalPlacedFeature<'a> {
    feature: &'a Feature,
    span: Range<usize>,
    row: usize,
    colour_idx: usize,
}

fn place_visible_features<'a>(
    display_features: &[DisplayFeature<'a>],
    col_range: &Range<usize>,
) -> Vec<LocalPlacedFeature<'a>> {
    let mut res: Vec<LocalPlacedFeature<'a>> = Vec::new();
    let mut row_ends = [0; MAX_LOCAL_FEATURE_ROWS];

    for display_feature in display_features {
        let Some(visible_range) = intersect(&display_feature.relative_col_range, col_range) else {
            continue;
        };
        let span = visible_range.start - col_range.start..visible_range.end - col_range.start;
        let Some(row) = row_ends.iter().position(|&row_end| row_end <= span.start) else {
            continue;
        };
        row_ends[row] = span.end;
        res.push(LocalPlacedFeature {
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

fn render_features(
    placed_features: &[LocalPlacedFeature<'_>],
    area: Rect,
    theme: &ThemeState,
    buf: &mut Buffer,
) {
    let width = usize::from(area.width);
    let height = usize::from(area.height);
    let blank = (' ', theme.styles.base_block);
    let mut cells = vec![blank; width * height];

    for placed_feature in placed_features {
        if height <= placed_feature.row {
            continue;
        }
        let styles = feature_style(theme, placed_feature.colour_idx);
        let span = placed_feature.span.start.min(width)..placed_feature.span.end.min(width);
        if span.is_empty() {
            continue;
        }

        for x in span.clone() {
            cells[placed_feature.row * width + x] = (' ', styles.background);
        }

        draw_label(
            &mut cells,
            width,
            placed_feature.row,
            span,
            placed_feature.feature,
            styles.text,
        );
    }

    let lines: Vec<Line<'static>> = cells
        .chunks(width)
        .map(|row| {
            let mut spans = Vec::new();
            let mut text = String::new();
            let mut current_style = row[0].1;

            for &(ch, style) in row {
                if style != current_style && !text.is_empty() {
                    spans.push(Span::styled(std::mem::take(&mut text), current_style));
                    current_style = style;
                }
                text.push(ch);
            }

            if !text.is_empty() {
                spans.push(Span::styled(text, current_style));
            }

            Line::from(spans)
        })
        .collect();
    Paragraph::new(lines)
        .style(theme.styles.base_block)
        .render(area, buf);
}

fn draw_label(
    cells: &mut [(char, ratatui::style::Style)],
    width: usize,
    row: usize,
    span: Range<usize>,
    feature: &Feature,
    text_style: ratatui::style::Style,
) {
    let available = span.len();
    if available == 0 {
        return;
    }

    let (label_start, label_width, arrow) = match feature.strand {
        Strand::Forward => (
            span.start,
            available.saturating_sub(1),
            Some((span.end - 1, '→')),
        ),
        Strand::Reverse => (
            span.start + 1,
            available.saturating_sub(1),
            Some((span.start, '←')),
        ),
        Strand::Unknown => (span.start, available, None),
    };

    if let Some((arrow_x, arrow)) = arrow {
        cells[row * width + arrow_x] = (arrow, text_style);
    }

    if label_width < MIN_LABEL_WIDTH {
        return;
    }

    let truncated: String = feature.name.chars().take(label_width).collect();
    let truncated_len = truncated.chars().count();
    let label_offset = (label_width - truncated_len) / 2;
    let x_start = label_start + label_offset;
    for (offset, ch) in truncated.chars().enumerate() {
        cells[row * width + x_start + offset] = (ch, text_style);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::gff::FeatureType;

    fn feature_with_name(name: &str, start: usize, end: usize) -> Feature {
        Feature {
            name: name.to_string(),
            kind: FeatureType::Gene,
            range: start..end,
            strand: Strand::Forward,
        }
    }

    fn raw(sequence: &[u8]) -> libmsa::RawSequence {
        libmsa::RawSequence {
            id: "seq".to_string(),
            sequence: sequence.to_vec(),
        }
    }

    fn model_with_len(len: usize) -> AlignmentModel {
        let alignment = libmsa::Alignment::new(vec![raw(&vec![b'A'; len])]).unwrap();
        AlignmentModel::new(alignment).unwrap()
    }

    #[test]
    fn local_feature_row_count_is_one_when_no_feature_is_visible() {
        let gff = Gff {
            features: vec![feature_with_name("left", 0, 5)],
        };
        let model = model_with_len(20);

        assert_eq!(local_feature_row_count(&gff, &model, &(10..15)), 1);
    }

    #[test]
    fn local_features_stack_overlapping_visible_spans() {
        let gff = Gff {
            features: vec![
                feature_with_name("a", 0, 10),
                feature_with_name("b", 2, 8),
                feature_with_name("c", 10, 12),
            ],
        };
        let model = model_with_len(20);

        assert_eq!(local_feature_row_count(&gff, &model, &(0..12)), 2);
    }

    #[test]
    fn local_feature_rows_are_capped_at_five() {
        let gff = Gff {
            features: (0..8)
                .map(|idx| feature_with_name(&format!("f{idx}"), 0, 10))
                .collect(),
        };
        let model = model_with_len(20);

        assert_eq!(local_feature_row_count(&gff, &model, &(0..12)), 5);
    }
}
