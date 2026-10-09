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
    let filter = state.mode().filter();
    let mut parts = Vec::new();

    if state.row_regex_filter.is_some() || filter.is_active() {
        let mut text = String::from("Filters:");
        if let Some(regex) = &state.row_regex_filter {
            let _ = write!(text, " [rows: {regex}]");
        }
        if let Some(fraction) = filter.max_gap_fraction {
            let _ = write!(text, " [gaps: <= {}%]", format_percent(fraction));
        }
        if let Some(fraction) = filter.min_const_fraction {
            let _ = write!(text, " [constant: >= {}%]", format_percent(fraction));
        }
        let _ = write!(text, " ({} rows)", layout.rows().len());
        if filter.is_active() {
            let _ = write!(text, " ({} cols)", layout.columns().len());
        }
        parts.push(text.set_style(theme.warning));
    }

    if !matches!(state.mode(), ViewMode::Plain { .. }) {
        parts.push(format!("Translation frame: {}", state.frame).set_style(theme.text));
    }

    if let Some(selection) = ui.selection {
        let count = selection.rows.last - selection.rows.start + 1;
        let first = layout.columns()[selection.columns.start];
        let last = layout.columns()[selection.columns.last];
        let columns = range_label(first + 1, last + 1);

        let text = if count == 1 {
            let protein = session.protein_columns(first..last + 1);
            let position =
                if matches!(state.mode(), ViewMode::TranslationOverlay) && !protein.is_empty() {
                    range_label(protein.start + 1, protein.end)
                } else {
                    columns
                };
            let name = session
                .base_alignment()
                .id(layout.rows()[selection.rows.start]);
            format!(
                "Selected: {} @ {position}",
                truncate_label(name, STATUS_BAR_SELECTED_NAME_MAX_CHARS)
            )
        } else {
            format!("{count} sequence(s) selected @ {columns}")
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
    let file_name = ui.meta.input_path.as_deref().map_or("Unknown", |input| {
        // for local paths, show just the file name for URLs makes more sense to show the full input.
        std::path::Path::new(input)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(input)
    });

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
    use std::ops;

    use super::*;
    use crate::{
        core::session::{Position, ViewState},
        test_utils::{full_window, session_with_ids, ui_state},
        ui::{layout::Window, selection::Selection},
    };

    fn status_text(spans: &[Span<'_>]) -> String {
        spans.iter().map(|span| span.content.as_ref()).collect()
    }

    fn seqs(sequences: &[&[u8]]) -> Session {
        let ids: Vec<String> = (1..=sequences.len()).map(|i| format!("seq{i}")).collect();
        let sequences: Vec<(&str, &[u8])> = ids
            .iter()
            .map(String::as_str)
            .zip(sequences.iter().copied())
            .collect();
        session_with_ids(&sequences)
    }

    fn changed(
        sequences: &[&[u8]],
        change: impl FnOnce(&mut ViewState) -> anyhow::Result<()>,
    ) -> Session {
        let mut session = seqs(sequences);
        session.update(Position::default(), change).unwrap();
        session
    }

    fn bottom_with_selection(
        session: &Session,
        rows: ops::RangeInclusive<usize>,
        columns: ops::RangeInclusive<usize>,
    ) -> String {
        let mut ui = ui_state();
        ui.selection = Some(Selection {
            rows: rows.into(),
            columns: columns.into(),
        });
        status_text(&build_bottom_status_bar(Some(session), &ui))
    }

    #[test]
    fn top_loaded() {
        let session = seqs(&[b"ACGTACGT", b"ACGTAC-T", b"ACGTACGA"]);
        let mut ui = ui_state();
        ui.meta.input_path = Some("/iamnotreal.fasta".to_string());
        ui.meta.loading_state = LoadingState::Loaded;
        ui.window = Window {
            columns: 0..5,
            ..full_window(&session)
        };

        insta::assert_snapshot!(status_text(&build_top_status_bar(Some(&session), &ui)));
    }

    #[test]
    fn top_failed() {
        let mut ui = ui_state();
        ui.meta.loading_state = LoadingState::Failed("boom".to_string());

        insta::assert_snapshot!(status_text(&build_top_status_bar(None, &ui)));
    }

    #[test]
    fn bottom_protein_filters() {
        let session = changed(&[b"ATGAAATTT", b"ATG---TTT", b"ATGAAGTTT"], |state| {
            state.row_regex_filter = Some(regex::Regex::new("seq1|seq2").unwrap());
            state.frame = libmsa::ReadingFrame::Frame2;
            state.toggle_protein_view()?;
            state.set_gap_filter(Some(0.5))
        });

        insta::assert_snapshot!(status_text(&build_bottom_status_bar(
            Some(&session),
            &ui_state()
        )));
    }

    #[test]
    fn bottom_constant_filter() {
        let cat: &[u8] = b"CATCATCATCAT";
        let session = changed(&[cat, cat, cat], |state| {
            state.set_constant_filter(Some(1.0))
        });

        insta::assert_snapshot!(status_text(&build_bottom_status_bar(
            Some(&session),
            &ui_state()
        )));
    }

    #[test]
    fn bottom_cell() {
        let session = seqs(&[b"ACGTACGT", b"ACGTACGT"]);

        insta::assert_snapshot!(bottom_with_selection(&session, 0..=0, 5..=5));
    }

    #[test]
    fn bottom_range() {
        let session = seqs(&[b"ACGTACGTAC", b"ACGTACGTAC"]);

        insta::assert_snapshot!(bottom_with_selection(&session, 0..=0, 2..=8));
    }

    #[test]
    fn bottom_codon() {
        let session = changed(
            &[b"ATGAAATTT", b"ATGAAATTT"],
            ViewState::toggle_translation_overlay,
        );

        insta::assert_snapshot!(bottom_with_selection(&session, 0..=0, 0..=2));
    }

    #[test]
    fn bottom_translated_range() {
        let session = changed(
            &[b"ATGAAATTT", b"ATGAAATTT"],
            ViewState::toggle_translation_overlay,
        );

        insta::assert_snapshot!(bottom_with_selection(&session, 0..=0, 0..=8));
    }

    #[test]
    fn bottom_rows() {
        let session = seqs(&[b"ACGTACGT", b"ACGTACGT", b"ACGTACGT"]);

        insta::assert_snapshot!(bottom_with_selection(&session, 0..=2, 1..=4));
    }

    #[test]
    fn bottom_rows_filtered() {
        let session = changed(&[b"ACGTACGT", b"ACGTACGT", b"ACGTACGT"], |state| {
            state.row_regex_filter = Some(regex::Regex::new("seq[13]").unwrap());
            Ok(())
        });

        insta::assert_snapshot!(bottom_with_selection(&session, 0..=1, 1..=4));
    }
}
