use crate::{
    core::{model::AlignmentModel, viewport::ViewportWindow},
    ui::{
        layout::{AppLayout, RULER_HEIGHT_ROWS, pinned_section_layout},
        ui_state::ThemeState,
    },
};
use ratatui::Frame;
use ratatui::style::{Style, Styled};
use ratatui::symbols::merge::MergeStrategy;
use ratatui::text::Line;
use ratatui::widgets::{Block, Paragraph};

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
    f: &mut Frame,
    alignment: &AlignmentModel,
    window: &ViewportWindow,
    theme: &ThemeState,
    area: ratatui::layout::Rect,
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

    f.render_widget(
        Paragraph::new(lines)
            .alignment(ratatui::layout::HorizontalAlignment::Left)
            .style(theme.styles.base_block),
        area,
    );
}

pub fn render_sequence_id_pane(
    f: &mut Frame,
    layout: &AppLayout,
    alignment: &AlignmentModel,
    window: &ViewportWindow,
    theme: &ThemeState,
) {
    let block = Block::bordered()
        .title(Line::from("Sequence Name".set_style(theme.styles.accent)))
        .border_style(theme.styles.border)
        .style(theme.styles.base_block)
        .merge_borders(MergeStrategy::Exact);
    let inner_area = block.inner(layout.sequence_id_pane);
    f.render_widget(block, layout.sequence_id_pane);

    render_sequence_id_rows(f, alignment, window, theme, inner_area);
}
