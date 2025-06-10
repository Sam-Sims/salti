use crate::state::State;
use crate::viewport::Viewport;
use ratatui::Frame;
use ratatui::layout::Rect;

#[derive(Debug, Default)]
pub struct SequenceIdComponent;

impl SequenceIdComponent {
    fn render_sequence_ids(
        alignments: &[crate::parser::Alignment],
        viewport: &Viewport,
        area: Rect,
        f: &mut Frame,
    ) {
        use ratatui::text::Line;
        use ratatui::widgets::Paragraph;

        let _sequence_count = alignments.len();
        let vertical_range = viewport.vertical_range();
        let visible_alignments = if alignments.is_empty() {
            &[]
        } else {
            &alignments[vertical_range.start..vertical_range.end.min(alignments.len())]
        };

        let ruler_height = 2;
        let mut id_lines = Vec::with_capacity(visible_alignments.len() + ruler_height);

        for _ in 0..ruler_height {
            id_lines.push(Line::from(""));
        }

        for alignment in visible_alignments {
            id_lines.push(Line::from(alignment.id.as_ref()));
        }

        let id_paragraph = Paragraph::new(id_lines);
        f.render_widget(id_paragraph, area);
    }

    pub fn render(
        &mut self,
        f: &mut Frame,
        layout: &crate::layout::AppLayout,
        app_state: &State,
        viewport: &Viewport,
    ) {
        let alignments = &app_state.alignments;

        let block = ratatui::widgets::Block::bordered().title("Sequence Name");
        let inner_area = block.inner(layout.id_area);
        f.render_widget(block, layout.id_area);

        Self::render_sequence_ids(alignments, viewport, inner_area, f);
    }
}
