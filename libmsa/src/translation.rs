use std::ops::Range;

use crate::residue::{UNKNOWN_AMINO_ACID, nucleotide_index};

/// Reading frames for translating.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReadingFrame {
    Frame1,
    Frame2,
    Frame3,
}

impl ReadingFrame {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Frame1 => "1",
            Self::Frame2 => "2",
            Self::Frame3 => "3",
        }
    }

    pub const fn all() -> [Self; 3] {
        [Self::Frame1, Self::Frame2, Self::Frame3]
    }

    /// Returns the nucleotide offset for this reading frame.
    pub const fn offset(self) -> usize {
        match self {
            Self::Frame1 => 0,
            Self::Frame2 => 1,
            Self::Frame3 => 2,
        }
    }

    /// Returns the nucleotide columns that span the codon given at `protein_col`
    pub const fn nt_range(self, protein_col: usize) -> Range<usize> {
        let start = self.offset() + 3 * protein_col;
        start..start + 3
    }

    /// Returns the protein columns where codons overlap the input `nt` range
    pub fn protein_range(self, nt: Range<usize>, nt_width: usize) -> Range<usize> {
        debug_assert!(!nt.is_empty(), "dont give an empty nt range");
        let len = self.translated_length(nt_width);
        let start = self.protein_col(nt.start).unwrap_or(0).min(len);
        let end = self.protein_col(nt.end - 1).map_or(0, |p| p + 1).min(len);
        start..end
    }

    /// Returns the protein column for an absolute nucleotide column, or `None`
    /// when the column lies before this frame's offset.
    pub const fn protein_col(self, absolute_nuc_col: usize) -> Option<usize> {
        let offset = self.offset();
        if absolute_nuc_col < offset {
            return None;
        }

        Some((absolute_nuc_col - offset) / 3)
    }

    /// Returns the translated protein length for a nucleotide sequence length,
    /// counting incomplete terminal codons that translate to `X`.
    pub const fn translated_length(self, nucleotide_length: usize) -> usize {
        let offset = self.offset();
        if nucleotide_length <= offset {
            return 0;
        }

        ((nucleotide_length - 1 - offset) / 3) + 1
    }
}

impl std::fmt::Display for ReadingFrame {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

impl std::str::FromStr for ReadingFrame {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::all()
            .into_iter()
            .find(|frame| frame.name() == value)
            .ok_or(())
    }
}

/// Translation table for mapping DNA codons to amino-acid bytes.
///
/// The layout is `[first][second][third]`, with nucleotides indexed in `A, T, C, G` order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TranslationTable {
    codons: [[[u8; 4]; 4]; 4],
}

impl TranslationTable {
    /// Standard translation table.
    pub const STANDARD: Self = Self {
        codons: [
            [*b"KNNK", *b"IIIM", *b"TTTT", *b"RSSR"],
            [*b"*YY*", *b"LFFL", *b"SSSS", *b"*CCW"],
            [*b"QHHQ", *b"LLLL", *b"PPPP", *b"RRRR"],
            [*b"EDDE", *b"VVVV", *b"AAAA", *b"GGGG"],
        ],
    };

    /// Builds a translation table from a lookup matrix.
    ///
    /// The matrix is stored as `[first][second][third]` in `A, T, C, G` order on
    /// each axis. Each entry is the amino-acid byte returned for the translated
    /// codon.
    ///
    /// # Panics
    ///
    /// If any amino-acid byte is outside ASCII. Column counting indexes
    /// 128-entry tables by byte value.
    pub const fn new(codons: [[[u8; 4]; 4]; 4]) -> Self {
        assert!(
            codons.as_flattened().as_flattened().is_ascii(),
            "translation table outputs must be ASCII"
        );
        Self { codons }
    }

    pub(crate) fn translate_codon(&self, codon: [u8; 3]) -> u8 {
        match (
            nucleotide_index(codon[0]),
            nucleotide_index(codon[1]),
            nucleotide_index(codon[2]),
        ) {
            (Some(first), Some(second), Some(third)) => self.codons[first][second][third],
            _ => UNKNOWN_AMINO_ACID,
        }
    }
}

/// Returns the amino acid byte for the input `codon` in input `sequence` translated
/// with the standard table.
///
/// A truncated codon at the end will return as the `UNKNOWN_AMINO_ACID` char
pub(crate) fn codon_at(sequence: &[u8], codon: Range<usize>) -> u8 {
    let first = *sequence
        .get(codon.start)
        .expect("col should be below width");
    let (Some(&second), Some(&third)) =
        (sequence.get(codon.start + 1), sequence.get(codon.start + 2))
    else {
        return UNKNOWN_AMINO_ACID;
    };
    TranslationTable::STANDARD.translate_codon([first, second, third])
}

#[cfg(test)]
mod translation_table_tests {
    use super::TranslationTable;

    // https://www.hgmd.cf.ac.uk/docs/cd_amino.html
    #[test]
    fn standard_table_matches_full_reference_table() {
        let expected = [
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

        for (codon, amino_acid) in expected {
            assert_eq!(
                TranslationTable::STANDARD.translate_codon(codon),
                amino_acid
            );
        }
    }

    #[test]
    fn invalid_codon_translates_to_x() {
        assert_eq!(TranslationTable::STANDARD.translate_codon(*b"ATN"), b'X');
        assert_eq!(TranslationTable::STANDARD.translate_codon(*b"A-G"), b'X');
    }

    #[test]
    #[should_panic(expected = "translation table outputs must be ASCII")]
    fn new_rejects_non_ascii_output() {
        let mut codons = [[[b'A'; 4]; 4]; 4];
        codons[3][2][1] = 0x80;
        TranslationTable::new(codons);
    }

    #[test]
    fn custom_translation_table() {
        let mut codons = [
            [
                [b'K', b'N', b'N', b'K'],
                [b'I', b'I', b'I', b'M'],
                [b'T', b'T', b'T', b'T'],
                [b'R', b'S', b'S', b'R'],
            ],
            [
                [b'*', b'Y', b'Y', b'*'],
                [b'L', b'F', b'F', b'L'],
                [b'S', b'S', b'S', b'S'],
                [b'*', b'C', b'C', b'W'],
            ],
            [
                [b'Q', b'H', b'H', b'Q'],
                [b'L', b'L', b'L', b'L'],
                [b'P', b'P', b'P', b'P'],
                [b'R', b'R', b'R', b'R'],
            ],
            [
                [b'E', b'D', b'D', b'E'],
                [b'V', b'V', b'V', b'V'],
                [b'A', b'A', b'A', b'A'],
                [b'G', b'G', b'G', b'G'],
            ],
        ];
        codons[0][1][3] = b'Z';
        let custom = TranslationTable::new(codons);

        assert_eq!(custom.translate_codon(*b"ATG"), b'Z');
        assert_eq!(TranslationTable::STANDARD.translate_codon(*b"ATG"), b'M');
        assert_eq!(custom.translate_codon(*b"TTT"), b'F');
        assert_eq!(custom.translate_codon(*b"GGG"), b'G');
    }
}

#[cfg(test)]
mod reading_frame_tests {
    use super::ReadingFrame;

    #[test]
    fn protein_col_maps_absolute_columns() {
        assert_eq!(ReadingFrame::Frame1.protein_col(0), Some(0));
        assert_eq!(ReadingFrame::Frame1.protein_col(2), Some(0));
        assert_eq!(ReadingFrame::Frame1.protein_col(3), Some(1));

        assert_eq!(ReadingFrame::Frame2.protein_col(0), None);
        assert_eq!(ReadingFrame::Frame2.protein_col(1), Some(0));
        assert_eq!(ReadingFrame::Frame2.protein_col(3), Some(0));
        assert_eq!(ReadingFrame::Frame2.protein_col(4), Some(1));

        assert_eq!(ReadingFrame::Frame3.protein_col(1), None);
        assert_eq!(ReadingFrame::Frame3.protein_col(2), Some(0));
        assert_eq!(ReadingFrame::Frame3.protein_col(4), Some(0));
        assert_eq!(ReadingFrame::Frame3.protein_col(5), Some(1));
    }
}
