pub mod alignment;
pub mod consensus;
mod features;
pub mod gff;
pub mod local_feature_track;
mod ruler;
pub mod sequence_id;

use std::ops::Range;

use libmsa::residue;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    symbols::merge::MergeStrategy,
    text::{Line, Span},
    widgets::{Block, Scrollbar, ScrollbarOrientation, ScrollbarState, StatefulWidget, Widget},
};

use self::{
    alignment::AlignmentPane,
    consensus::{ConsensusAlignmentPane, ConsensusSequenceIdPane},
    gff::{GffInfoPane, GffPane},
    sequence_id::SequenceIdPane,
};
use crate::{
    config::theme::Theme,
    gff::Gff,
    session::{Cell, Session, WindowColumns},
    ui::{UiState, layout::ScreenLayout},
};

// the text drawn for each byte: printable ASCII shows as itself, anything else as a question mark
// TODO: this was for drawing IUPAC codes quickly, so revisit it now all printable ASCII maps
// to itself
const BYTE_TO_CHAR: [&str; 256] = [
    "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?",
    "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", " ", "!", "\"", "#", "$", "%",
    "&", "'", "(", ")", "*", "+", ",", "-", ".", "/", "0", "1", "2", "3", "4", "5", "6", "7", "8",
    "9", ":", ";", "<", "=", ">", "?", "@", "A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K",
    "L", "M", "N", "O", "P", "Q", "R", "S", "T", "U", "V", "W", "X", "Y", "Z", "[", "\\", "]", "^",
    "_", "`", "a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "l", "m", "n", "o", "p", "q",
    "r", "s", "t", "u", "v", "w", "x", "y", "z", "{", "|", "}", "~", "?", "?", "?", "?", "?", "?",
    "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?",
    "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?",
    "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?",
    "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?",
    "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?",
    "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?",
    "?", "?", "?", "?", "?", "?", "?", "?", "?",
];

pub fn render(
    buf: &mut Buffer,
    screen: &ScreenLayout,
    session: &Session,
    gff: Option<&Gff>,
    ui: &UiState,
    theme: &Theme,
) {
    let (layout, window) = (&screen.app, &screen.window);
    let columns = session.window_columns(window.columns.clone());

    if let Some(gff) = gff {
        GffInfoPane {
            tooltip: ui.gff_tooltip.as_deref(),
            theme,
        }
        .render(layout.gff_info_pane, buf);
        GffPane {
            gff,
            session,
            window,
            theme,
        }
        .render(layout.gff_pane, buf);
    }

    SequenceIdPane {
        session,
        window,
        local_feature_rows: layout.local_feature_rows,
        selection: ui.selection,
        theme,
    }
    .render(layout.sequence_id_pane, buf);
    AlignmentPane {
        session,
        window,
        columns: &columns,
        gff,
        local_feature_rows: layout.local_feature_rows,
        selection: ui.selection,
        theme,
    }
    .render(layout.alignment_pane, buf);
    ConsensusSequenceIdPane { theme }.render(layout.consensus_sequence_id_pane, buf);
    ConsensusAlignmentPane {
        session,
        columns: &columns,
        theme,
    }
    .render(layout.consensus_alignment_pane, buf);
}

/// a residue that matches `diff` shows as `.`
///
/// in the translation overlay a codon is coloured across its three columns, and only the middle one
/// shows the amino acid
pub fn sequence_line(
    columns: &WindowColumns<'_>,
    bytes: &[u8],
    diff: Option<&[u8]>,
    theme: &Theme,
) -> Line<'static> {
    let alignment_type = columns.grid.alignment_type();
    columns
        .cells
        .iter()
        .map(|cell| {
            let Some(Cell { index, centre }) = *cell else {
                return Span::raw(" ");
            };
            let byte = bytes[index];
            let matches =
                diff.is_some_and(|diff| residue::same_residue(diff[index], byte, alignment_type));
            let style = theme.residues.style(alignment_type, byte);
            match (matches, centre) {
                (true, true) => Span::styled(".", theme.diff_match),
                (true, false) => Span::raw(" "),
                (false, true) => Span::styled(BYTE_TO_CHAR[usize::from(byte)], style),
                // the rest of a codon is blank but keeps its colour, so it reads as one block
                (false, false) => Span::styled(" ", style),
            }
        })
        .collect()
}

/// only draws when the columns don't all fit
pub fn render_column_scrollbar(
    theme: &Theme,
    total: usize,
    window: &Range<usize>,
    area: Rect,
    buf: &mut Buffer,
) {
    if total <= window.len() {
        return;
    }

    Scrollbar::new(ScrollbarOrientation::HorizontalBottom)
        .begin_symbol(None)
        .end_symbol(None)
        .track_symbol(None)
        .thumb_symbol("▬")
        .thumb_style(theme.scrollbar)
        .render(
            area,
            buf,
            &mut ScrollbarState::new(total - window.len() + 1)
                .position(window.start)
                .viewport_content_length(window.len()),
        );
}

pub fn render_pane(theme: &Theme, title: Option<Line<'_>>, area: Rect, buf: &mut Buffer) -> Rect {
    let mut block = Block::bordered()
        .border_style(theme.border)
        .style(theme.base)
        .merge_borders(MergeStrategy::Exact);
    if let Some(title) = title {
        block = block.title(title);
    }
    let inner_area = block.inner(area);
    block.render(area, buf);
    inner_area
}

pub fn separator_line(width: u16, theme: &Theme) -> Line<'static> {
    use ratatui::style::Styled;

    Line::from("─".repeat(usize::from(width)).set_style(theme.border))
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;
    use crate::test_utils::session;

    const CENTRE: Option<Cell> = Some(Cell {
        index: 0,
        centre: true,
    });
    const FLANK: Option<Cell> = Some(Cell {
        index: 0,
        centre: false,
    });

    fn line_text(cell: Option<Cell>, byte: u8, diff: Option<u8>) -> String {
        let session = session(&[b"A"]);
        let columns = WindowColumns {
            grid: session.grid(),
            columns: vec![0],
            cells: vec![cell],
            consensus: Vec::new(),
            conservation: None,
        };
        let diff = diff.map(|byte| vec![byte]);

        sequence_line(&columns, &[byte], diff.as_deref(), &Theme::default()).to_string()
    }

    #[rstest]
    #[case::centre_shows_letter(CENTRE, b'A', None, "A")]
    #[case::flank_is_blank(FLANK, b'A', None, " ")]
    #[case::no_cell_is_blank(None, b'A', None, " ")]
    #[case::diff_match_centre_is_dot(CENTRE, b'A', Some(b'A'), ".")]
    #[case::diff_match_flank_is_blank(FLANK, b'A', Some(b'A'), " ")]
    #[case::diff_mismatch_shows_letter(CENTRE, b'G', Some(b'A'), "G")]
    #[case::diff_ignores_case(CENTRE, b'a', Some(b'A'), ".")]
    #[case::unprintable_is_question_mark(CENTRE, 0x07, None, "?")]
    fn sequence_line_works(
        #[case] cell: Option<Cell>,
        #[case] byte: u8,
        #[case] diff: Option<u8>,
        #[case] expected: &str,
    ) {
        assert_eq!(line_text(cell, byte, diff), expected);
    }

    #[test]
    fn sequence_line_reads_byte_at_cell_index() {
        let session = session(&[b"AC"]);
        let columns = WindowColumns {
            grid: session.grid(),
            columns: vec![0, 1],
            cells: vec![
                Some(Cell {
                    index: 1,
                    centre: true,
                }),
                Some(Cell {
                    index: 0,
                    centre: true,
                }),
            ],
            consensus: Vec::new(),
            conservation: None,
        };

        let line = sequence_line(&columns, b"AC", Some(b"AG"), &Theme::default());

        assert_eq!(line.to_string(), "C.");
    }
}
