use crate::config::schemes::ColorSchemeFormatter;
use crate::state::State;
use crate::viewport::Viewport;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

#[derive(Debug, Default)]
pub struct AlignmentComponent;

impl AlignmentComponent {
    fn add_number_to_ruler(number_line: &mut [Span], center_pos: usize, number: usize) {
        let num_str = number.to_string();
        let num_len = num_str.len();
        let ruler_width = number_line.len();

        let start_idx = center_pos
            .saturating_sub(num_len / 2)
            .min(ruler_width.saturating_sub(num_len));

        for (j, c) in num_str.chars().enumerate() {
            if let Some(cell) = number_line.get_mut(start_idx + j) {
                *cell = Span::styled(c.to_string(), Style::default().fg(Color::Cyan));
            }
        }
    }

    fn build_ruler(viewport: &Viewport, _sequence_length: usize) -> (Line<'_>, Line<'_>) {
        let horizontal_range = viewport.horizontal_range();
        let start_pos = horizontal_range.start;
        let width = viewport.visible_width;

        let mut number_line = vec![Span::raw(" "); width];
        let mut marker_line = vec![Span::raw(" "); width];

        for (i, marker_span) in marker_line.iter_mut().enumerate().take(width) {
            let display_pos = start_pos + i + 1;

            if display_pos == 1 || display_pos % 5 == 0 {
                let is_major_tick = display_pos % 10 == 0;

                *marker_span = Span::styled(
                    if is_major_tick { "|" } else { "." },
                    Style::default().fg(if is_major_tick {
                        Color::Cyan
                    } else {
                        Color::DarkGray
                    }),
                );

                if is_major_tick || display_pos == 1 {
                    Self::add_number_to_ruler(&mut number_line, i, display_pos);
                }
            }
        }

        (Line::from(number_line), Line::from(marker_line))
    }

    fn render_ruler(viewport: &Viewport, area: Rect, f: &mut Frame, app_state: &State) {
        let sequence_length = app_state.sequence_length;
        let (number_line, marker_line) = Self::build_ruler(viewport, sequence_length);
        let ruler_paragraph = Paragraph::new(vec![number_line, marker_line]);
        f.render_widget(ruler_paragraph, area);
    }

    fn render_sequence_column(
        alignments: &[crate::parser::Alignment],
        viewport: &Viewport,
        color_scheme_manager: &ColorSchemeFormatter,
        area: Rect,
        f: &mut Frame,
    ) {
        let _sequence_count = alignments.len();
        let _sequence_length = alignments.first().map_or(0, |a| a.sequence.len());
        let vertical_range = viewport.vertical_range();
        let horizontal_range = viewport.horizontal_range();

        let visible_alignments =
            &alignments[vertical_range.start..vertical_range.end.min(alignments.len())];

        let alignment_lines: Vec<Line> = visible_alignments
            .iter()
            .map(|alignment| {
                let end = horizontal_range.end.min(alignment.sequence.len());
                let seq_slice = &alignment.sequence[horizontal_range.start..end];
                let spans = color_scheme_manager.format_sequence_bytes(seq_slice);
                Line::from(spans)
            })
            .collect();

        let paragraph = Paragraph::new(alignment_lines);
        f.render_widget(paragraph, area);
    }

    fn render_sequence_border(area: Rect, f: &mut Frame) -> Rect {
        let seq_block = Block::bordered().title("Alignment");
        let inner_area = seq_block.inner(area);
        f.render_widget(seq_block, area);
        inner_area
    }

    pub fn render(
        &mut self,
        f: &mut Frame,
        layout: &crate::layout::AppLayout,
        app_state: &State,
        viewport: &Viewport,
    ) {
        let alignments = &app_state.alignments;
        let color_scheme_manager = &app_state.color_scheme_manager;

        let inner_area = Self::render_sequence_border(layout.sequence_area, f);

        let areas =
            Layout::vertical([Constraint::Length(2), Constraint::Fill(1)]).split(inner_area);
        let ruler_area = areas[0];
        let sequence_content_area = areas[1];

        Self::render_ruler(viewport, ruler_area, f, app_state);
        Self::render_sequence_column(
            alignments,
            viewport,
            color_scheme_manager,
            sequence_content_area,
            f,
        );
    }
}
