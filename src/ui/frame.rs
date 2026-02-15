use crate::core::{CoreState, LoadingState};
use crate::ui::selection::{display_index_by_sequence_id, selection_row_bounds};
use crate::ui::utils::truncate_label;
use crate::ui::UiState;
use ratatui::layout::Rect;
use ratatui::style::Styled;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

/// maximum displayed character count for a selected sequence name in the status bar before truncation
const STATUS_BAR_SELECTED_NAME_MAX_CHARS: usize = 25;

fn build_bottom_status_bar(
    core: &CoreState,
    ui: &UiState,
    theme: &crate::config::theme::ThemeStyles,
) -> Vec<Span<'static>> {
    let sequence_length = core.data.sequence_length;

    let position_range = if sequence_length > 0 {
        let range = core.viewport.window().col_range;
        let start = range.start + 1;
        let end = range.end;
        format!("Positions: {start}-{end}")
    } else {
        "Positions: 0-0".to_string()
    };

    let alignment_count = core.data.sequences.len();
    let mut parts = vec![format!("{alignment_count} alignments").set_style(theme.text_dim)];

    // optional filter text building
    if !core.filter_text.is_empty() {
        let visible_count = core.visible_sequences().count();
        let filter_text = &core.filter_text;
        parts.push(Span::raw(" | "));
        parts.push(
            format!("Filtered to {visible_count} alignments with filter: {filter_text}")
                .set_style(theme.warning),
        );
    }

    // optional selection info building
    let display_indices = display_index_by_sequence_id(core);
    if let Some(selection) = ui.mouse_selection {
        let (row_min, row_max) = selection_row_bounds(selection, &display_indices);
        let selected_sequence_count = row_max - row_min + 1;
        let col_start = selection.column.min(selection.end_column) + 1;
        let col_end = selection.column.max(selection.end_column) + 1;

        parts.push(Span::raw(" | "));
        if selected_sequence_count == 1 && col_start == col_end {
            let sequence = &core.data.sequences[selection.sequence_id];
            let sequence_name = truncate_label(
                sequence.alignment.id.as_ref(),
                STATUS_BAR_SELECTED_NAME_MAX_CHARS,
            );
            parts.push(format!("Selected: {sequence_name} @ {col_start}").set_style(theme.accent));
        } else {
            parts.push(
                format!("{selected_sequence_count} sequence(s) selected @ {col_start}-{col_end}")
                    .set_style(theme.accent),
            );
        }
    }

    parts.push(Span::raw(" | "));
    parts.push(position_range.set_style(theme.text_dim));
    parts
}

fn build_top_status_bar(
    core: &CoreState,
    _ui: &UiState,
    theme: &crate::config::theme::ThemeStyles,
) -> Vec<Span<'static>> {
    let file_path = &core.data.file_path;

    let file_name = file_path
        .as_ref()
        .and_then(|path| path.file_name())
        .and_then(|name| name.to_str())
        .unwrap_or("Unknown");

    let loading_text = core.loading_state.to_string();
    let loading_style = match &core.loading_state {
        LoadingState::Idle => theme.text_dim,
        LoadingState::Loaded => theme.success,
        LoadingState::Failed(_) => theme.error,
    };
    let loading_status = loading_text.set_style(loading_style);

    vec![
        format!("File: {file_name}").set_style(theme.text_dim),
        Span::raw(" | "),
        loading_status,
    ]
}

pub fn render_frame(
    f: &mut Frame,
    top_status_area: Rect,
    bottom_status_area: Rect,
    core: &CoreState,
    ui: &UiState,
) {
    let theme = &ui.theme_styles;
    let top_status_bar = build_top_status_bar(core, ui, theme);
    let bottom_status_bar = build_bottom_status_bar(core, ui, theme);

    if top_status_area.height > 0 {
        let top_line = Line::from(top_status_bar).right_aligned();
        f.render_widget(Paragraph::new(top_line), top_status_area);
    }
    if bottom_status_area.height > 0 {
        let left_line = Line::from(bottom_status_bar).left_aligned();
        let right_line = Line::from("Press 'q' to quit".set_style(theme.text_dim)).right_aligned();
        f.render_widget(
            Paragraph::new(left_line).style(theme.panel_block),
            bottom_status_area,
        );
        f.render_widget(
            Paragraph::new(right_line).style(theme.panel_block),
            bottom_status_area,
        );
    }
}
