use crate::core::CoreState;
use crate::ui::UiState;
use crate::ui::utils::split_pinned_rows;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Style, Styled};
use ratatui::symbols::merge::MergeStrategy;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;

fn build_sequence_id_line(
    ui: &UiState,
    sequence_id: usize,
    alignment_id: &str,
    name_offset: usize,
    name_width: usize,
    is_pinned: bool,
) -> Line<'static> {
    let theme = &ui.theme_styles;
    let number_prefix = format!("{} ", sequence_id + 1).set_style(theme.success);
    // sequence IDs can be longer than the visible sequence ID pane width.
    let id_slice: String = alignment_id
        .chars()
        .skip(name_offset)
        .take(name_width)
        .collect();
    let id_style = if is_pinned { theme.warning } else { theme.text };
    let id_span = id_slice.set_style(id_style);
    Line::from(vec![number_prefix, id_span])
}

fn build_pinned_divider_line(width: usize, style: Style) -> Line<'static> {
    Line::from("─".repeat(width).set_style(style))
}

fn render_sequence_id_rows(core: &CoreState, ui: &UiState, area: Rect, f: &mut Frame) {
    let window = core.viewport.window();
    let ruler_height = 2;
    let row_capacity = area.height.saturating_sub(ruler_height) as usize;
    let pinned_visible: Vec<_> = core.visible_pinned_sequences().take(row_capacity).collect();
    let (pinned_rows, has_pins, unpinned_rows) =
        split_pinned_rows(row_capacity, pinned_visible.len());
    let unpinned_start = window.row_range.start;
    let mut id_lines = Vec::with_capacity(row_capacity + ruler_height as usize);

    // create blank lines where the alignment pane ruler sits, so sequence ID pane rows stay aligned.
    id_lines.push(Line::from(" "));
    if pinned_visible.is_empty() {
        id_lines.push(Line::from(" "));
    } else {
        id_lines.push(Line::from(
            "Pinned sequences:".set_style(ui.theme_styles.text_muted),
        ));
    }

    let name_width = window
        .name_range
        .end
        .saturating_sub(window.name_range.start);

    for sequence in pinned_visible.into_iter().take(pinned_rows) {
        id_lines.push(build_sequence_id_line(
            ui,
            sequence.sequence_id,
            sequence.alignment.id.as_ref(),
            window.name_range.start,
            name_width,
            true,
        ));
    }

    if has_pins {
        id_lines.push(build_pinned_divider_line(
            area.width as usize,
            ui.theme_styles.border,
        ));
    }

    for sequence in core
        .visible_unpinned_sequences()
        .skip(unpinned_start)
        .take(unpinned_rows)
    {
        id_lines.push(build_sequence_id_line(
            ui,
            sequence.sequence_id,
            sequence.alignment.id.as_ref(),
            window.name_range.start,
            name_width,
            false,
        ));
    }

    let id_paragraph = Paragraph::new(id_lines)
        .alignment(ratatui::layout::HorizontalAlignment::Left)
        .style(ui.theme_styles.base_block);
    f.render_widget(id_paragraph, area);
}

pub fn render_sequence_id_pane(
    f: &mut Frame,
    layout: &crate::ui::layout::AppLayout,
    core: &CoreState,
    ui: &UiState,
) {
    let theme = &ui.theme_styles;
    let block = ratatui::widgets::Block::bordered()
        .title(Line::from("Sequence Name".set_style(theme.accent)))
        .border_style(theme.border)
        .style(theme.base_block)
        .merge_borders(MergeStrategy::Exact);
    let inner_area = block.inner(layout.sequence_id_pane_area);
    f.render_widget(block, layout.sequence_id_pane_area);

    render_sequence_id_rows(core, ui, inner_area, f);
}
