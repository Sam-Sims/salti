use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Styled,
    text::Line,
    widgets::{Paragraph, Widget},
};

use crate::{
    core::session::Session,
    ui::{
        layout::{AlignmentHeaderLayout, Window, screen_rows},
        ui_state::ThemeState,
        utils::pane_block,
    },
};

pub(crate) struct SequenceIdPane<'a> {
    pub(crate) session: &'a Session,
    pub(crate) window: &'a Window,
    pub(crate) header: AlignmentHeaderLayout,
    pub(crate) theme: &'a ThemeState,
}

impl Widget for SequenceIdPane<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = pane_block(&self.theme.styles);
        let inner_area = block.inner(area);
        block.render(area, buf);

        let (layout, theme) = (self.session.layout(), self.theme);
        let mut lines = vec![Line::from(" "); usize::from(self.header.height())];
        if self.header.ruler_rows > 1 && !self.window.pinned.is_empty() {
            lines[usize::from(self.header.local_feature_rows) + 1] =
                Line::from("Pinned sequences:".set_style(theme.styles.text_muted));
        }

        let names = &self.window.names;
        lines.extend(screen_rows(layout, self.window).map(|pos| {
            match pos {
                Some(pos) => {
                    let row = layout.rows()[pos];
                    let style = if pos < layout.pinned() {
                        theme.styles.accent
                    } else {
                        theme.styles.text
                    };

                    let id: String = self
                        .session
                        .base_alignment()
                        .id(row)
                        .chars()
                        .skip(names.start)
                        .take(names.len())
                        .collect();
                    Line::from(vec![
                        format!("{} ", row + 1).set_style(theme.styles.success),
                        id.set_style(style),
                    ])
                }
                None => Line::from(
                    "─"
                        .repeat(usize::from(inner_area.width))
                        .set_style(theme.styles.border),
                ),
            }
        }));

        Paragraph::new(lines)
            .style(theme.styles.base_block)
            .render(inner_area, buf);
    }
}

#[cfg(test)]
mod tests {
    use ratatui::{buffer::Buffer, layout::Rect};

    use super::*;
    use crate::ui::layout::AppLayout;

    fn raw(id: &str, sequence: &[u8]) -> libmsa::Sequence {
        libmsa::Sequence {
            id: id.to_string(),
            residues: sequence.to_vec(),
        }
    }

    fn alignment_model(sequences: Vec<libmsa::Sequence>) -> AlignmentModel {
        let alignment = libmsa::Alignment::new(sequences).unwrap();
        AlignmentModel::new(alignment).unwrap()
    }

    fn render_sequence_id_pane_text(alignment: &AlignmentModel, window: &ViewportWindow) -> String {
        render_sequence_id_pane_text_with_header(
            alignment,
            window,
            AlignmentHeaderLayout::without_features(),
        )
    }

    fn render_sequence_id_pane_text_with_header(
        alignment: &AlignmentModel,
        window: &ViewportWindow,
        header: AlignmentHeaderLayout,
    ) -> String {
        let area = Rect::new(0, 0, 150, 12);
        let mut buffer = Buffer::empty(area);
        let layout = AppLayout::new(area, 0, header);
        let theme = ThemeState::default();

        SequenceIdPane {
            alignment,
            window,
            header: layout.alignment_header,
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
    fn basic_sequence_ids_snapshot() {
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
    fn local_feature_rows_reserved_snapshot() {
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
            "sequence_id_pane_local_feature_rows",
            render_sequence_id_pane_text_with_header(
                &alignment,
                &window,
                AlignmentHeaderLayout::with_features(2),
            )
        );
    }

    #[test]
    fn pinned_sequence_ids_snapshot() {
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
    fn scrolled_sequence_names_snapshot() {
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
