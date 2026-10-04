use crate::{
    alignment_type::{AlignmentType, detect_alignment_type},
    error::AlignmentError,
};

/// One row of an [`Alignment`]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sequence {
    pub id: String,
    pub residues: Vec<u8>,
}

/// A multiple sequence alignment
#[derive(Debug)]
pub struct Alignment {
    sequences: Vec<Sequence>,
    width: usize,
    detected_type: AlignmentType,
}

impl Alignment {
    /// Creates an alignment from `sequences` and detects its type
    ///
    /// # Errors
    ///
    /// Checks row by row and returns the first error found:
    ///
    /// - [`AlignmentError::Empty`] if there are no sequences
    /// - [`AlignmentError::EmptySequence`] if the first sequence has no residues
    /// - [`AlignmentError::NonAsciiSequence`] if a sequence has a byte outside ASCII
    /// - [`AlignmentError::LengthMismatch`] if a sequence's length differs from the first
    ///   sequence's, which includes a later empty sequence
    pub fn new(sequences: Vec<Sequence>) -> Result<Self, AlignmentError> {
        let first = sequences.first().ok_or(AlignmentError::Empty)?;
        let width = first.residues.len();
        if width == 0 {
            return Err(AlignmentError::EmptySequence {
                id: first.id.clone(),
            });
        }
        for seq in &sequences {
            if !seq.residues.is_ascii() {
                return Err(AlignmentError::NonAsciiSequence { id: seq.id.clone() });
            }
            if seq.residues.len() != width {
                return Err(AlignmentError::LengthMismatch {
                    expected: width,
                    actual: seq.residues.len(),
                    id: seq.id.clone(),
                });
            }
        }
        let detected_type = detect_alignment_type(&sequences);
        Ok(Self {
            sequences,
            width,
            detected_type,
        })
    }

    /// Returns the number of columns
    pub fn width(&self) -> usize {
        self.width
    }

    /// Returns the identifier of `row`
    ///
    /// # Panics
    ///
    /// If `row` is not below [`row_count`](Self::row_count)
    pub fn id(&self, row: usize) -> &str {
        &self.row(row).id
    }

    /// Returns the number of rows
    pub fn row_count(&self) -> usize {
        self.sequences.len()
    }

    /// Returns the length in characters of the longest identifier
    pub fn max_id_len(&self) -> usize {
        self.sequences
            .iter()
            .map(|seq| seq.id.chars().count())
            .max()
            .expect("new should reject an empty alignment")
    }

    /// Returns the type detected by [`new`](Self::new)
    pub fn detected_type(&self) -> AlignmentType {
        self.detected_type
    }

    pub(crate) fn row(&self, row: usize) -> &Sequence {
        self.sequences
            .get(row)
            .expect("row id should be below the row count")
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    fn seq(id: &str, residues: &[u8]) -> Sequence {
        Sequence {
            id: id.to_string(),
            residues: residues.to_vec(),
        }
    }

    #[rstest]
    #[case::dna(vec![seq("s0", b"ACGT"), seq("s1", b"TGCA")], 4, AlignmentType::Dna)]
    #[case::every_ascii_byte(
        vec![seq("s0", &(0..=0x7f).collect::<Vec<u8>>())],
        128,
        AlignmentType::Generic
    )]
    fn new_works(
        #[case] sequences: Vec<Sequence>,
        #[case] width: usize,
        #[case] detected_type: AlignmentType,
    ) {
        let row_count = sequences.len();
        let alignment = Alignment::new(sequences).unwrap();
        assert_eq!(alignment.width(), width);
        assert_eq!(alignment.row_count(), row_count);
        assert_eq!(alignment.detected_type(), detected_type);
    }

    #[rstest]
    #[case::empty(vec![], AlignmentError::Empty)]
    #[case::empty_sequence(
        vec![seq("s0", b"")],
        AlignmentError::EmptySequence { id: "s0".to_string() }
    )]
    #[case::non_ascii(
        vec![seq("s0", b"ACGT"), seq("s1", &[b'A', b'C', b'G', 0x80])],
        AlignmentError::NonAsciiSequence { id: "s1".to_string() }
    )]
    #[case::length_mismatch(
        vec![seq("s0", b"ACGT"), seq("s1", b"ACG")],
        AlignmentError::LengthMismatch { expected: 4, actual: 3, id: "s1".to_string() }
    )]
    #[case::first_bad_row_wins(
        vec![seq("s0", b"ACGT"), seq("s1", &[b'A', b'C', b'G', 0xff]), seq("s2", b"AC")],
        AlignmentError::NonAsciiSequence { id: "s1".to_string() }
    )]
    fn new_rejects(#[case] sequences: Vec<Sequence>, #[case] expected: AlignmentError) {
        assert_eq!(Alignment::new(sequences).unwrap_err(), expected);
    }

    #[rstest]
    #[case::single(&["seq-1"], 5)]
    #[case::longest_not_first(&["a", "longest", "mid"], 7)]
    #[case::counts_chars_not_bytes(&["αβγ", "ab"], 3)]
    fn max_id_len_works(#[case] ids: &[&str], #[case] expected: usize) {
        let alignment = Alignment::new(ids.iter().map(|id| seq(id, b"A")).collect()).unwrap();
        assert_eq!(alignment.max_id_len(), expected);
    }
}
