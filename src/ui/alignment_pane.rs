use crate::core::viewport::ViewportWindow;
use crate::core::{CoreState, LoadingState};
use crate::ui::UiState;
use crate::ui::rows::{format_row_spans, select_row_render_mode};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::macros::vertical;
use ratatui::style::{Styled, Stylize};
use ratatui::symbols::merge::MergeStrategy;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

fn render_failed_alignment_message(ui: &UiState, area: Rect, error: &str, f: &mut Frame) {
    let theme = &ui.theme_styles;
    let [_, message_area, _] = area.layout(&vertical![*=1, ==1, *=1]);
    let message = Line::from(format!("Failed to load alignment: {error}").set_style(theme.error));
    let paragraph = Paragraph::new(message)
        .alignment(ratatui::layout::HorizontalAlignment::Center)
        .style(theme.base_block);
    f.render_widget(paragraph, message_area);
}

fn render_idle_alignment_message(ui: &UiState, area: Rect, f: &mut Frame) {
    let theme = &ui.theme_styles;
    let [_, message_area, _] = area.layout(&vertical![*=1, ==5, *=1]);
    let message = vec![
        Line::from(
            "salti: A modern MSA browser for the terminal."
                .fg(ui.theme.text)
                .bold(),
        ),
        Line::from("Use the command palette to open an alignment.".set_style(theme.text)),
        Line::from(""),
        Line::from(
            "Hint: press : and run the load-alignment <path> command".set_style(theme.text_dim),
        ),
    ];
    let paragraph = Paragraph::new(message)
        .alignment(ratatui::layout::HorizontalAlignment::Center)
        .style(theme.base_block);
    f.render_widget(paragraph, message_area);
}

fn render_sequence_rows(
    core: &CoreState,
    window: &ViewportWindow,
    ui: &UiState,
    area: Rect,
    theme: &crate::config::theme::ThemeStyles,
    f: &mut Frame,
    consensus: Option<&[u8]>,
    row_ids: &[Option<usize>],
) {
    let horizontal_range = window.col_range.clone();
    let mut alignment_lines = Vec::with_capacity(row_ids.len());
    let render_mode = select_row_render_mode(core, consensus);

    for row_id in row_ids.iter().copied() {
        let Some(sequence_id) = row_id else {
            alignment_lines.push(Line::from(
                "─".repeat(area.width as usize).set_style(theme.border),
            ));
            continue;
        };
        let sequence = &core.data.sequences[sequence_id];
        let spans = format_row_spans(
            sequence.alignment.sequence.as_ref(),
            &horizontal_range,
            &ui.theme.sequence,
            render_mode,
        );
        alignment_lines.push(Line::from(spans));
    }

    let paragraph = Paragraph::new(alignment_lines).style(theme.base_block);
    f.render_widget(paragraph, area);
}

fn render_scrollbar(
    core: &CoreState,
    window: &ViewportWindow,
    ui: &UiState,
    area: Rect,
    f: &mut Frame,
) {
    if area.width < 2 || area.height == 0 {
        return;
    }

    if core.viewport.max_size.cols <= core.viewport.dims.cols {
        return;
    }

    let width = area.width.saturating_sub(2) as usize;
    let max_index = core.viewport.max_size.cols.saturating_sub(1);
    let col_offset = window.col_range.start;
    let percent = if max_index == 0 {
        0
    } else {
        col_offset.saturating_mul(100) / max_index
    };
    let track_max = width.saturating_sub(1);
    let thumb_index = if track_max == 0 {
        0
    } else {
        (percent * track_max) / 100
    };
    let scrollbar_area = Rect {
        x: area.x + 1,
        y: area.y + area.height.saturating_sub(1),
        width: area.width.saturating_sub(2),
        height: 1,
    };
    let thumb_width = if width >= 3 { 3 } else { 1 };
    let thumb_start = thumb_index.saturating_sub(thumb_width / 2);
    let thumb_end = (thumb_start + thumb_width).min(width);
    let thumb_y = scrollbar_area.y;
    let thumb_colour = ui.theme.accent_alt;

    for offset in thumb_start..thumb_end {
        let thumb_x = scrollbar_area.x + offset as u16;
        if let Some(cell) = f.buffer_mut().cell_mut((thumb_x, thumb_y)) {
            let track_colour = cell.fg;
            cell.set_char('▬');
            cell.set_fg(thumb_colour);
            cell.set_bg(track_colour);
        }
    }
}

fn add_number_to_ruler(
    number_line: &mut [Span],
    centre_pos: usize,
    number: usize,
    theme: &crate::config::theme::ThemeStyles,
) {
    let number_string = number.to_string();
    let number_length = number_string.len();
    let ruler_width = number_line.len();

    let start_idx = centre_pos
        .saturating_sub(number_length / 2)
        .min(ruler_width.saturating_sub(number_length));

    for (i, digit) in number_string.chars().enumerate() {
        if let Some(cell) = number_line.get_mut(start_idx + i) {
            *cell = digit.to_string().set_style(theme.accent);
        }
    }
}
fn build_ruler(
    core: &CoreState,
    window: &ViewportWindow,
    theme: &crate::config::theme::ThemeStyles,
) -> (Line<'static>, Line<'static>) {
    let start_pos = window.col_range.start;
    let width = window.col_range.end.saturating_sub(window.col_range.start);
    let total_cols = core.viewport.max_size.cols;

    let mut number_line = vec![Span::raw(" "); width];
    let mut marker_line = vec![Span::raw(" "); width];

    for (i, marker_span) in marker_line.iter_mut().enumerate() {
        let display_pos = start_pos + i + 1;
        if display_pos > total_cols {
            break;
        }

        if display_pos == 1 || display_pos.is_multiple_of(5) {
            let is_major_tick = display_pos.is_multiple_of(10);

            *marker_span = if is_major_tick {
                "|".set_style(theme.accent)
            } else {
                ".".set_style(theme.text_dim)
            };

            if is_major_tick || display_pos == 1 {
                add_number_to_ruler(&mut number_line, i, display_pos, theme);
            }
        }
    }

    (Line::from(number_line), Line::from(marker_line))
}

fn render_ruler(
    core: &CoreState,
    window: &ViewportWindow,
    area: Rect,
    theme: &crate::config::theme::ThemeStyles,
    f: &mut Frame,
) {
    let (number_line, marker_line) = build_ruler(core, window, theme);
    let ruler_paragraph = Paragraph::new(vec![number_line, marker_line]).style(theme.base_block);
    f.render_widget(ruler_paragraph, area);
}

pub fn render_alignment_pane(
    f: &mut Frame,
    layout: &crate::ui::layout::AppLayout,
    core: &CoreState,
    window: &ViewportWindow,
    ui: &UiState,
    row_ids: &[Option<usize>],
) {
    let consensus = core.consensus.as_deref();
    let theme = &ui.theme_styles;

    let alignment_pane_block = Block::bordered()
        .title(Line::from("Alignment".set_style(theme.accent)))
        .border_style(theme.border)
        .style(theme.base_block)
        .merge_borders(MergeStrategy::Exact);

    let inner_area = alignment_pane_block.inner(layout.alignment_pane);
    f.render_widget(alignment_pane_block, layout.alignment_pane);

    if let LoadingState::Failed(error) = &core.loading_state {
        render_failed_alignment_message(ui, inner_area, error, f);
        return;
    }
    if matches!(&core.loading_state, LoadingState::Idle) {
        render_idle_alignment_message(ui, inner_area, f);
        return;
    }

    let [ruler_area, sequence_content_area] = inner_area.layout(&vertical![==2, *=1]);

    render_ruler(core, window, ruler_area, theme, f);
    render_sequence_rows(
        core,
        window,
        ui,
        sequence_content_area,
        theme,
        f,
        consensus,
        row_ids,
    );
    render_scrollbar(core, window, ui, layout.alignment_pane, f);
}
