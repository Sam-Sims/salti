use std::ops::Range;

use libmsa::ReadingFrame;

/// A single codon visible in the current viewport.
///
/// All fields use absolute nucleotide column coordinates except `protein_col`
/// which is a protein-space index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VisibleCodon {
    /// Protein-space column index for this codon.
    pub protein_col: usize,
    /// Absolute nucleotide column where the codon begins.
    pub nuc_start: usize,
    /// Absolute nucleotide column that displays the amino-acid letter
    /// (the centre cell of the three-wide codon span).
    pub centre: usize,
}

/// Context for the translation visual overlay.
///
/// Computed once per render frame from the active reading frame and the
/// alignment's nucleotide column count, then threaded through the rendering
/// pipeline. All methods operate in nucleotide-space viewport coordinates.
#[derive(Debug, Clone, Copy)]
pub struct TranslationOverlay {
    /// Active reading frame for the overlay.
    pub frame: ReadingFrame,
    /// Total nucleotide columns in the current alignment view.
    pub nucleotide_len: usize,
}

impl TranslationOverlay {
    /// Protein column range overlapping a nucleotide viewport range.
    pub fn visible_protein_range(&self, nuc_range: &Range<usize>) -> Option<Range<usize>> {
        visible_protein_range(nuc_range, self.frame, self.nucleotide_len)
    }

    /// Iterator of [`VisibleCodon`]s for a nucleotide viewport range.
    pub fn visible_codons(&self, nuc_range: &Range<usize>) -> impl Iterator<Item = VisibleCodon> {
        visible_codons(nuc_range, self.frame, self.nucleotide_len)
    }

    /// The nucleotide range `[start..start+3)` for an absolute nucleotide
    /// column's enclosing codon. Returns `None` for incomplete codons or
    /// columns before the frame offset.
    pub fn codon_span(&self, absolute_col: usize) -> Option<Range<usize>> {
        codon_span_for_absolute_column(absolute_col, self.frame, self.nucleotide_len)
    }
}

/// A window into a pre-translated byte slice, offset by `start` in
/// protein-column space.
///
/// Used for diff-against and consensus rendering in translation mode.
#[derive(Debug, Clone, Copy)]
pub struct TranslatedByteRange<'a> {
    start: usize,
    bytes: &'a [u8],
}

impl<'a> TranslatedByteRange<'a> {
    /// Creates a translated byte window starting at `start` in protein space.
    pub const fn new(start: usize, bytes: &'a [u8]) -> Self {
        Self { start, bytes }
    }

    /// Returns the byte at `protein_col`, if it lies within this window.
    pub fn byte_at(self, protein_col: usize) -> Option<u8> {
        let offset = protein_col.checked_sub(self.start)?;
        self.bytes.get(offset).copied()
    }
}

/// Translated bytes used for diff rendering.
pub type TranslatedDiffRange<'a> = TranslatedByteRange<'a>;

/// Number of complete three-nucleotide codons. Delegates to
/// [`ReadingFrame::complete_codons`].
pub const fn complete_protein_len(frame: ReadingFrame, nucleotide_len: usize) -> usize {
    frame.complete_codons(nucleotide_len)
}

/// Protein column range overlapping a nucleotide viewport range.
///
/// Returns `None` when no complete codons are visible.
pub fn visible_protein_range(
    visible_nuc_range: &Range<usize>,
    frame: ReadingFrame,
    nucleotide_len: usize,
) -> Option<Range<usize>> {
    let last_visible_col = visible_nuc_range.end.checked_sub(1)?;
    if last_visible_col < frame.offset() {
        return None;
    }

    let protein_len = complete_protein_len(frame, nucleotide_len);
    if protein_len == 0 {
        return None;
    }

    let start = visible_nuc_range.start.saturating_sub(frame.offset()) / 3;
    let end = ((last_visible_col - frame.offset()) / 3 + 1).min(protein_len);

    (start < end).then_some(start..end)
}

/// Iterator of [`VisibleCodon`]s for a nucleotide viewport range.
///
/// Only complete codons are yielded.
pub fn visible_codons(
    visible_nuc_range: &Range<usize>,
    frame: ReadingFrame,
    nucleotide_len: usize,
) -> impl Iterator<Item = VisibleCodon> {
    visible_protein_range(visible_nuc_range, frame, nucleotide_len)
        .into_iter()
        .flatten()
        .map(move |protein_col| {
            let nuc_start = nuc_start(protein_col, frame);
            VisibleCodon {
                protein_col,
                nuc_start,
                centre: nuc_start + 1,
            }
        })
}

/// The nucleotide range `[start..start+3)` for an absolute nucleotide
/// column's enclosing codon.
///
/// Returns `None` when the column lies before the frame offset or
/// the codon extends past the alignment width (incomplete terminal codon).
pub fn codon_span_for_absolute_column(
    absolute_col: usize,
    frame: ReadingFrame,
    nucleotide_len: usize,
) -> Option<Range<usize>> {
    let offset = frame.offset();
    if absolute_col < offset {
        return None;
    }

    let codon_start = offset + ((absolute_col - offset) / 3) * 3;
    let codon_end = codon_start + 3;
    (codon_end <= nucleotide_len).then_some(codon_start..codon_end)
}

/// First nucleotide index of a protein column:
/// `frame.offset() + protein_col * 3`.
pub fn nuc_start(protein_col: usize, frame: ReadingFrame) -> usize {
    frame.offset() + protein_col * 3
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visible_protein_range_includes_complete_codons_overlapping_window() {
        let range = visible_protein_range(&(1..8), ReadingFrame::Frame1, 9);
        assert_eq!(range, Some(0..3));

        let range = visible_protein_range(&(0..2), ReadingFrame::Frame3, 9);
        assert!(range.is_none());
    }

    #[test]
    fn codon_span_maps_any_column_in_the_same_codon() {
        let frame = ReadingFrame::Frame1;

        assert_eq!(codon_span_for_absolute_column(0, frame, 9), Some(0..3));
        assert_eq!(codon_span_for_absolute_column(1, frame, 9), Some(0..3));
        assert_eq!(codon_span_for_absolute_column(2, frame, 9), Some(0..3));
        assert_eq!(codon_span_for_absolute_column(3, frame, 9), Some(3..6));
    }

    #[test]
    fn codon_span_returns_none_for_partial_frame_edges() {
        let frame = ReadingFrame::Frame2;

        assert_eq!(codon_span_for_absolute_column(0, frame, 9), None);
        assert_eq!(codon_span_for_absolute_column(8, frame, 9), None);
    }

    #[test]
    fn nuc_start_is_frame_offset_plus_triple() {
        assert_eq!(nuc_start(0, ReadingFrame::Frame1), 0);
        assert_eq!(nuc_start(1, ReadingFrame::Frame1), 3);
        assert_eq!(nuc_start(0, ReadingFrame::Frame2), 1);
        assert_eq!(nuc_start(1, ReadingFrame::Frame2), 4);
        assert_eq!(nuc_start(0, ReadingFrame::Frame3), 2);
    }

    #[test]
    fn visible_codons_yields_correct_codons() {
        let codons: Vec<VisibleCodon> = visible_codons(&(0..9), ReadingFrame::Frame1, 9).collect();
        assert_eq!(codons.len(), 3);
        assert_eq!(
            codons[0],
            VisibleCodon {
                protein_col: 0,
                nuc_start: 0,
                centre: 1,
            }
        );
        assert_eq!(
            codons[1],
            VisibleCodon {
                protein_col: 1,
                nuc_start: 3,
                centre: 4,
            }
        );
        assert_eq!(
            codons[2],
            VisibleCodon {
                protein_col: 2,
                nuc_start: 6,
                centre: 7,
            }
        );
    }

    #[test]
    fn translated_byte_range_resolves_offset() {
        let range = TranslatedByteRange::new(2, b"MKF");
        assert_eq!(range.byte_at(2), Some(b'M'));
        assert_eq!(range.byte_at(3), Some(b'K'));
        assert_eq!(range.byte_at(4), Some(b'F'));
        assert_eq!(range.byte_at(1), None);
        assert_eq!(range.byte_at(5), None);
    }
}
