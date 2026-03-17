use std::ops::Range;

use crate::Alignment;
use crate::alignment_type::AlignmentType;
use crate::data::{AlignmentData, RawSequence};
use crate::error::AlignmentError;
use crate::metrics::{ConsensusMethod, consensus_from_columns, counted_translated_columns_range};

/// Reading frames for translating.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
        ],
    };

    /// Builds a translation table from a lookup matrix.
    ///
    /// The matrix is stored as `[first][second][third]` in `A, T, C, G` order on
    /// each axis. Each entry is the amino-acid byte returned for the translated
    /// codon.
    pub const fn new(codons: [[[u8; 4]; 4]; 4]) -> Self {
        Self { codons }
    }

    pub(crate) fn translate_codon(&self, codon: [u8; 3]) -> u8 {
        let Some(first) = index_nucleotide(codon[0]) else {
            return b'X';
        };
        let Some(second) = index_nucleotide(codon[1]) else {
            return b'X';
        };
        let Some(third) = index_nucleotide(codon[2]) else {
            return b'X';
        };

        self.codons[first][second][third]
    }
}

/// Translated view over an alignment.
///
/// The source alignment must be translation-capable, be a full column
/// projection, and have at least one visible row
#[derive(Debug, Clone, Copy)]
pub struct TranslatedAlignment<'a> {
    source: &'a Alignment,
    frame: ReadingFrame,
    table: TranslationTable,
    translated_column_count: usize,
}

impl<'a> TranslatedAlignment<'a> {
    /// Returns the translated view for one visible row by absolute row id.
    ///
    /// The row id is resolved against the source alignment's current row
    /// projection, not against the translated view itself. Returns `None` when
    /// the row is not visible in the source alignment.
    pub fn sequence_by_absolute(&self, absolute_row: usize) -> Option<TranslatedSequenceView<'a>> {
        let _relative = self.source.relative_row_id(absolute_row)?;
        let sequence = self.source.data.sequences.get(absolute_row)?;
        Some(TranslatedSequenceView {
            data: sequence.sequence(),
            frame: self.frame,
            table: self.table,
            translated_len: self.translated_column_count,
        })
    }

    /// Returns a [`TranslatedSequenceView`] for the absolute row but projected
    /// through this alignment's current column projection.
    ///
    ///
    /// Unlike [`sequence_by_absolute`], this method does not require `abs_row`
    /// to be visible in the current row projection.
    ///
    /// Returns `None` only when `abs_row` is out of bounds for the underlying
    /// alignment data.
    pub fn project_absolute_row(&self, abs_row: usize) -> Option<TranslatedSequenceView<'a>> {
        let sequence = self.source.data.sequences.get(abs_row)?;
        Some(TranslatedSequenceView {
            data: sequence.sequence(),
            frame: self.frame,
            table: self.table,
            translated_len: self.translated_column_count,
        })
    }

    /// Builds a full protein alignment from this translated view.
    ///
    /// The new alignment contains one translated sequence for each visible row in
    /// the source alignment. It uses the reading frame and translation table stored
    /// in this view, and it preserves the current row projection by translating
    /// only visible rows.
    ///
    /// # Errors
    ///
    /// Returns any [`AlignmentError`] encountered while materialising the
    /// translated rows into a concrete alignment.
    pub fn to_alignment(&self) -> Result<Alignment, AlignmentError> {
        let translated = self
            .source
            .absolute_row_ids()
            .map(|absolute_row| {
                let sequence = self.source.data.sequences.get(absolute_row).ok_or(
                    AlignmentError::RowOutOfBounds {
                        index: absolute_row,
                        row_count: self.source.data.sequences.len(),
                    },
                )?;
                Ok(RawSequence {
                    id: sequence.id().to_string(),
                    sequence: translate_sequence(sequence.sequence(), self.frame, &self.table),
                })
            })
            .collect::<Result<Vec<_>, _>>()?;

        let data = AlignmentData::from_raw(translated)?;
        Ok(Alignment::from_typed_data(data, AlignmentType::Protein))
    }

    /// Returns the translated consensus byte for each protein column in `range`.
    ///
    /// The returned positions use protein-column coordinates from the translated
    /// view. Consensus is calculated from the visible rows in the source alignment
    /// with this view's reading frame and translation table.
    ///
    /// # Errors
    ///
    /// [`AlignmentError::EmptyRange`] if `range` is empty.
    ///
    /// [`AlignmentError::ColumnOutOfBounds`] if `range.end` is greater than
    /// the translated width of this view.
    pub fn consensus_range(
        &self,
        range: Range<usize>,
        method: ConsensusMethod,
    ) -> Result<Vec<(usize, Option<u8>)>, AlignmentError> {
        let columns = counted_translated_columns_range(
            &self.source.data,
            &self.source.rows,
            range,
            self.frame,
            &self.table,
        )?;
        let mut rng = rand::rng();
        Ok(consensus_from_columns(&columns, method, &mut rng))
    }
}

impl<'a> TranslatedAlignment<'a> {
    pub(crate) fn new(
        source: &'a Alignment,
        frame: ReadingFrame,
        table: TranslationTable,
    ) -> Result<Self, AlignmentError> {
        if !source.active_type().supports_translation() {
            return Err(AlignmentError::UnsupportedOperation {
                operation: "translate",
                kind: source.active_type(),
            });
        }

        if !source.columns.is_full() {
            return Err(AlignmentError::UnsupportedOperation {
                operation: "translate (columns are filtered)",
                kind: source.active_type(),
            });
        }

        if source.row_count() == 0 {
            return Err(AlignmentError::EmptyRowSubset);
        }

        let translated_column_count = translated_length(source.columns.len(), frame);
        if translated_column_count == 0 {
            return Err(AlignmentError::TranslationEmpty {
                frame,
                length: source.columns.len(),
            });
        }

        Ok(Self {
            source,
            frame,
            table,
            translated_column_count,
        })
    }
}

/// Translated view over one sequence row.
#[derive(Debug, Clone, Copy)]
pub struct TranslatedSequenceView<'a> {
    data: &'a [u8],
    frame: ReadingFrame,
    table: TranslationTable,
    translated_len: usize,
}

impl<'a> TranslatedSequenceView<'a> {
    /// Returns the translated byte at `protein_col`.
    ///
    /// The column index is a protein-column coordinate in this translated sequence.
    /// Returns `None` when `protein_col` is outside the translated length.
    pub fn byte_at(&self, protein_col: usize) -> Option<u8> {
        translated_byte_at(self.data, protein_col, self.frame, &self.table)
    }

    /// Returns translated bytes for a range of protein columns.
    ///
    /// Each item in the iterator contains the protein-column position and the
    /// translated byte at that position. The range is resolved against this
    /// translated sequence's protein coordinates.
    ///
    /// # Errors
    ///
    /// [`AlignmentError::EmptyRange`] if `range` is empty.
    ///
    /// [`AlignmentError::ColumnOutOfBounds`] if `range.end` is greater than
    /// the translated length of this sequence.
    pub fn bytes_range(
        &self,
        range: Range<usize>,
    ) -> Result<impl Iterator<Item = (usize, u8)> + '_, AlignmentError> {
        if range.is_empty() {
            return Err(AlignmentError::EmptyRange);
        }

        if range.end > self.translated_len {
            return Err(AlignmentError::ColumnOutOfBounds {
                index: range.end - 1,
                length: self.translated_len,
            });
        }

        let bytes = range
            .map(|protein_col| {
                let byte = translated_byte_at(self.data, protein_col, self.frame, &self.table)
                    .ok_or(AlignmentError::ColumnOutOfBounds {
                        index: protein_col,
                        length: self.translated_len,
                    })?;
                Ok((protein_col, byte))
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(bytes.into_iter())
    }
}

pub(crate) fn normalise_nucleotide(byte: u8) -> Option<u8> {
    let byte = byte.to_ascii_uppercase();

    match byte {
        b'A' | b'C' | b'G' | b'T' => Some(byte),
        b'U' => Some(b'T'),
        _ => None,
    }
}

pub(crate) fn translate_sequence(
    sequence: &[u8],
    frame: ReadingFrame,
    table: &TranslationTable,
) -> Vec<u8> {
    let mut translated = Vec::with_capacity(translated_length(sequence.len(), frame));

    for codon_start in (frame.offset()..sequence.len()).step_by(3) {
        let codon = [
            sequence
                .get(codon_start)
                .and_then(|&byte| normalise_nucleotide(byte)),
            sequence
                .get(codon_start + 1)
                .and_then(|&byte| normalise_nucleotide(byte)),
            sequence
                .get(codon_start + 2)
                .and_then(|&byte| normalise_nucleotide(byte)),
        ];

        let amino_acid = match codon {
            [Some(first), Some(second), Some(third)] => {
                table.translate_codon([first, second, third])
            }
            _ => b'X',
        };

        translated.push(amino_acid);
    }

    translated
}

pub(crate) fn translated_byte_at(
    sequence: &[u8],
    protein_col: usize,
    frame: ReadingFrame,
    table: &TranslationTable,
) -> Option<u8> {
    let codon_start = frame.offset().checked_add(protein_col.checked_mul(3)?)?;
    let first_raw = *sequence.get(codon_start)?;
    let codon = [
        normalise_nucleotide(first_raw),
        sequence
            .get(codon_start + 1)
            .and_then(|&byte| normalise_nucleotide(byte)),
        sequence
            .get(codon_start + 2)
            .and_then(|&byte| normalise_nucleotide(byte)),
    ];

    match codon {
        [Some(first), Some(second), Some(third)] => {
            Some(table.translate_codon([first, second, third]))
        }
        _ => Some(b'X'),
    }
}

pub(crate) fn translated_length(sequence_len: usize, frame: ReadingFrame) -> usize {
    frame.translated_length(sequence_len)
}

fn index_nucleotide(base: u8) -> Option<usize> {
    match base {
        b'A' => Some(0),
        b'T' => Some(1),
        b'C' => Some(2),
        b'G' => Some(3),
        _ => None,
    }
}

#[cfg(test)]
mod translation_table_tests {
    use super::{
        ReadingFrame, TranslationTable, normalise_nucleotide, translate_sequence,
        translated_byte_at,
    };

    #[test]
    fn normalises_u_to_t() {
        assert_eq!(normalise_nucleotide(b'U'), Some(b'T'));
        assert_eq!(normalise_nucleotide(b'u'), Some(b'T'));
    }

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
    fn translated_sequence_includes_incomplete_terminal_codon() {
        let translated =
            translate_sequence(b"ATGA", ReadingFrame::Frame1, &TranslationTable::STANDARD);
        assert_eq!(translated, b"MX");
    }

    #[test]
    fn translated_frame_works() {
        let translate_frame1 =
            translate_sequence(b"GTCATT", ReadingFrame::Frame1, &TranslationTable::STANDARD);
        let translate_frame2 = translate_sequence(
            b"GGAATTG",
            ReadingFrame::Frame2,
            &TranslationTable::STANDARD,
        );
        let translate_frame3 = translate_sequence(
            b"GGGATTTA",
            ReadingFrame::Frame3,
            &TranslationTable::STANDARD,
        );
        assert_eq!(translate_frame1, b"VI");
        assert_eq!(translate_frame2, b"EL");
        assert_eq!(translate_frame3, b"DL");
    }

    #[test]
    fn translated_byte_at_returns_x_for_all_gap() {
        assert_eq!(
            translated_byte_at(b"---", 0, ReadingFrame::Frame1, &TranslationTable::STANDARD),
            Some(b'X')
        );
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
    use super::{ReadingFrame, translated_length};

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

    #[test]
    fn translated_length_matches_helper_for_edge_cases() {
        for frame in [
            ReadingFrame::Frame1,
            ReadingFrame::Frame2,
            ReadingFrame::Frame3,
        ] {
            for length in 0..8 {
                assert_eq!(
                    frame.translated_length(length),
                    translated_length(length, frame)
                );
            }
        }

        assert_eq!(ReadingFrame::Frame1.translated_length(4), 2);
        assert_eq!(ReadingFrame::Frame2.translated_length(2), 1);
        assert_eq!(ReadingFrame::Frame3.translated_length(2), 0);
    }
}

#[cfg(test)]
mod translated_alignment_tests {
    use super::{ReadingFrame, TranslatedSequenceView, TranslationTable};
    use crate::{Alignment, AlignmentError, AlignmentType, ConsensusMethod, RawSequence};

    fn raw(id: &str, sequence: &[u8]) -> RawSequence {
        RawSequence {
            id: id.to_string(),
            sequence: sequence.to_vec(),
        }
    }

    fn translated_bytes(view: TranslatedSequenceView<'_>, len: usize) -> Vec<(usize, u8)> {
        view.bytes_range(0..len).unwrap().collect()
    }

    #[test]
    fn translated_sequence_byte_at_respects_frames() {
        let alignment =
            Alignment::new_with_type(vec![raw("s1", b"ATGCCCTAA")], AlignmentType::Dna).unwrap();

        let frame1 = alignment.translated(ReadingFrame::Frame1).unwrap();
        let frame2 = alignment.translated(ReadingFrame::Frame2).unwrap();
        let frame3 = alignment.translated(ReadingFrame::Frame3).unwrap();

        let frame1_sequence = frame1.sequence_by_absolute(0).unwrap();
        let frame2_sequence = frame2.sequence_by_absolute(0).unwrap();
        let frame3_sequence = frame3.sequence_by_absolute(0).unwrap();

        assert_eq!(frame1_sequence.byte_at(0), Some(b'M'));
        assert_eq!(frame1_sequence.byte_at(1), Some(b'P'));
        assert_eq!(frame1_sequence.byte_at(2), Some(b'*'));
        assert_eq!(frame2_sequence.byte_at(0), Some(b'C'));
        assert_eq!(frame2_sequence.byte_at(1), Some(b'P'));
        assert_eq!(frame2_sequence.byte_at(2), Some(b'X'));
        assert_eq!(frame3_sequence.byte_at(0), Some(b'A'));
        assert_eq!(frame3_sequence.byte_at(1), Some(b'L'));
        assert_eq!(frame3_sequence.byte_at(2), Some(b'X'));
        assert_eq!(frame3_sequence.byte_at(3), None);
    }

    #[test]
    fn translated_sequence_by_absolute_returns_visible_row() {
        let alignment = Alignment::new_with_type(
            vec![
                raw("s1", b"ATGAAA"),
                raw("s2", b"TTTCCC"),
                raw("s3", b"GGGAAA"),
            ],
            AlignmentType::Dna,
        )
        .unwrap();
        let filtered = alignment
            .filter()
            .unwrap()
            .without_rows([1])
            .apply()
            .unwrap();
        let translated = filtered.translated(ReadingFrame::Frame1).unwrap();

        let sequence = translated.sequence_by_absolute(2).unwrap();
        assert_eq!(sequence.byte_at(0), Some(b'G'));
        assert!(translated.sequence_by_absolute(1).is_none());
    }

    #[test]
    fn translated_alignment_builds_protein_alignment() {
        let alignment = Alignment::new_with_type(
            vec![raw("s1", b"ATGAAA"), raw("s2", b"ATGAAG")],
            AlignmentType::Dna,
        )
        .unwrap();
        let translated = alignment.translated(ReadingFrame::Frame1).unwrap();
        let materialised = translated.to_alignment().unwrap();
        let first = materialised.sequence(0).unwrap();
        let second = materialised.sequence(1).unwrap();

        assert_eq!(materialised.active_type(), AlignmentType::Protein);
        assert_eq!(materialised.row_count(), 2);
        assert_eq!(materialised.column_count(), 2);
        assert_eq!(first.id(), "s1");
        assert_eq!(second.id(), "s2");
        assert_eq!(
            first
                .indexed_bytes_range(0..first.len())
                .unwrap()
                .map(|(_, byte)| byte)
                .collect::<Vec<_>>(),
            b"MK"
        );
        assert_eq!(
            second
                .indexed_bytes_range(0..second.len())
                .unwrap()
                .map(|(_, byte)| byte)
                .collect::<Vec<_>>(),
            b"MK"
        );
    }

    #[test]
    fn translated_alignment_rejects_protein_source() {
        let alignment =
            Alignment::new_with_type(vec![raw("s1", b"MKM")], AlignmentType::Protein).unwrap();

        assert!(matches!(
            alignment.translated(ReadingFrame::Frame1),
            Err(AlignmentError::UnsupportedOperation {
                operation: "translate",
                kind: AlignmentType::Protein,
            })
        ));
    }

    #[test]
    fn translated_alignment_rejects_filtered_columns() {
        let alignment = Alignment::new_with_type(
            vec![raw("s1", b"ATG---"), raw("s2", b"ATG---")],
            AlignmentType::Dna,
        )
        .unwrap();
        let filtered = alignment
            .filter()
            .unwrap()
            .with_max_gap_fraction(0.0)
            .apply()
            .unwrap();

        assert!(matches!(
            filtered.translated(ReadingFrame::Frame1),
            Err(AlignmentError::UnsupportedOperation {
                operation: "translate (columns are filtered)",
                kind: AlignmentType::Dna,
            })
        ));
    }

    #[test]
    fn translated_alignment_accepts_filtered_rows() {
        let alignment = Alignment::new_with_type(
            vec![
                raw("s1", b"ATGAAA"),
                raw("s2", b"TTTCCC"),
                raw("s3", b"GGGAAA"),
            ],
            AlignmentType::Dna,
        )
        .unwrap();
        let filtered = alignment
            .filter()
            .unwrap()
            .without_rows([0, 2])
            .apply()
            .unwrap();
        let translated = filtered.translated(ReadingFrame::Frame1).unwrap();
        let materialised = translated.to_alignment().unwrap();

        assert!(translated.sequence_by_absolute(1).is_some());
        assert!(translated.sequence_by_absolute(0).is_none());
        assert!(translated.sequence_by_absolute(2).is_none());
        assert_eq!(
            materialised
                .sequence(0)
                .unwrap()
                .indexed_bytes_range(0..materialised.sequence(0).unwrap().len())
                .unwrap()
                .map(|(_, byte)| byte)
                .collect::<Vec<_>>(),
            b"FP"
        );
    }

    #[test]
    fn translated_alignment_rejects_empty_row_projection() {
        let alignment =
            Alignment::new_with_type(vec![raw("s1", b"ATGAAA")], AlignmentType::Dna).unwrap();
        let filtered = alignment
            .filter()
            .unwrap()
            .without_rows([0])
            .apply()
            .unwrap();

        assert!(matches!(
            filtered.translated(ReadingFrame::Frame1),
            Err(AlignmentError::EmptyRowSubset)
        ));
    }

    #[test]
    fn translated_alignment_rejects_empty_translation() {
        let alignment =
            Alignment::new_with_type(vec![raw("s1", b"AT")], AlignmentType::Dna).unwrap();

        assert!(matches!(
            alignment.translated(ReadingFrame::Frame3),
            Err(AlignmentError::TranslationEmpty {
                frame: ReadingFrame::Frame3,
                length: 2,
            })
        ));
    }

    #[test]
    fn translated_alignment_custom_table_is_used_for_consensus() {
        let alignment = Alignment::new_with_type(
            vec![
                raw("s1", b"ATGAAA"),
                raw("s2", b"ATGAAA"),
                raw("s3", b"ATGAAA"),
            ],
            AlignmentType::Dna,
        )
        .unwrap();
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

        let translated = alignment
            .translated_with(ReadingFrame::Frame1, custom)
            .unwrap();

        assert_eq!(
            translated
                .consensus_range(0..1, ConsensusMethod::MajorityNonGap)
                .unwrap(),
            vec![(0, Some(b'Z'))]
        );
    }

    #[test]
    fn project_absolute_row_bypasses_row_filter() {
        let alignment = Alignment::new_with_type(
            vec![
                raw("s1", b"ATGAAA"),
                raw("s2", b"TTTCCC"),
                raw("s3", b"GGGAAA"),
            ],
            AlignmentType::Dna,
        )
        .unwrap();
        let filtered = alignment
            .filter()
            .unwrap()
            .without_rows([1])
            .apply()
            .unwrap();
        let translated = filtered.translated(ReadingFrame::Frame1).unwrap();

        assert!(translated.sequence_by_absolute(1).is_none());
        assert_eq!(
            translated_bytes(translated.project_absolute_row(1).unwrap(), 2),
            vec![(0, b'F'), (1, b'P')]
        );
    }

    #[test]
    fn project_absolute_row_matches() {
        let alignment = Alignment::new_with_type(
            vec![raw("s1", b"ATGAAA"), raw("s2", b"TTTCCC")],
            AlignmentType::Dna,
        )
        .unwrap();
        let translated = alignment.translated(ReadingFrame::Frame1).unwrap();

        for abs_row in 0..2 {
            assert_eq!(
                translated_bytes(translated.sequence_by_absolute(abs_row).unwrap(), 2),
                translated_bytes(translated.project_absolute_row(abs_row).unwrap(), 2)
            );
        }
    }

    #[test]
    fn project_absolute_row_out_of_bounds() {
        let alignment =
            Alignment::new_with_type(vec![raw("s1", b"ATGAAA")], AlignmentType::Dna).unwrap();
        let translated = alignment.translated(ReadingFrame::Frame1).unwrap();

        assert!(translated.project_absolute_row(1).is_none());
        assert!(translated.project_absolute_row(usize::MAX).is_none());
    }
}
