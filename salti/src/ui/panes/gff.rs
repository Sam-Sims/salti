use std::ops::Range;

use crossterm::event::MouseEvent;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Styled,
    symbols::merge::MergeStrategy,
    text::{Line, Span},
    widgets::{Block, Paragraph, Widget},
};

use crate::{
    command::Command,
    core::{
        gff::{Feature, Gff, Strand},
        model::AlignmentModel,
    },
    input::movement::HorizontalDrag,
    ui::ui_state::ThemeState,
};

const MIN_LABEL_WIDTH: usize = 2;
const NUCLEOTIDE_POSITIONS_PER_COL: usize = 1;
const PROTEIN_POSITIONS_PER_COL: usize = 3;

pub(crate) struct GffPane<'a> {
    pub(crate) gff: &'a Gff,
    pub(crate) alignment: &'a AlignmentModel,
    pub(crate) viewport_col_range: &'a Range<usize>,
    pub(crate) theme: &'a ThemeState,
}

impl Widget for GffPane<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::bordered()
            .border_style(self.theme.styles.border)
            .style(self.theme.styles.base_block)
            .merge_borders(MergeStrategy::Exact);
        let inner_area = if area.width > 2 && area.height > 2 {
            block.inner(area)
        } else {
            Rect::default()
        };
        block.render(area, buf);

        let track =
            FeatureTrack::for_alignment(self.gff, self.alignment, usize::from(inner_area.width));
        let content_rows = Rect {
            width: u16::try_from(track.width()).unwrap_or(u16::MAX),
            ..inner_area
        };
        if content_rows.width == 0 || content_rows.height == 0 || track.total_columns() == 0 {
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

        render_features(&track, feature_rows, self.theme, buf);

        let line = generate_scroll_line(
            usize::from(navigation_row.width),
            self.viewport_col_range,
            track.total_columns(),
            self.theme,
        );
        Paragraph::new(line)
            .style(self.theme.styles.base_block)
            .render(navigation_row, buf);
    }
}

pub(crate) struct GffInfoPane<'a> {
    pub(crate) tooltip: Option<&'a str>,
    pub(crate) theme: &'a ThemeState,
}

impl Widget for GffInfoPane<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::bordered()
            .title(Line::from(
                "Feature Info".set_style(self.theme.styles.text_muted),
            ))
            .border_style(self.theme.styles.border)
            .style(self.theme.styles.base_block)
            .merge_borders(MergeStrategy::Exact);
        let inner_area = if area.width > 2 && area.height > 2 {
            block.inner(area)
        } else {
            Rect::default()
        };
        block.render(area, buf);
        if inner_area.width == 0 || inner_area.height == 0 {
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
        viewport_col_range: &Range<usize>,
        alignment: &AlignmentModel,
    ) -> Option<Command> {
        let total_columns = FeatureTrack::total_columns_for_alignment(alignment);
        self.pan_drag.handle_mouse(
            mouse,
            gff_content_area(gff_inner, total_columns),
            viewport_col_range,
            total_columns,
            position_from_mouse,
        )
    }

    pub(crate) fn is_dragging(&self) -> bool {
        self.pan_drag.is_dragging()
    }
}

#[derive(Debug, Clone)]
struct FeatureMap {
    total_columns: usize,
    absolute_total_columns: usize,
    offset: usize,
    // proteins use three nucleotide positions per displayed column; nucleotides use one.
    positions_to_col: usize,
}

impl FeatureMap {
    fn for_alignment(alignment: &AlignmentModel) -> Self {
        let visible_columns = alignment.view().column_count();
        let absolute_total_columns = alignment.base().column_count();
        let frame_offset = alignment.translation_frame().offset();

        if alignment.is_reloaded_as_protein() {
            Self::protein(visible_columns, absolute_total_columns, frame_offset)
        } else {
            Self::nucleotide(visible_columns, absolute_total_columns)
        }
    }

    fn nucleotide(total_columns: usize, absolute_total_columns: usize) -> Self {
        Self {
            total_columns,
            absolute_total_columns,
            offset: 0,
            positions_to_col: NUCLEOTIDE_POSITIONS_PER_COL,
        }
    }

    fn protein(total_columns: usize, absolute_total_columns: usize, frame_offset: usize) -> Self {
        Self {
            total_columns,
            absolute_total_columns,
            offset: frame_offset,
            positions_to_col: PROTEIN_POSITIONS_PER_COL,
        }
    }

    fn map_feature(&self, view: &libmsa::Alignment, feature: &Feature) -> Option<Range<usize>> {
        let start = if feature.range.start < self.offset {
            0
        } else {
            (feature.range.start - self.offset) / self.positions_to_col
        };
        let end = if feature.range.end <= self.offset {
            0
        } else {
            (feature.range.end - self.offset).div_ceil(self.positions_to_col)
        };
        let clipped_range =
            start.min(self.absolute_total_columns)..end.min(self.absolute_total_columns);
        (!clipped_range.is_empty()).then_some(())?;

        view.relative_column_range_intersecting(clipped_range)
    }
}

pub(crate) fn tooltip_at(
    gff: &Gff,
    alignment: &AlignmentModel,
    gff_inner: Rect,
    mouse_x: u16,
    mouse_y: u16,
) -> Option<String> {
    let track = FeatureTrack::for_alignment(gff, alignment, usize::from(gff_inner.width));
    if gff_inner.width == 0 || gff_inner.height == 0 || track.total_columns() == 0 {
        return None;
    }
    let content_rows = Rect {
        width: u16::try_from(track.width()).unwrap_or(u16::MAX),
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

pub(crate) fn feature_row_count(gff: &Gff, alignment: &AlignmentModel, width: usize) -> usize {
    FeatureTrack::for_alignment(gff, alignment, width).row_count()
}

fn gff_content_area(area: Rect, total_columns: usize) -> Rect {
    let width = area
        .width
        .min(u16::try_from(total_columns).unwrap_or(u16::MAX));
    Rect { width, ..area }
}

fn format_tooltip(feature: &Feature) -> String {
    let length = feature.range.end.saturating_sub(feature.range.start);
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

fn render_features(track: &FeatureTrack<'_>, inner: Rect, theme: &ThemeState, buf: &mut Buffer) {
    let width = usize::from(inner.width);
    let max_row = usize::from(inner.height);
    let foreground = theme.theme.sequence.foreground;
    let dna = theme.theme.sequence.dna;
    let palette = [dna.a, dna.t, dna.c, dna.g];
    let blank = (' ', theme.styles.base_block);
    let mut cells = vec![blank; width * max_row];
    let mut feature_idx = 0;

    for placed_feature in track.placed_features() {
        if placed_feature.row >= max_row {
            continue;
        }

        let feature = placed_feature.feature;
        let feature_span = placed_feature.span.clone();
        let colour = palette[feature_idx % palette.len()];
        let feature_style = theme.styles.base_block.bg(colour);
        let text_style = feature_style.fg(foreground);
        feature_idx += 1;

        for x in feature_span.clone() {
            let idx = placed_feature.row * width + x;
            cells[idx] = (' ', feature_style);
        }

        let available = feature_span.end - feature_span.start;
        let (label_x, label_width, strand_arrow) = match feature.strand {
            Strand::Forward if 0 < available => (
                feature_span.start,
                available.saturating_sub(1),
                Some((available - 1, '→')),
            ),
            Strand::Reverse if 0 < available => (
                feature_span.start + 1,
                available.saturating_sub(1),
                Some((0, '←')),
            ),
            Strand::Unknown | Strand::Forward | Strand::Reverse => {
                (feature_span.start, available, None)
            }
        };

        if let Some((arrow_offset, arrow)) = strand_arrow {
            let x = feature_span.start + arrow_offset;
            let idx = placed_feature.row * width + x;
            cells[idx] = (arrow, text_style);
        }

        if label_width >= MIN_LABEL_WIDTH {
            let truncated_len = feature.name.chars().take(label_width).count();
            let label_offset = (label_width - truncated_len) / 2;
            let label_start = label_x + label_offset;
            for (i, ch) in feature.name.chars().take(label_width).enumerate() {
                let x = label_start + i;
                let idx = placed_feature.row * width + x;
                cells[idx] = (ch, text_style);
            }
        }
    }

    let lines: Vec<Line<'static>> = cells
        .chunks(width)
        .map(|row| {
            Line::from(
                row.iter()
                    .map(|&(ch, style)| Span::styled(ch.to_string(), style))
                    .collect::<Vec<_>>(),
            )
        })
        .collect();
    Paragraph::new(lines)
        .style(theme.styles.base_block)
        .render(inner, buf);
}

#[derive(Debug, Clone)]
struct FeatureTrack<'a> {
    placed_features: Vec<PlacedFeature<'a>>,
    total_columns: usize,
    width: usize,
}

impl<'a> FeatureTrack<'a> {
    fn for_alignment(gff: &'a Gff, alignment: &AlignmentModel, available_width: usize) -> Self {
        let mapping = FeatureMap::for_alignment(alignment);
        let width = available_width.min(mapping.total_columns);
        let placed_features = placed_features(gff, alignment.view(), &mapping, width);
        Self {
            placed_features,
            total_columns: mapping.total_columns,
            width,
        }
    }

    fn total_columns_for_alignment(alignment: &AlignmentModel) -> usize {
        FeatureMap::for_alignment(alignment).total_columns
    }

    fn total_columns(&self) -> usize {
        self.total_columns
    }

    fn width(&self) -> usize {
        self.width
    }

    fn placed_features(&self) -> &[PlacedFeature<'a>] {
        &self.placed_features
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

#[derive(Debug, Clone)]
struct PlacedFeature<'a> {
    feature: &'a Feature,
    span: Range<usize>,
    row: usize,
}

fn placed_features<'a>(
    gff: &'a Gff,
    view: &libmsa::Alignment,
    mapping: &FeatureMap,
    width: usize,
) -> Vec<PlacedFeature<'a>> {
    let mut res = Vec::new();
    let mut previous_span = None;
    let mut alternating_row = 0;
    let mut stack_offset = 0;

    for feature in &gff.features {
        let Some(span) = feature_drawn_span(feature, view, mapping, width) else {
            continue;
        };

        let row = match previous_span {
            Some(previous_span) if previous_span == span => {
                stack_offset += 1;
                alternating_row + stack_offset
            }
            Some(_) | None => {
                alternating_row = res.len() % 2;
                stack_offset = 0;
                alternating_row
            }
        };
        previous_span = Some(span.clone());
        res.push(PlacedFeature { feature, span, row });
    }

    res
}

fn feature_drawn_span(
    feature: &Feature,
    view: &libmsa::Alignment,
    mapping: &FeatureMap,
    width: usize,
) -> Option<Range<usize>> {
    let screen_span = feature_screen_span(feature, view, mapping, width)?;
    let drawn_width = if 1 < screen_span.end - screen_span.start {
        screen_span.end - screen_span.start - 1
    } else {
        screen_span.end - screen_span.start
    };
    Some(screen_span.start..screen_span.start + drawn_width)
}

fn feature_screen_span(
    feature: &Feature,
    view: &libmsa::Alignment,
    mapping: &FeatureMap,
    width: usize,
) -> Option<Range<usize>> {
    let total_columns = mapping.total_columns;
    if width == 0 || total_columns == 0 {
        return None;
    }

    let mapped_span = mapping.map_feature(view, feature)?;
    let x_start = mapped_span.start.saturating_mul(width) / total_columns;
    let x_end = mapped_span
        .end
        .saturating_mul(width)
        .div_ceil(total_columns)
        .max(x_start + 1)
        .min(width);
    Some(x_start..x_end)
}

fn scroll_bar_span(
    width: usize,
    viewport_col_range: &Range<usize>,
    total_columns: usize,
) -> Option<Range<usize>> {
    if total_columns == 0 || width == 0 {
        return None;
    }

    let viewport_span = viewport_col_range
        .end
        .saturating_sub(viewport_col_range.start);
    let thumb_width = viewport_span
        .saturating_mul(width)
        .div_ceil(total_columns)
        .max(1)
        .min(width);
    let thumb_start =
        (viewport_col_range.start.saturating_mul(width) / total_columns).min(width - thumb_width);

    Some(thumb_start..thumb_start + thumb_width)
}

fn generate_scroll_line(
    width: usize,
    viewport_col_range: &Range<usize>,
    total_columns: usize,
    theme: &ThemeState,
) -> Line<'static> {
    let thumb_span = scroll_bar_span(width, viewport_col_range, total_columns);
    let spans: Vec<Span<'static>> = (0..width)
        .map(|x| {
            if thumb_span.as_ref().is_some_and(|span| span.contains(&x)) {
                Span::styled("▁", theme.styles.accent)
            } else {
                Span::styled(" ", theme.styles.base_block)
            }
        })
        .collect();
    Line::from(spans)
}

fn position_from_mouse(mouse_x: u16, area: Rect, total_columns: usize) -> usize {
    let offset = usize::from(mouse_x.saturating_sub(area.x));
    let width = usize::from(area.width);
    let column = offset.saturating_mul(total_columns) / width;
    column.min(total_columns.saturating_sub(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feature(start: usize, end: usize) -> Feature {
        feature_with_name("gene", start, end)
    }

    fn feature_with_name(name: &str, start: usize, end: usize) -> Feature {
        Feature {
            name: name.to_string(),
            kind: crate::core::gff::FeatureType::Gene,
            range: start..end + 1,
            strand: Strand::Forward,
        }
    }

    fn raw(sequence: &[u8]) -> libmsa::RawSequence {
        libmsa::RawSequence {
            id: "seq".to_string(),
            sequence: sequence.to_vec(),
        }
    }

    fn model_with_sequence(sequence: &[u8]) -> AlignmentModel {
        let alignment = libmsa::Alignment::new(vec![raw(sequence)]).unwrap();
        AlignmentModel::new(alignment).unwrap()
    }

    fn model_with_len(len: usize) -> AlignmentModel {
        model_with_sequence(&vec![b'A'; len])
    }

    #[test]
    fn filtered_nucleotide_mapping_collapses_hidden_columns() {
        let mut model = model_with_sequence(b"-A-CC--G");
        model.set_gap_filter(Some(0.0)).unwrap();
        let mapping = FeatureMap::for_alignment(&model);

        assert_eq!(
            model.view().absolute_column_ids().collect::<Vec<_>>(),
            vec![1, 3, 4, 7]
        );
        assert_eq!(
            mapping.map_feature(model.view(), &feature(2, 6)),
            Some(1..3)
        );
    }

    #[test]
    fn filtered_nucleotide_mapping_hides_fully_filtered_feature() {
        let mut model = model_with_sequence(b"-A-CC--G");
        model.set_gap_filter(Some(0.0)).unwrap();
        let mapping = FeatureMap::for_alignment(&model);

        assert_eq!(mapping.map_feature(model.view(), &feature(5, 6)), None);
    }

    #[test]
    fn filtered_protein_mapping_uses_projected_protein_columns() {
        let mut model = model_with_sequence(b"M-M-M");
        model.set_gap_filter(Some(0.0)).unwrap();
        let mapping = FeatureMap::protein(3, 5, 0);

        assert_eq!(
            model.view().absolute_column_ids().collect::<Vec<_>>(),
            vec![0, 2, 4]
        );
        assert_eq!(
            mapping.map_feature(model.view(), &feature(3, 8)),
            Some(1..2)
        );
    }

    #[test]
    fn mapping_clips_feature_that_extends_past_alignment_end() {
        let model = model_with_len(8);
        let mapping = FeatureMap::for_alignment(&model);

        assert_eq!(
            mapping.map_feature(model.view(), &feature(6, 10)),
            Some(6..8)
        );
    }

    #[test]
    fn mapping_hides_feature_after_alignment_end() {
        let model = model_with_len(8);
        let mapping = FeatureMap::for_alignment(&model);

        assert_eq!(mapping.map_feature(model.view(), &feature(10, 12)), None);
    }

    #[test]
    fn placed_features_alternate_rows() {
        let gff = Gff {
            features: vec![feature(0, 1), feature(2, 3), feature(4, 5)],
        };
        let model = model_with_len(6);
        let mapping = FeatureMap::for_alignment(&model);
        let rows: Vec<usize> = placed_features(&gff, model.view(), &mapping, 6)
            .into_iter()
            .map(|placed_feature| placed_feature.row)
            .collect();

        assert_eq!(rows, vec![0, 1, 0]);
        assert_eq!(feature_row_count(&gff, &model, 6), 2);
    }

    #[test]
    fn tooltip_uses_stacked_feature_rows() {
        let gff = Gff {
            features: vec![
                feature_with_name("gene1", 0, 0),
                feature_with_name("gene2", 1, 1),
            ],
        };
        let model = model_with_len(100);
        let rows = Rect::new(10, 5, 1, 3);

        let tooltip = tooltip_at(&gff, &model, rows, 10, 6).unwrap();

        assert!(tooltip.starts_with("gene2 "));
    }

    #[test]
    fn content_area_shrinks_to_visible_columns() {
        let area = Rect::new(10, 5, 100, 3);

        assert_eq!(gff_content_area(area, 12), Rect::new(10, 5, 12, 3));
        assert_eq!(gff_content_area(area, 120), area);
    }

    #[test]
    fn viewport_thumb_scales_with_visible_coordinate_span() {
        let nucleotide = scroll_bar_span(12, &(0..4), 12).unwrap();
        let protein = scroll_bar_span(12, &(0..4), 6).unwrap();

        assert!(nucleotide.len() < protein.len());
    }
}
