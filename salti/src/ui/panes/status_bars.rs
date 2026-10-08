use std::fmt::Write as _;

use ratatui::{
    Frame,
    layout::Rect,
    style::Styled,
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::{
    core::session::{Session, ViewMode},
    ui::{
        ui_state::{LoadingState, UiState},
        utils::truncate_label,
    },
};

/// maximum displayed character count for a selected sequence name in the status bar before truncation
const STATUS_BAR_SELECTED_NAME_MAX_CHARS: usize = 25;

fn format_percent(fraction: f64) -> String {
    ((fraction * 10_000.0).round() / 100.0).to_string()
}

fn range_label(start: usize, end: usize) -> String {
    if start == end {
        start.to_string()
    } else {
        format!("{start}-{end}")
    }
}

fn build_bottom_status_bar(session: Option<&Session>, ui: &UiState) -> Vec<Span<'static>> {
    let Some(session) = session else {
        return Vec::new();
    };
    let theme = &ui.theme.styles;
    let (state, layout) = (session.state(), session.layout());
    let mut parts = Vec::new();

    if state.row_regex_filter.is_some() || state.filter.is_active() {
        let mut text = String::from("Filters:");
        if let Some(regex) = &state.row_regex_filter {
            let _ = write!(text, " [rows: {regex}]");
        }
        if let Some(fraction) = state.filter.max_gap_fraction {
            let _ = write!(text, " [gaps: <= {}%]", format_percent(fraction));
        }
        if let Some(fraction) = state.filter.min_const_fraction {
            let _ = write!(text, " [constant: >= {}%]", format_percent(fraction));
        }
        let _ = write!(text, " ({} rows)", layout.rows().len());
        if state.filter.is_active() {
            let _ = write!(text, " ({} cols)", layout.columns().len());
        }
        parts.push(text.set_style(theme.warning));
    }

    if state.mode != ViewMode::Default {
        parts.push(format!("Translation frame: {}", state.frame).set_style(theme.text));
    }

    if let Some(selection) = ui.selection {
        let count = selection.rows.last - selection.rows.start + 1;
        let lo = layout.columns()[selection.columns.start];
        let hi = layout.columns()[selection.columns.last];
        let nucleotides = range_label(lo + 1, hi + 1);

        let text = if count == 1 {
            let protein = session.protein_columns(lo..hi + 1);
            let position = if state.mode == ViewMode::QuickTranslate && !protein.is_empty() {
                range_label(protein.start + 1, protein.end)
            } else {
                nucleotides
            };
            let name = session
                .base_alignment()
                .id(layout.rows()[selection.rows.start]);
            format!(
                "Selected: {} @ {position}",
                truncate_label(name, STATUS_BAR_SELECTED_NAME_MAX_CHARS)
            )
        } else {
            format!("{count} sequence(s) selected @ {nucleotides}")
        };
        parts.push(text.set_style(theme.text));
    }

    parts
        .into_iter()
        .flat_map(|part| [Span::raw(" | "), part])
        .skip(1)
        .collect()
}

fn build_top_status_bar(session: Option<&Session>, ui: &UiState) -> Vec<Span<'static>> {
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

    let loading_status = match &ui.meta.loading_state {
        LoadingState::Idle => Span::styled("Status: Idle", theme.text_dim),
        LoadingState::Loading => Span::styled("Status: Loading", theme.text_dim),
        LoadingState::Loaded => Span::styled("Status: Loaded", theme.success),
        LoadingState::Failed(_) => Span::styled("Status: Failed", theme.error),
    };

    let layout = session.map(Session::layout);
    let alignment_count = layout.map_or(0, |layout| layout.rows().len());
    let alignment_length = session.map_or(0, |session| session.grid().width());
    let shown = layout
        .map(|layout| &layout.columns()[ui.window.columns.clone()])
        .unwrap_or_default();
    let position_range = match (shown.first(), shown.last()) {
        (Some(start), Some(end)) => format!("Positions: {}-{}", start + 1, end + 1),
        _ => "Positions: 0-0".to_string(),
    };

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
    session: Option<&Session>,
    ui: &UiState,
) {
    let theme = &ui.theme.styles;
    let top_status_bar = build_top_status_bar(session, ui);
    let bottom_status_bar = build_bottom_status_bar(session, ui);

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
    use crate::{cli::StartupState, ui::ui_state::MouseSelection};

    fn raw(id: &str, sequence: &[u8]) -> libmsa::Sequence {
        libmsa::Sequence {
            id: id.to_string(),
            residues: sequence.to_vec(),
        }
    }

    fn status_text(spans: &[Span<'_>]) -> String {
        spans.iter().map(|span| span.content.as_ref()).collect()
    }

    fn alignment_model(sequences: Vec<libmsa::Sequence>) -> AlignmentModel {
        let alignment = libmsa::Alignment::new(sequences).unwrap();
        AlignmentModel::new(alignment).unwrap()
    }

    fn ui_state() -> UiState {
        let mut ui = UiState::new(StartupState::default());
        ui.meta.loading_state = LoadingState::Loaded;
        ui
    }

    #[test]
    fn top_status_loaded_alignment() {
        let alignment = alignment_model(vec![
            raw("seq1", b"ACGTACGT"),
            raw("seq2", b"ACGTAC-T"),
            raw("seq3", b"ACGTACGA"),
        ]);
        let mut ui = ui_state();
        ui.meta.input_path = Some("/iamnotreal.fasta".to_string());
        ui.viewport.update_dimensions(5, 3, 0);
        ui.viewport.set_bounds(
            alignment.view().row_count(),
            alignment.view().column_count(),
            alignment.base().max_id_len(),
        );

        assert_eq!(
            status_text(&build_top_status_bar(Some(&alignment), &ui)),
            "File: iamnotreal.fasta | Status: Loaded | 3 alignments | Length: 8 | Positions: 1-5"
        );
    }

    #[test]
    fn top_status_failed_load() {
        let mut ui = UiState::new(StartupState::default());
        ui.meta.loading_state = LoadingState::Failed("boom".to_string());

        assert_eq!(
            status_text(&build_top_status_bar(None, &ui)),
            "File: Unknown | Status: Failed | 0 alignments | Length: 0 | Positions: 0-0"
        );
    }

    #[test]
    fn bottom_status_filters_translation() {
        let mut alignment = alignment_model(vec![
            raw("seq1", b"ATGAAATTT"),
            raw("seq2", b"ATG---TTT"),
            raw("seq3", b"ATGAAGTTT"),
        ]);
        alignment.set_filter("seq1|seq2".to_string()).unwrap();
        alignment.set_gap_filter(Some(0.5)).unwrap();
        alignment
            .set_translation(Some(libmsa::ReadingFrame::Frame2))
            .unwrap();

        assert_eq!(
            status_text(&build_bottom_status_bar(Some(&alignment), &ui_state())),
            "Filters: [rows: seq1|seq2] [gaps: <= 50%] (2 rows) (9 cols) | Translation frame: 2"
        );
    }

    #[test]
    fn bottom_status_constant_filter() {
        let mut alignment = alignment_model(vec![
            raw("seq1", b"CATCATCATCAT"),
            raw("seq2", b"CATCATCATCAT"),
            raw("seq3", b"CATCATCATCAT"),
        ]);
        alignment.set_constant_filter(Some(1.0)).unwrap();

        assert_eq!(
            status_text(&build_bottom_status_bar(Some(&alignment), &ui_state())),
            "Filters: [constant: >= 100%] (3 rows) (0 cols)"
        );
    }

    #[test]
    fn bottom_status_single_selection() {
        let alignment = alignment_model(vec![raw("seq1", b"ACGTACGT"), raw("seq2", b"ACGTACGT")]);
        let mut ui = ui_state();
        ui.selection = Some(MouseSelection {
            sequence_id: 0,
            column: 5,
            end_sequence_id: 0,
            end_column: 5,
        });

        assert_eq!(
            status_text(&build_bottom_status_bar(Some(&alignment), &ui)),
            "Selected: seq1 @ 6"
        );
    }

    #[test]
    fn bottom_status_single_range_selection() {
        let alignment = alignment_model(vec![raw("seq1", b"ACGTACGT"), raw("seq2", b"ACGTACGT")]);
        let mut ui = ui_state();
        ui.selection = Some(MouseSelection {
            sequence_id: 0,
            column: 2,
            end_sequence_id: 0,
            end_column: 8,
        });

        assert_eq!(
            status_text(&build_bottom_status_bar(Some(&alignment), &ui)),
            "Selected: seq1 @ 3-9"
        );
    }

    #[test]
    fn bottom_status_translated_codon_selection() {
        let mut alignment =
            alignment_model(vec![raw("seq1", b"ATGAAATTT"), raw("seq2", b"ATGAAATTT")]);
        alignment
            .set_translation_frame(libmsa::ReadingFrame::Frame1)
            .unwrap();
        alignment.toggle_translation_view().unwrap();
        let mut ui = ui_state();
        ui.selection = Some(MouseSelection {
            sequence_id: 0,
            column: 0,
            end_sequence_id: 0,
            end_column: 2,
        });

        assert_eq!(
            status_text(&build_bottom_status_bar(Some(&alignment), &ui)),
            "Translation frame: 1 | Selected: seq1 @ 1"
        );
    }

    #[test]
    fn bottom_status_translated_range_selection() {
        let mut alignment =
            alignment_model(vec![raw("seq1", b"ATGAAATTT"), raw("seq2", b"ATGAAATTT")]);
        alignment
            .set_translation_frame(libmsa::ReadingFrame::Frame1)
            .unwrap();
        alignment.toggle_translation_view().unwrap();
        let mut ui = ui_state();
        ui.selection = Some(MouseSelection {
            sequence_id: 0,
            column: 0,
            end_sequence_id: 0,
            end_column: 8,
        });

        assert_eq!(
            status_text(&build_bottom_status_bar(Some(&alignment), &ui)),
            "Translation frame: 1 | Selected: seq1 @ 1-3"
        );
    }

    #[test]
    fn bottom_status_multi_selection() {
        let mut ui = ui_state();
        ui.selection = Some(MouseSelection {
            sequence_id: 0,
            column: 1,
            end_sequence_id: 2,
            end_column: 4,
        });

        assert_eq!(
            status_text(&build_bottom_status_bar(None, &ui)),
            "3 sequence(s) selected @ 2-5"
        );
    }
}
