use crate::core::viewport::ViewportWindow;
use crate::core::{CoreState, LoadingState};
use crate::ui::UiState;
use crate::ui::selection::selection_row_bounds;
use crate::ui::utils::truncate_label;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Styled;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

/// maximum displayed character count for a selected sequence name in the status bar before truncation
const STATUS_BAR_SELECTED_NAME_MAX_CHARS: usize = 25;

fn build_bottom_status_bar(
    core: &CoreState,
    ui: &UiState,
    theme: &crate::config::theme::ThemeStyles,
) -> Vec<Span<'static>> {
    let mut parts = Vec::new();

    // optional filter text building
    if !core.filter.text.is_empty() {
        let visible_count = core.row_visibility.visible_count();
        let filter_text = &core.filter.text;
        parts.push(
            format!("Filtered to {visible_count} alignments with filter: {filter_text}")
                .set_style(theme.warning),
        );
    }

    // optional selection info building
    if let Some(selection) = ui.mouse.selection {
        let (row_min, row_max) = selection_row_bounds(selection, &ui.display_index);
        let selected_sequence_count = row_max - row_min + 1;
        let col_start = selection.column.min(selection.end_column) + 1;
        let col_end = selection.column.max(selection.end_column) + 1;

        if !parts.is_empty() {
            parts.push(Span::raw(" | "));
        }

        if selected_sequence_count == 1 && col_start == col_end {
            let sequence = &core.data.sequences[selection.sequence_id];
            let sequence_name = truncate_label(
                sequence.alignment.id.as_ref(),
                STATUS_BAR_SELECTED_NAME_MAX_CHARS,
            );
            parts.push(format!("Selected: {sequence_name} @ {col_start}").set_style(theme.text));
        } else {
            parts.push(
                format!("{selected_sequence_count} sequence(s) selected @ {col_start}-{col_end}")
                    .set_style(theme.text),
            );
        }
    }

    parts
}

fn build_top_status_bar(
    core: &CoreState,
    window: &ViewportWindow,
    theme: &crate::config::theme::ThemeStyles,
) -> Vec<Span<'static>> {
    let file_name = core
        .input_path
        .as_deref()
        .map(|input| {
            // for local paths, show just the file name for URLs makes more sense to show the full input.
            std::path::Path::new(input)
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or(input)
        })
        .unwrap_or("Unknown");

    let loading_text = core.loading_state.to_string();
    let loading_style = match &core.loading_state {
        LoadingState::Idle | LoadingState::Loading => theme.text_dim,
        LoadingState::Loaded => theme.success,
        LoadingState::Failed(_) => theme.error,
    };
    let loading_status = loading_text.set_style(loading_style);

    let alignment_count = core.data.sequences.len();
    let position_range = if core.data.sequence_length > 0 {
        let range = &window.col_range;
        let start = range.start + 1;
        let end = range.end;
        format!("Positions: {start}-{end}")
    } else {
        "Positions: 0-0".to_string()
    };

    vec![
        format!("File: {file_name}").set_style(theme.text_dim),
        Span::raw(" | "),
        loading_status,
        Span::raw(" | "),
        format!("{alignment_count} alignments").set_style(theme.text),
        Span::raw(" | "),
        position_range.set_style(theme.text),
    ]
}

pub fn render_frame(
    f: &mut Frame,
    top_status_area: Rect,
    bottom_status_area: Rect,
    core: &CoreState,
    window: &ViewportWindow,
    ui: &UiState,
) {
    let theme = &ui.theme_styles;
    let top_status_bar = build_top_status_bar(core, window, theme);
    let bottom_status_bar = build_bottom_status_bar(core, ui, theme);

    if top_status_area.height > 0 {
        let top_line = Line::from(top_status_bar).right_aligned();
        f.render_widget(
            Paragraph::new(top_line).style(theme.panel_block),
            top_status_area,
        );
    }
    if bottom_status_area.height > 0 {
        let contextual_line = Line::from(bottom_status_bar).right_aligned();
        f.render_widget(
            Paragraph::new(contextual_line).style(theme.panel_block),
            bottom_status_area,
        );
    }
}
