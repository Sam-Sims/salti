use rayon::iter::Either;

use crate::{Alignment, AlignmentType, ReadingFrame, translation::codon_at};

/// Reads the cells of an [`Alignment`], either as stored or translated in a reading frame
///
/// Row ids are rows of the alignment. Column ids are columns of the grid
#[derive(Debug, Clone, Copy)]
pub struct Grid<'alignment> {
    alignment: &'alignment Alignment,
    alignment_type: AlignmentType,
    // when frame is Some we are protein alignment
    frame: Option<ReadingFrame>,
}

impl Grid<'_> {
    /// Returns the number of columns
    ///
    /// When translated, a trailing partial codon counts as one column and reads as `X`
    pub fn width(self) -> usize {
        let width = self.alignment.width();
        self.frame
            .map_or(width, |frame| frame.translated_length(width))
    }

    /// Returns the type of the cells, which is protein when translated
    pub fn alignment_type(self) -> AlignmentType {
        self.alignment_type
    }

    /// Returns the cells of `row` at each id in `cols`, in `cols` order
    ///
    /// Ids may be in any order and may repeat
    ///
    /// # Panics
    ///
    /// If `row` is not below the row count, or an id in `cols` is not below [`width`](Self::width)
    pub fn cells(self, row: usize, cols: &[usize]) -> impl Iterator<Item = u8> {
        let residues = &self.alignment.row(row).residues;
        match self.frame {
            None => Either::Left(
                cols.iter()
                    .map(move |&col| *residues.get(col).expect("col id should be below the width")),
            ),
            Some(frame) => Either::Right(
                cols.iter()
                    .map(move |&protein_col| codon_at(residues, frame.nt_range(protein_col))),
            ),
        }
    }

    /// Returns the cell at `row` and `col`
    ///
    /// # Panics
    ///
    /// If `row` is not below the row count, or `col` is not below [`width`](Self::width)
    pub fn cell(self, row: usize, col: usize) -> u8 {
        self.cells(row, &[col])
            .next()
            .expect("cells should yield one byte per id")
    }
}

impl Alignment {
    /// Returns a grid that reads cells as stored, as `alignment_type`
    pub fn grid(&self, alignment_type: AlignmentType) -> Grid<'_> {
        Grid {
            alignment: self,
            alignment_type,
            frame: None,
        }
    }

    /// Returns a grid that translates each codon in `frame` into one protein cell
    ///
    /// Doesn't check the alignment type. Callers need to check
    /// [`AlignmentType::supports_translation`] first
    pub fn translated_grid(&self, frame: ReadingFrame) -> Grid<'_> {
        Grid {
            alignment: self,
            alignment_type: AlignmentType::Protein,
            frame: Some(frame),
        }
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::Grid;
    use crate::{Alignment, AlignmentType, ReadingFrame, Sequence};

    fn dna(sequences: &[&[u8]]) -> Alignment {
        Alignment::new(
            sequences
                .iter()
                .enumerate()
                .map(|(i, sequence)| Sequence {
                    id: format!("s{i}"),
                    residues: sequence.to_vec(),
                })
                .collect(),
        )
        .unwrap()
    }

    fn grid(alignment: &Alignment, frame: Option<ReadingFrame>) -> Grid<'_> {
        frame.map_or(alignment.grid(AlignmentType::Dna), |frame| {
            alignment.translated_grid(frame)
        })
    }

    #[rstest]
    #[case::raw(None, &[0, 1, 2], b"ATG")]
    #[case::raw_unsorted_repeated(None, &[3, 0, 3], b"CAC")]
    #[case::translated(Some(ReadingFrame::Frame1), &[0, 1, 2], b"MP*")]
    #[case::translated_unsorted_repeated(Some(ReadingFrame::Frame1), &[2, 0, 0], b"*MM")]
    fn cells_works(
        #[case] frame: Option<ReadingFrame>,
        #[case] cols: &[usize],
        #[case] expected: &[u8],
    ) {
        let alignment = dna(&[b"ATGCCCTAA"]);
        let cells: Vec<u8> = grid(&alignment, frame).cells(0, cols).collect();
        assert_eq!(cells, expected);
    }

    #[rstest]
    fn cells_is_empty(#[values(None, Some(ReadingFrame::Frame1))] frame: Option<ReadingFrame>) {
        let alignment = dna(&[b"ATGCCCTAA"]);
        assert_eq!(grid(&alignment, frame).cells(0, &[]).count(), 0);
    }

    #[test]
    fn cells_reads_requested_row() {
        let alignment = dna(&[b"ATGAAA", b"TTTCCC"]);
        let grid = alignment.translated_grid(ReadingFrame::Frame1);

        assert_eq!(grid.cells(1, &[0, 1]).collect::<Vec<_>>(), b"FP");
        assert_eq!(grid.cells(0, &[0, 1]).collect::<Vec<_>>(), b"MK");
    }

    #[rstest]
    #[case::raw(None, 9)]
    #[case::translated(Some(ReadingFrame::Frame1), 3)]
    fn width_works(#[case] frame: Option<ReadingFrame>, #[case] expected: usize) {
        let alignment = dna(&[b"ATGCCCTAA"]);
        assert_eq!(grid(&alignment, frame).width(), expected);
    }
}
