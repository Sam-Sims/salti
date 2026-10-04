use rayon::iter::Either;

use crate::{Alignment, AlignmentType, ReadingFrame, translation::codon_at};

/// A grid representing an alignment
#[derive(Debug, Clone, Copy)]
pub struct Grid<'alignment> {
    alignment: &'alignment Alignment,
    alignment_type: AlignmentType,
    // when frame is Some we are protein alignment
    frame: Option<ReadingFrame>,
}

impl<'alignment> Grid<'alignment> {
    /// Return the number of columns in the grid
    ///
    /// When translated a truncated partial codon  at the end will
    /// still count as X
    pub fn width(self) -> usize {
        let width = self.alignment.width();
        self.frame
            .map_or(width, |frame| frame.translated_length(width))
    }

    /// Returns current alignment type for grid
    pub fn alignment_type(self) -> AlignmentType {
        self.alignment_type
    }

    pub fn cells(self, row: usize, cols: &[usize]) -> impl Iterator<Item = u8> {
        let sequence = &self
            .alignment
            .data
            .sequences
            .get(row)
            .expect("row id should be below the row count, salti only holds rows that exist")
            .sequence;
        match self.frame {
            None => Either::Left(cols.iter().map(move |&col| {
                *sequence
                    .get(col)
                    .expect("col id should be below the width, salti only holds cols that exist")
            })),
            Some(frame) => Either::Right(
                cols.iter()
                    .map(move |&protein_col| codon_at(sequence, frame.codon(protein_col))),
            ),
        }
    }

    /// Returns the cell at `row` and `col`
    ///
    /// A cell is the single byte at a given position
    pub fn cell(self, row: usize, col: usize) -> u8 {
        // shouldnt cant panic as next only gets one and salti will garunatee that
        // we dont access anything out of bounds
        self.cells(row, &[col])
            .next()
            .expect("cells should yeild only 1 byte")
    }
}

impl Alignment {
    /// Returns a grid
    pub fn grid(&self, alignment_type: AlignmentType) -> Grid<'_> {
        Grid {
            alignment: self,
            alignment_type,
            frame: None,
        }
    }

    /// Returns a transalted grid, transalted in `frame`
    pub fn translated_grid(&self, frame: ReadingFrame) -> Grid<'_> {
        Grid {
            alignment: self,
            alignment_type: AlignmentType::Protein,
            frame: Some(frame),
        }
    }
}
