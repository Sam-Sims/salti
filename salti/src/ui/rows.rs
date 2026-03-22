use std::ops::Range;

use crate::config::theme::SequenceTheme;
use ratatui::style::Stylize;
use ratatui::text::Span;

/// Lookup table that maps each byte value (`0-255`) to a str for display.
///
/// All printable ASCII bytes map to themselves.
/// Any byte not mapped to one of those outputs is rendered as `"?"`.
// TODO: This was originally for quick rendering of IUPAC bases and handling any chars outside of those.
// Revisit this now we have full ASCII mapping.
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

#[derive(Debug, Clone, Copy)]
pub struct RowRenderMode<'a> {
    pub alignment_type: libmsa::AlignmentType,
    pub diff_against: Option<&'a [u8]>,
}

#[derive(Debug, Clone, Copy)]
pub struct TranslatedByteRange<'a> {
    start: usize,
    bytes: &'a [u8],
}

impl<'a> TranslatedByteRange<'a> {
    pub fn new(start: usize, bytes: &'a [u8]) -> Self {
        Self { start, bytes }
    }

    fn byte_at(self, protein_col: usize) -> Option<u8> {
        let offset = protein_col.checked_sub(self.start)?;
        self.bytes.get(offset).copied()
    }
}

pub type TranslatedDiffRange<'a> = TranslatedByteRange<'a>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct VisibleCodon {
    protein_col: usize,
    nuc_start: usize,
    centre: usize,
}

#[inline]
fn span_for_sequence_byte(
    sequence_byte: u8,
    sequence_theme: &SequenceTheme,
    alignment_type: libmsa::AlignmentType,
) -> Span<'static> {
    let character = BYTE_TO_CHAR[usize::from(sequence_byte)];
    let style = sequence_theme.style_for(sequence_byte, alignment_type);
    Span::styled(character, style)
}

#[inline]
fn format_visible_bytes(
    bytes: &[u8],
    sequence_theme: &SequenceTheme,
    alignment_type: libmsa::AlignmentType,
) -> Vec<Span<'static>> {
    bytes
        .iter()
        .map(|&byte| span_for_sequence_byte(byte, sequence_theme, alignment_type))
        .collect()
}

#[inline]
fn format_visible_bytes_with_diff(
    bytes: &[u8],
    diff_against: &[u8],
    sequence_theme: &SequenceTheme,
    alignment_type: libmsa::AlignmentType,
) -> Vec<Span<'static>> {
    assert_eq!(
        bytes.len(),
        diff_against.len(),
        "diff bytes must match the visible width"
    );

    bytes
        .iter()
        .zip(diff_against.iter())
        .map(|(&byte, &diff_byte)| {
            if byte == diff_byte {
                ".".fg(sequence_theme.diff_match)
            } else {
                span_for_sequence_byte(byte, sequence_theme, alignment_type)
            }
        })
        .collect()
}

pub fn format_row_spans(
    visible_bytes: &[u8],
    sequence_theme: &SequenceTheme,
    mode: RowRenderMode<'_>,
) -> Vec<Span<'static>> {
    match mode.diff_against {
        Some(diff_against) => format_visible_bytes_with_diff(
            visible_bytes,
            diff_against,
            sequence_theme,
            mode.alignment_type,
        ),
        None => format_visible_bytes(visible_bytes, sequence_theme, mode.alignment_type),
    }
}

fn complete_protein_len(frame: libmsa::ReadingFrame, nucleotide_len: usize) -> usize {
    nucleotide_len.saturating_sub(frame.offset()) / 3
}

pub fn visible_protein_range(
    visible_nucleotide_range: &Range<usize>,
    frame: libmsa::ReadingFrame,
    nucleotide_len: usize,
) -> Option<Range<usize>> {
    let last_visible_col = visible_nucleotide_range.end.checked_sub(1)?;
    if last_visible_col < frame.offset() {
        return None;
    }

    let protein_len = complete_protein_len(frame, nucleotide_len);
    if protein_len == 0 {
        return None;
    }

    let start = visible_nucleotide_range
        .start
        .saturating_sub(frame.offset())
        / 3;
    let end = ((last_visible_col - frame.offset()) / 3 + 1).min(protein_len);

    (start < end).then_some(start..end)
}

fn visible_codons(
    visible_nucleotide_range: &Range<usize>,
    frame: libmsa::ReadingFrame,
    nucleotide_len: usize,
) -> impl Iterator<Item = VisibleCodon> {
    visible_protein_range(visible_nucleotide_range, frame, nucleotide_len)
        .into_iter()
        .flatten()
        .map(move |protein_col| {
            let nuc_start = frame.offset() + protein_col * 3;
            VisibleCodon {
                protein_col,
                nuc_start,
                centre: nuc_start + 1,
            }
        })
}

pub fn format_translated_row_spans(
    sequence: libmsa::TranslatedSequenceView<'_>,
    visible_nucleotide_range: &Range<usize>,
    nucleotide_len: usize,
    frame: libmsa::ReadingFrame,
    sequence_theme: &SequenceTheme,
    diff_against: Option<TranslatedDiffRange<'_>>,
) -> Vec<Span<'static>> {
    format_translated_spans(
        visible_nucleotide_range,
        nucleotide_len,
        frame,
        sequence_theme,
        diff_against,
        |protein_col| sequence.byte_at(protein_col),
    )
}

pub fn format_translated_byte_range_spans(
    bytes: TranslatedByteRange<'_>,
    visible_nucleotide_range: &Range<usize>,
    nucleotide_len: usize,
    frame: libmsa::ReadingFrame,
    sequence_theme: &SequenceTheme,
    diff_against: Option<TranslatedDiffRange<'_>>,
) -> Vec<Span<'static>> {
    format_translated_spans(
        visible_nucleotide_range,
        nucleotide_len,
        frame,
        sequence_theme,
        diff_against,
        |protein_col| bytes.byte_at(protein_col),
    )
}

fn format_translated_spans(
    visible_nucleotide_range: &Range<usize>,
    nucleotide_len: usize,
    frame: libmsa::ReadingFrame,
    sequence_theme: &SequenceTheme,
    diff_against: Option<TranslatedDiffRange<'_>>,
    mut byte_at: impl FnMut(usize) -> Option<u8>,
) -> Vec<Span<'static>> {
    let width = visible_nucleotide_range.len();
    let mut spans = vec![Span::raw(" "); width];

    for codon in visible_codons(visible_nucleotide_range, frame, nucleotide_len) {
        let residue = byte_at(codon.protein_col).expect("visible codon must resolve");
        let translated_style = sequence_theme.style_for(residue, libmsa::AlignmentType::Protein);
        let diff_matches =
            diff_against.and_then(|diff| diff.byte_at(codon.protein_col)) == Some(residue);

        for absolute_col in codon.nuc_start..=codon.nuc_start + 2 {
            let Some(window_offset) = absolute_col.checked_sub(visible_nucleotide_range.start)
            else {
                continue;
            };
            if window_offset >= width {
                continue;
            }

            spans[window_offset] = if diff_matches {
                if absolute_col == codon.centre {
                    ".".fg(sequence_theme.diff_match)
                } else {
                    Span::raw(" ")
                }
            } else if absolute_col == codon.centre {
                Span::styled(BYTE_TO_CHAR[usize::from(residue)], translated_style)
            } else {
                Span::styled(" ", translated_style)
            };
        }
    }

    spans
}

/// Collects visible bytes from a sequence view for the given relative column range.
pub fn visible_bytes(sequence: libmsa::SequenceView<'_>, col_range: &Range<usize>) -> Vec<u8> {
    if col_range.is_empty() {
        return Vec::new();
    }

    sequence
        .indexed_bytes_range(col_range.clone())
        .expect("viewport range must be within the current view")
        .map(|(_, byte)| byte)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(id: &str, sequence: &[u8]) -> libmsa::RawSequence {
        libmsa::RawSequence {
            id: id.to_string(),
            sequence: sequence.to_vec(),
        }
    }

    fn spans_text(spans: &[Span<'_>]) -> String {
        spans.iter().map(|span| span.content.as_ref()).collect()
    }

    #[test]
    fn visible_protein_range_includes_complete_codons_overlapping_window() {
        let range = visible_protein_range(&(1..8), libmsa::ReadingFrame::Frame1, 9);
        assert_eq!(range, Some(0..3));

        let range = visible_protein_range(&(0..2), libmsa::ReadingFrame::Frame3, 9);
        assert!(range.is_none());
    }

    #[test]
    fn translated_row_spans_render_codons_across_nucleotide_width() {
        let alignment = libmsa::Alignment::new(vec![raw("seq1", b"ATGAAATTT")])
            .expect("test alignment should be valid");
        let sequence = alignment
            .translated(libmsa::ReadingFrame::Frame1)
            .expect("DNA alignment should translate")
            .sequence_by_absolute(0)
            .expect("visible row should resolve");

        let spans = format_translated_row_spans(
            sequence,
            &(0..9),
            9,
            libmsa::ReadingFrame::Frame1,
            &crate::config::theme::EVERFOREST_DARK.sequence,
            None,
        );

        assert_eq!(spans.len(), 9);
        assert_eq!(spans_text(&spans), " M  K  F ");
    }

    #[test]
    fn translated_row_spans_render_diff_matches_in_centre_cells_only() {
        let alignment = libmsa::Alignment::new(vec![raw("seq1", b"ATGCCCTTT")])
            .expect("test alignment should be valid");
        let sequence = alignment
            .translated(libmsa::ReadingFrame::Frame1)
            .expect("DNA alignment should translate")
            .sequence_by_absolute(0)
            .expect("visible row should resolve");
        let diff_against = TranslatedDiffRange::new(0, b"MKF");

        let spans = format_translated_row_spans(
            sequence,
            &(0..9),
            9,
            libmsa::ReadingFrame::Frame1,
            &crate::config::theme::EVERFOREST_DARK.sequence,
            Some(diff_against),
        );

        assert_eq!(spans_text(&spans), " .  P  . ");
    }

    #[test]
    fn translated_row_spans_leave_diff_match_flanks_unstyled() {
        let alignment = libmsa::Alignment::new(vec![raw("seq1", b"ATGAAATTT")])
            .expect("test alignment should be valid");
        let sequence = alignment
            .translated(libmsa::ReadingFrame::Frame1)
            .expect("DNA alignment should translate")
            .sequence_by_absolute(0)
            .expect("visible row should resolve");
        let diff_against = TranslatedDiffRange::new(0, b"MKF");

        let spans = format_translated_row_spans(
            sequence,
            &(0..9),
            9,
            libmsa::ReadingFrame::Frame1,
            &crate::config::theme::EVERFOREST_DARK.sequence,
            Some(diff_against),
        );

        assert_eq!(spans[0].style, ratatui::style::Style::default());
        assert_eq!(spans[2].style, ratatui::style::Style::default());
        assert_eq!(spans[3].style, ratatui::style::Style::default());
        assert_eq!(spans[5].style, ratatui::style::Style::default());
    }

    #[test]
    fn translated_row_spans_leave_incomplete_frame_edges_blank() {
        let alignment = libmsa::Alignment::new(vec![raw("seq1", b"AATGAAATT")])
            .expect("test alignment should be valid");
        let sequence = alignment
            .translated(libmsa::ReadingFrame::Frame2)
            .expect("DNA alignment should translate")
            .sequence_by_absolute(0)
            .expect("visible row should resolve");

        let spans = format_translated_row_spans(
            sequence,
            &(0..9),
            9,
            libmsa::ReadingFrame::Frame2,
            &crate::config::theme::EVERFOREST_DARK.sequence,
            None,
        );

        assert_eq!(spans_text(&spans), "  M  K   ");
    }
}
