use crate::{
    core::{model::AlignmentModel, viewport::ViewportWindow},
    ui::{
        layout::{RULER_HEIGHT_ROWS, pinned_section_layout},
        ui_state::ThemeState,
    },
};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Style, Styled};
use ratatui::symbols::merge::MergeStrategy;
use ratatui::text::Line;
use ratatui::widgets::{Block, Paragraph, Widget};

pub(crate) struct SequenceIdPane<'a> {
    pub(crate) alignment: &'a AlignmentModel,
    pub(crate) window: &'a ViewportWindow,
    pub(crate) theme: &'a ThemeState,
}

impl Widget for SequenceIdPane<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::bordered()
            .title("Sequence Name")
            .border_style(self.theme.styles.border)
            .style(self.theme.styles.base_block)
            .merge_borders(MergeStrategy::Exact);
        let inner_area = block.inner(area);
        block.render(area, buf);

        render_sequence_id_rows(self.alignment, self.window, self.theme, inner_area, buf);
    }
}

fn build_sequence_id_line(
    theme: &ThemeState,
    absolute_row: usize,
    alignment_id: &str,
    name_offset: usize,
    name_width: usize,
    id_style: Style,
) -> Line<'static> {
    let number_prefix = format!("{} ", absolute_row + 1).set_style(theme.styles.success);
    // sequence IDs can be longer than the visible sequence ID pane width.
    let id_slice: String = alignment_id
        .chars()
        .skip(name_offset)
        .take(name_width)
        .collect();

    Line::from(vec![number_prefix, id_slice.set_style(id_style)])
}

fn build_pinned_divider_line(width: usize, style: Style) -> Line<'static> {
    Line::from("─".repeat(width).set_style(style))
}

fn render_sequence_id_rows(
    alignment: &AlignmentModel,
    window: &ViewportWindow,
    theme: &ThemeState,
    area: Rect,
    buf: &mut Buffer,
) {
    let ruler_height = usize::from(RULER_HEIGHT_ROWS);
    let available_content_height = area.height.saturating_sub(RULER_HEIGHT_ROWS) as usize;
    let band_layout =
        pinned_section_layout(alignment.rows().pinned().len(), available_content_height);
    let mut lines = Vec::with_capacity(ruler_height + area.height as usize);

    let has_pins = !alignment.rows().pinned().is_empty();
    for ruler_row in 0..ruler_height {
        if ruler_row == 1 && has_pins {
            lines.push(Line::from(
                "Pinned sequences:".set_style(theme.styles.text_muted),
            ));
        } else {
            lines.push(Line::from(" "));
        }
    }

    let name_width = window
        .name_range
        .end
        .saturating_sub(window.name_range.start);

    for &absolute_row in alignment
        .rows()
        .pinned()
        .iter()
        .take(band_layout.pinned_rendered)
    {
        let Some(sequence) = alignment.base().project_absolute_row(absolute_row) else {
            continue;
        };
        lines.push(build_sequence_id_line(
            theme,
            absolute_row,
            sequence.id(),
            window.name_range.start,
            name_width,
            theme.styles.accent,
        ));
    }

    if band_layout.divider_height == 1 {
        lines.push(build_pinned_divider_line(
            area.width as usize,
            theme.styles.border,
        ));
    }

    for relative_row in window.row_range.clone() {
        let Some(sequence) = alignment.view().sequence(relative_row) else {
            continue;
        };
        lines.push(build_sequence_id_line(
            theme,
            sequence.absolute_row_id(),
            sequence.id(),
            window.name_range.start,
            name_width,
            theme.styles.text,
        ));
    }

    Paragraph::new(lines)
        .alignment(ratatui::layout::HorizontalAlignment::Left)
        .style(theme.styles.base_block)
        .render(area, buf);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::layout::AppLayout;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;

    fn raw(id: &str, sequence: &[u8]) -> libmsa::RawSequence {
        libmsa::RawSequence {
            id: id.to_string(),
            sequence: sequence.to_vec(),
        }
    }

    fn alignment_model(sequences: Vec<libmsa::RawSequence>) -> AlignmentModel {
        let alignment = libmsa::Alignment::new(sequences).unwrap();
        AlignmentModel::new(alignment).unwrap()
    }

    fn render_sequence_id_pane_text(alignment: &AlignmentModel, window: &ViewportWindow) -> String {
        let area = Rect::new(0, 0, 150, 12);
        let mut buffer = Buffer::empty(area);
        let layout = AppLayout::new(area, 0);
        let theme = ThemeState::default();

        SequenceIdPane {
            alignment,
            window,
            theme: &theme,
        }
        .render(layout.sequence_id_pane, &mut buffer);

        buffer_text(&buffer)
    }

    fn buffer_text(buffer: &Buffer) -> String {
        let area = buffer.area;
        let mut lines = Vec::new();

        for y in area.top()..area.bottom() {
            let mut line = String::new();
            for x in area.left()..area.right() {
                let symbol = buffer[(x, y)].symbol();
                if symbol.is_empty() {
                    line.push(' ');
                } else {
                    line.push_str(symbol);
                }
            }
            while line.ends_with(' ') {
                line.pop();
            }
            lines.push(line);
        }

        while matches!(lines.last(), Some(last) if last.is_empty()) {
            lines.pop();
        }

        lines.join("\n")
    }

    #[test]
    fn sequence_id_pane_basic_snapshot() {
        let alignment = alignment_model(vec![
            raw("seq1", b"CATCATCATCATCATCAT"),
            raw("seq2", b"CATCATCATCATCATCAT"),
            raw("seq3", b"CATCATCATCATCATCAT"),
        ]);
        let window = ViewportWindow {
            row_range: 0..alignment.view().row_count(),
            col_range: 0..alignment.view().column_count(),
            name_range: 0..18,
        };

        insta::assert_snapshot!(
            "sequence_id_pane_basic",
            render_sequence_id_pane_text(&alignment, &window)
        );
    }

    #[test]
    fn sequence_id_pane_pinned_snapshot() {
        let mut alignment = alignment_model(vec![
            raw("seq1", b"CATCATCATCATCATCAT"),
            raw("seq2", b"CATCATCATCATCATCAT"),
            raw("seq3", b"CATCATCATCATCATCAT"),
            raw("seq4", b"CATCATCATCATCATCAT"),
        ]);
        alignment.pin(1).unwrap();
        alignment.pin(3).unwrap();

        let window = ViewportWindow {
            row_range: 0..alignment.view().row_count(),
            col_range: 0..alignment.view().column_count(),
            name_range: 0..18,
        };

        insta::assert_snapshot!(
            "sequence_id_pane_pinned",
            render_sequence_id_pane_text(&alignment, &window)
        );
    }

    #[test]
    fn sequence_id_pane_name_scroll_snapshot() {
        let alignment = alignment_model(vec![
            raw("seq1-loooooooooooong-name", b"CATCATCATCATCATCAT"),
            raw("seq2-loooooooooooong-name", b"CATCATCATCATCATCAT"),
            raw("seq3-loooooooooooong-name", b"CATCATCATCATCATCAT"),
        ]);
        let window = ViewportWindow {
            row_range: 0..alignment.view().row_count(),
            col_range: 0..alignment.view().column_count(),
            name_range: 5..23,
        };

        insta::assert_snapshot!(
            "sequence_id_pane_name_scroll",
            render_sequence_id_pane_text(&alignment, &window)
        );
    }
}
