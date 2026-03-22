use crate::{
    core::model::AlignmentModel,
    ui::{
        selection::selection_row_bounds,
        ui_state::{LoadingState, UiState},
        utils::truncate_label,
    },
};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Styled;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

/// maximum displayed character count for a selected sequence name in the status bar before truncation
const STATUS_BAR_SELECTED_NAME_MAX_CHARS: usize = 25;

fn format_gap_percent(max_gap_fraction: f32) -> String {
    let mut text = format!("{:.2}", max_gap_fraction * 100.0);
    while text.ends_with('0') {
        text.pop();
    }
    if text.ends_with('.') {
        text.pop();
    }
    text
}

fn build_bottom_status_bar(alignment: Option<&AlignmentModel>, ui: &UiState) -> Vec<Span<'static>> {
    let theme = &ui.theme.styles;
    let mut parts = Vec::new();

    if let Some(alignment) = alignment.filter(|alignment| alignment.filter().is_active()) {
        let visible_rows = alignment.view().row_count();
        let mut filter_text = String::from("Filters:");
        let mut counts = format!(" ({visible_rows} rows)");
        if let Some(pattern) = alignment.filter().pattern() {
            filter_text.push_str(&format!(" [rows: {pattern}]"));
        }
        if let Some(max_gap_fraction) = alignment.filter().max_gap_fraction() {
            filter_text.push_str(&format!(
                " [gaps: <= {}%]",
                format_gap_percent(max_gap_fraction)
            ));
            let visible_cols = alignment.view().column_count();
            counts.push_str(&format!(" ({visible_cols} cols)"));
        }
        parts.push(format!("{filter_text}{counts}").set_style(theme.warning));
    }

    // optional selection info building
    if let Some(selection) = ui.selection {
        let (row_min, row_max) = selection_row_bounds(selection);
        let selected_sequence_count = row_max - row_min + 1;
        let col_start = selection.column.min(selection.end_column) + 1;
        let col_end = selection.column.max(selection.end_column) + 1;

        if !parts.is_empty() {
            parts.push(Span::raw(" | "));
        }

        if selected_sequence_count == 1 && col_start == col_end {
            let sequence_name = if let Some(alignment) = alignment {
                if let Some(sequence) = alignment.base().project_absolute_row(selection.sequence_id)
                {
                    truncate_label(sequence.id(), STATUS_BAR_SELECTED_NAME_MAX_CHARS)
                } else {
                    "Unknown".to_string()
                }
            } else {
                "Unknown".to_string()
            };
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

fn build_top_status_bar(alignment: Option<&AlignmentModel>, ui: &UiState) -> Vec<Span<'static>> {
    let theme = &ui.theme.styles;
    let file_name = ui
        .meta
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

    let loading_text = ui.meta.loading_state.to_string();
    let loading_style = match &ui.meta.loading_state {
        LoadingState::Idle | LoadingState::Loading => theme.text_dim,
        LoadingState::Loaded => theme.success,
        LoadingState::Failed(_) => theme.error,
    };
    let loading_status = loading_text.set_style(loading_style);

    let alignment_count = alignment
        .map(|alignment| alignment.view().row_count())
        .unwrap_or(0);
    let alignment_length = alignment
        .map(|alignment| alignment.base().column_count())
        .unwrap_or(0);
    let position_range = alignment.map_or_else(
        || "Positions: 0-0".to_string(),
        |alignment| {
            let window = ui.viewport.window();
            match (
                alignment.view().absolute_column_id(window.col_range.start),
                window
                    .col_range
                    .end
                    .checked_sub(1)
                    .and_then(|end| alignment.view().absolute_column_id(end)),
            ) {
                (Some(start), Some(end)) => format!("Positions: {}-{}", start + 1, end + 1),
                _ => "Positions: 0-0".to_string(),
            }
        },
    );

    vec![
        format!("File: {file_name}").set_style(theme.text_dim),
        Span::raw(" | "),
        loading_status,
        Span::raw(" | "),
        format!("{alignment_count} alignments").set_style(theme.text),
        Span::raw(" | "),
        format!("Length: {alignment_length}").set_style(theme.text),
        Span::raw(" | "),
        position_range.set_style(theme.text),
    ]
}

pub fn render_frame(
    f: &mut Frame,
    top_status_area: Rect,
    bottom_status_area: Rect,
    alignment: Option<&AlignmentModel>,
    ui: &UiState,
) {
    let theme = &ui.theme.styles;
    let top_status_bar = build_top_status_bar(alignment, ui);
    let bottom_status_bar = build_bottom_status_bar(alignment, ui);

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::StartupState;
    use crate::core::model::AlignmentModel;

    fn raw(id: &str, sequence: &[u8]) -> libmsa::RawSequence {
        libmsa::RawSequence {
            id: id.to_string(),
            sequence: sequence.to_vec(),
        }
    }

    fn status_text(spans: &[Span<'_>]) -> String {
        spans.iter().map(|span| span.content.as_ref()).collect()
    }

    fn top_status_text(alignment: Option<&AlignmentModel>, ui: &UiState) -> String {
        status_text(&build_top_status_bar(alignment, ui))
    }

    fn ui_state() -> UiState {
        let mut ui = UiState::new(StartupState::default());
        ui.meta.loading_state = LoadingState::Loaded;
        ui
    }

    #[test]
    fn bottom_status_bar_formats_row_filter_summary() {
        let alignment = libmsa::Alignment::new(vec![
            raw("alpha", b"ACGT"),
            raw("beta", b"ACGT"),
            raw("gamma", b"ACGT"),
        ])
        .expect("alignment should be valid");
        let mut alignment = AlignmentModel::new(alignment).expect("alignment model should build");
        alignment
            .set_filter("alpha|beta".to_string())
            .expect("row filter should apply");
        let ui = ui_state();

        assert_eq!(
            status_text(&build_bottom_status_bar(Some(&alignment), &ui)),
            "Filters: [rows: alpha|beta] (2 rows)"
        );
    }

    #[test]
    fn bottom_status_bar_formats_row_and_gap_filter_summary() {
        let alignment = libmsa::Alignment::new(vec![
            raw("alpha", b"A--T"),
            raw("beta", b"A--T"),
            raw("gamma", b"ACGT"),
        ])
        .expect("alignment should be valid");
        let mut alignment = AlignmentModel::new(alignment).expect("alignment model should build");
        alignment
            .set_filter("alpha|beta".to_string())
            .expect("row filter should apply");
        alignment
            .set_gap_filter(Some(0.0))
            .expect("gap filter should apply");
        let ui = ui_state();

        assert_eq!(
            status_text(&build_bottom_status_bar(Some(&alignment), &ui)),
            "Filters: [rows: alpha|beta] [gaps: <= 0%] (2 rows) (2 cols)"
        );
    }

    #[test]
    fn top_status_bar_shows_alignment_length() {
        let alignment = libmsa::Alignment::new(vec![
            raw("alpha", b"ACGT"),
            raw("beta", b"ACGT"),
            raw("gamma", b"ACGT"),
        ])
        .expect("alignment should be valid");
        let alignment = AlignmentModel::new(alignment).expect("alignment model should build");
        let mut ui = ui_state();
        ui.viewport.update_dimensions(4, 3, 0);
        ui.viewport.set_bounds(
            alignment.view().row_count(),
            alignment.view().column_count(),
            alignment.base().max_id_len(),
        );

        assert_eq!(
            top_status_text(Some(&alignment), &ui),
            "File: Unknown | Status: Loaded | 3 alignments | Length: 4 | Positions: 1-4"
        );
    }
}
