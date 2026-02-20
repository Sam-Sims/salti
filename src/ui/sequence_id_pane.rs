use crate::core::CoreState;
use crate::core::viewport::ViewportWindow;
use crate::ui::UiState;
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

fn render_sequence_id_rows(
    core: &CoreState,
    window: &ViewportWindow,
    ui: &UiState,
    area: Rect,
    f: &mut Frame,
    visible_rows: &[Option<usize>],
) {
    let ruler_height: usize = 2;
    let mut id_lines = Vec::with_capacity(visible_rows.len() + ruler_height);
    let has_pins = visible_rows.first().is_some_and(|r| {
        r.is_some_and(|id| core.is_sequence_pinned(id))
    });

    // create blank lines where the alignment pane ruler sits, so sequence ID pane rows stay aligned.
    id_lines.push(Line::from(" "));
    if has_pins {
        id_lines.push(Line::from(
            "Pinned sequences:".set_style(ui.theme_styles.text_muted),
        ));
    } else {
        id_lines.push(Line::from(" "));
    }

    let name_width = window
        .name_range
        .end
        .saturating_sub(window.name_range.start);

    for row_id in visible_rows {
        match row_id {
            Some(sequence_id) => {
                let sequence = &core.data.sequences[*sequence_id];
                let is_pinned = core.is_sequence_pinned(*sequence_id);
                id_lines.push(build_sequence_id_line(
                    ui,
                    *sequence_id,
                    sequence.alignment.id.as_ref(),
                    window.name_range.start,
                    name_width,
                    is_pinned,
                ));
            }
            None => {
                id_lines.push(build_pinned_divider_line(
                    area.width as usize,
                    ui.theme_styles.border,
                ));
            }
        }
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
    window: &ViewportWindow,
    ui: &UiState,
    visible_rows: &[Option<usize>],
) {
    let theme = &ui.theme_styles;
    let block = ratatui::widgets::Block::bordered()
        .title(Line::from("Sequence Name".set_style(theme.accent)))
        .border_style(theme.border)
        .style(theme.base_block)
        .merge_borders(MergeStrategy::Exact);
    let inner_area = block.inner(layout.sequence_id_pane);
    f.render_widget(block, layout.sequence_id_pane);

    render_sequence_id_rows(core, window, ui, inner_area, f, visible_rows);
}
