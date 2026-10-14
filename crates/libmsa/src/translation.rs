use std::ops::Range;

use crate::residue::{UNKNOWN_AMINO_ACID, nucleotide_index};

const STANDARD_CODONS: [[[u8; 4]; 4]; 4] = [
    [*b"KNNK", *b"IIIM", *b"TTTT", *b"RSSR"],
    [*b"*YY*", *b"LFFL", *b"SSSS", *b"*CCW"],
    [*b"QHHQ", *b"LLLL", *b"PPPP", *b"RRRR"],
    [*b"EDDE", *b"VVVV", *b"AAAA", *b"GGGG"],
];

/// A reading frame, which sets the nucleotide column the first codon starts at and the strand
/// its codons are read on
///
/// A reverse frame uses the same codon columns as the forward frame with the same number, and
/// reads each codon backwards and complemented
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, strum::Display, strum::EnumString, strum::VariantArray,
)]
pub enum ReadingFrame {
    #[strum(serialize = "+1", serialize = "1")]
    Frame1,
    #[strum(serialize = "+2", serialize = "2")]
    Frame2,
    #[strum(serialize = "+3", serialize = "3")]
    Frame3,
    #[strum(serialize = "-1")]
    Frame1Reverse,
    #[strum(serialize = "-2")]
    Frame2Reverse,
    #[strum(serialize = "-3")]
    Frame3Reverse,
}

impl ReadingFrame {
    /// Returns the nucleotide column the first codon starts at
    pub const fn offset(self) -> usize {
        match self {
            Self::Frame1 | Self::Frame1Reverse => 0,
            Self::Frame2 | Self::Frame2Reverse => 1,
            Self::Frame3 | Self::Frame3Reverse => 2,
        }
    }

    /// Returns the strand the codons are read on
    pub(crate) const fn strand(self) -> Strand {
        match self {
            Self::Frame1 | Self::Frame2 | Self::Frame3 => Strand::Forward,
            Self::Frame1Reverse | Self::Frame2Reverse | Self::Frame3Reverse => Strand::Reverse,
        }
    }

    /// Returns the nucleotides columns of the codon at `protein_col`
    pub const fn nt_range(self, protein_col: usize) -> Range<usize> {
        let start = self.offset() + 3 * protein_col;
        start..start + 3
    }

    /// Returns the protein columns whose codons overlap `nt_range`, clipped to the translated
    /// length of `nt_width`
    ///
    /// `nt_range` must not be empty. Nucleotides before the offset belong to no codon, so a range
    /// that ends before it gives `0..0` and a start before it maps to 0
    pub fn protein_range(self, nt_range: Range<usize>, nt_width: usize) -> Range<usize> {
        debug_assert!(!nt_range.is_empty(), "dont give an empty nt range");
        let len = self.translated_length(nt_width);
        let start = self.protein_col(nt_range.start).unwrap_or(0).min(len);
        let end = self
            .protein_col(nt_range.end - 1)
            .map_or(0, |p| p + 1)
            .min(len);
        start..end
    }

    /// Returns the protein column that `absolute_nuc_col` falls in, or `None` before the offset
    pub const fn protein_col(self, absolute_nuc_col: usize) -> Option<usize> {
        let offset = self.offset();
        if absolute_nuc_col < offset {
            return None;
        }

        Some((absolute_nuc_col - offset) / 3)
    }

    /// Returns the number of protein columns for `nucleotide_length` nucleotide columns
    ///
    /// A trailing partial codon counts as one column, which reads as `X`
    pub(crate) const fn translated_length(self, nucleotide_length: usize) -> usize {
        let offset = self.offset();
        if nucleotide_length <= offset {
            return 0;
        }

        ((nucleotide_length - 1 - offset) / 3) + 1
    }
}

/// The strand a reading frame translates
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Strand {
    Forward,
    Reverse,
}

pub(crate) fn codon_at(sequence: &[u8], codon: Range<usize>, strand: Strand) -> u8 {
    assert!(codon.start < sequence.len(), "col should be below width");
    match sequence.get(codon) {
        Some(&[a, b, c]) => translate_codon([a, b, c], strand),
        _ => UNKNOWN_AMINO_ACID,
    }
}

fn translate_codon(codon: [u8; 3], strand: Strand) -> u8 {
    let [Some(first), Some(second), Some(third)] = codon.map(nucleotide_index) else {
        return UNKNOWN_AMINO_ACID;
    };
    match strand {
        Strand::Forward => STANDARD_CODONS[first][second][third],
        // can xor here since bases are ordered ATCG - never change this or this breaks
        Strand::Reverse => STANDARD_CODONS[third ^ 1][second ^ 1][first ^ 1],
    }
}

#[cfg(test)]
mod tests {
    use std::ops::Range;

    use rstest::rstest;

    use super::{ReadingFrame, Strand, codon_at, translate_codon};

    // https://www.hgmd.cf.ac.uk/docs/cd_amino.html
    // full lookup table provided here as a secondary source of truth to compare the "production" STANDARD_CODONS against
    const STANDARD_TABLE: [([u8; 3], u8); 64] = [
        (*b"TTT", b'F'),
        (*b"TTC", b'F'),
        (*b"TTA", b'L'),
        (*b"TTG", b'L'),
        (*b"TCT", b'S'),
        (*b"TCC", b'S'),
        (*b"TCA", b'S'),
        (*b"TCG", b'S'),
        (*b"TAT", b'Y'),
        (*b"TAC", b'Y'),
        (*b"TAA", b'*'),
        (*b"TAG", b'*'),
        (*b"TGT", b'C'),
        (*b"TGC", b'C'),
        (*b"TGA", b'*'),
        (*b"TGG", b'W'),
        (*b"CTT", b'L'),
        (*b"CTC", b'L'),
        (*b"CTA", b'L'),
        (*b"CTG", b'L'),
        (*b"CCT", b'P'),
        (*b"CCC", b'P'),
        (*b"CCA", b'P'),
        (*b"CCG", b'P'),
        (*b"CAT", b'H'),
        (*b"CAC", b'H'),
        (*b"CAA", b'Q'),
        (*b"CAG", b'Q'),
        (*b"CGT", b'R'),
        (*b"CGC", b'R'),
        (*b"CGA", b'R'),
        (*b"CGG", b'R'),
        (*b"ATT", b'I'),
        (*b"ATC", b'I'),
        (*b"ATA", b'I'),
        (*b"ATG", b'M'),
        (*b"ACT", b'T'),
        (*b"ACC", b'T'),
        (*b"ACA", b'T'),
        (*b"ACG", b'T'),
        (*b"AAT", b'N'),
        (*b"AAC", b'N'),
        (*b"AAA", b'K'),
        (*b"AAG", b'K'),
        (*b"AGT", b'S'),
        (*b"AGC", b'S'),
        (*b"AGA", b'R'),
        (*b"AGG", b'R'),
        (*b"GTT", b'V'),
        (*b"GTC", b'V'),
        (*b"GTA", b'V'),
        (*b"GTG", b'V'),
        (*b"GCT", b'A'),
        (*b"GCC", b'A'),
        (*b"GCA", b'A'),
        (*b"GCG", b'A'),
        (*b"GAT", b'D'),
        (*b"GAC", b'D'),
        (*b"GAA", b'E'),
        (*b"GAG", b'E'),
        (*b"GGT", b'G'),
        (*b"GGC", b'G'),
        (*b"GGA", b'G'),
        (*b"GGG", b'G'),
    ];

    fn reverse_complement(codon: [u8; 3]) -> [u8; 3] {
        let mut complement = codon.map(|base| match base {
            b'A' => b'T',
            b'T' => b'A',
            b'C' => b'G',
            b'G' => b'C',
            _ => unreachable!("table holds only ACGT"),
        });
        complement.reverse();
        complement
    }

    #[test]
    fn translate_codon_matches_reference_table() {
        for (codon, amino_acid) in STANDARD_TABLE {
            assert_eq!(translate_codon(codon, Strand::Forward), amino_acid);
        }
    }

    #[test]
    fn translate_codon_reverse_matches_reference_table() {
        // this test should never fail if it does you have done something bad
        for (codon, amino_acid) in STANDARD_TABLE {
            assert_eq!(
                translate_codon(reverse_complement(codon), Strand::Reverse),
                amino_acid
            );
        }
    }

    #[rstest]
    #[case::unknown_base(*b"ATN")]
    #[case::gap(*b"A-G")]
    fn translate_codon_is_unknown(
        #[case] codon: [u8; 3],
        #[values(Strand::Forward, Strand::Reverse)] strand: Strand,
    ) {
        assert_eq!(translate_codon(codon, strand), b'X');
    }

    #[rstest]
    #[case::frame1_codon_start(ReadingFrame::Frame1, 0, 0)]
    #[case::frame1_codon_end(ReadingFrame::Frame1, 2, 0)]
    #[case::frame1_second_codon(ReadingFrame::Frame1, 3, 1)]
    #[case::frame2_offset(ReadingFrame::Frame2, 1, 0)]
    #[case::frame2_codon_end(ReadingFrame::Frame2, 3, 0)]
    #[case::frame2_second_codon(ReadingFrame::Frame2, 4, 1)]
    #[case::frame3_offset(ReadingFrame::Frame3, 2, 0)]
    #[case::frame3_codon_end(ReadingFrame::Frame3, 4, 0)]
    #[case::frame3_second_codon(ReadingFrame::Frame3, 5, 1)]
    fn protein_col_works(#[case] frame: ReadingFrame, #[case] nt: usize, #[case] expected: usize) {
        assert_eq!(frame.protein_col(nt), Some(expected));
    }

    #[rstest]
    #[case::frame2_first_nt(ReadingFrame::Frame2, 0)]
    #[case::frame3_first_nt(ReadingFrame::Frame3, 0)]
    #[case::frame3_second_nt(ReadingFrame::Frame3, 1)]
    fn protein_col_is_none(#[case] frame: ReadingFrame, #[case] nt: usize) {
        assert_eq!(frame.protein_col(nt), None);
    }

    #[rstest]
    #[case::frame1_first(ReadingFrame::Frame1, 0, 0..3)]
    #[case::frame1_third(ReadingFrame::Frame1, 2, 6..9)]
    #[case::frame2_first(ReadingFrame::Frame2, 0, 1..4)]
    #[case::frame2_second(ReadingFrame::Frame2, 1, 4..7)]
    #[case::frame3_first(ReadingFrame::Frame3, 0, 2..5)]
    #[case::frame3_second(ReadingFrame::Frame3, 1, 5..8)]
    #[case::frame1_reverse_first(ReadingFrame::Frame1Reverse, 0, 0..3)]
    #[case::frame2_reverse_first(ReadingFrame::Frame2Reverse, 0, 1..4)]
    #[case::frame3_reverse_second(ReadingFrame::Frame3Reverse, 1, 5..8)]
    fn nt_range_works(
        #[case] frame: ReadingFrame,
        #[case] protein_col: usize,
        #[case] expected: Range<usize>,
    ) {
        assert_eq!(frame.nt_range(protein_col), expected);
    }

    #[rstest]
    fn nt_range_agrees_with_protein_range(
        #[values(ReadingFrame::Frame1, ReadingFrame::Frame2, ReadingFrame::Frame3)]
        frame: ReadingFrame,
    ) {
        let nt_width = 12 + frame.offset();
        for protein_col in 0..4 {
            let codon = frame.nt_range(protein_col);
            for nt in codon.clone() {
                assert_eq!(frame.protein_col(nt), Some(protein_col));
            }
            assert_eq!(
                frame.protein_range(codon, nt_width),
                protein_col..protein_col + 1
            );
        }
    }

    #[rstest]
    fn translated_length_agrees_with_nt_range(
        #[values(ReadingFrame::Frame1, ReadingFrame::Frame2, ReadingFrame::Frame3)]
        frame: ReadingFrame,
    ) {
        for length in 0..8 {
            let codons = (0..=length)
                .take_while(|&protein_col| frame.nt_range(protein_col).start < length)
                .count();
            assert_eq!(frame.translated_length(length), codons);
        }
    }

    #[rstest]
    #[case::partial_codon(ReadingFrame::Frame1, 4, 2)]
    #[case::partial_codon_after_offset(ReadingFrame::Frame2, 2, 1)]
    #[case::full_codons(ReadingFrame::Frame3, 8, 2)]
    fn translated_length_works(
        #[case] frame: ReadingFrame,
        #[case] nucleotide_length: usize,
        #[case] expected: usize,
    ) {
        assert_eq!(frame.translated_length(nucleotide_length), expected);
    }

    #[rstest]
    #[case::empty(ReadingFrame::Frame1, 0)]
    #[case::within_offset(ReadingFrame::Frame3, 2)]
    fn translated_length_is_zero(#[case] frame: ReadingFrame, #[case] nucleotide_length: usize) {
        assert_eq!(frame.translated_length(nucleotide_length), 0);
    }

    #[rstest]
    #[case::first_codon(ReadingFrame::Frame3, 1..3, 9, 0..1)]
    #[case::codon_boundary(ReadingFrame::Frame1, 2..4, 9, 0..2)]
    #[case::partial_codon(ReadingFrame::Frame1, 9..10, 10, 3..4)]
    #[case::clipped_to_width(ReadingFrame::Frame1, 6..20, 9, 2..3)]
    fn protein_range_works(
        #[case] frame: ReadingFrame,
        #[case] nt: Range<usize>,
        #[case] nt_width: usize,
        #[case] expected: Range<usize>,
    ) {
        assert_eq!(frame.protein_range(nt, nt_width), expected);
    }

    #[rstest]
    #[case::before_offset(ReadingFrame::Frame3, 0..2, 9)]
    #[case::single_nt_before_offset(ReadingFrame::Frame2, 0..1, 9)]
    #[case::past_width(ReadingFrame::Frame1, 12..15, 9)]
    fn protein_range_is_empty(
        #[case] frame: ReadingFrame,
        #[case] nt: Range<usize>,
        #[case] nt_width: usize,
    ) {
        assert!(frame.protein_range(nt, nt_width).is_empty());
    }

    #[rstest]
    #[case::first_codon(b"ATGCCC", 0..3, b'M')]
    #[case::codon_ending_at_sequence_end(b"ATGCCC", 3..6, b'P')]
    fn codon_at_works(#[case] sequence: &[u8], #[case] codon: Range<usize>, #[case] expected: u8) {
        assert_eq!(codon_at(sequence, codon, Strand::Forward), expected);
    }

    #[rstest]
    #[case::one_nt_short(b"ATGCC", 3..6)]
    #[case::two_nt_short(b"ATGC", 3..6)]
    fn codon_at_is_unknown(#[case] sequence: &[u8], #[case] codon: Range<usize>) {
        assert_eq!(codon_at(sequence, codon, Strand::Forward), b'X');
    }
}
