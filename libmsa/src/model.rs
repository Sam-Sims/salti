use std::sync::Arc;

use crate::{
    alignment_type::AlignmentType,
    data::{AlignmentData, RawSequence},
    detection::detect_alignment_type,
    error::AlignmentError,
};

/// A multiple sequence alignment.
///
/// `Alignment` stores a set of equal-length sequences together with
/// the current view over that data. The view can expose all rows and columns or
/// a filtered projection, while still preserving absolute row and column
/// coordinates into the underlying alignment.
#[derive(Debug, Clone)]
pub struct Alignment {
    pub(crate) data: Arc<AlignmentData>,
    detected_type: AlignmentType,
}

impl Alignment {
    /// Creates an alignment from raw sequences and detects its kind.
    ///
    /// The returned alignment starts with all rows and columns visible. The detected kind becomes both the
    /// detected kind and the active kind for the new alignment.
    ///
    /// # Errors
    ///
    /// [`AlignmentError::Empty`] if `seqs` is empty.
    ///
    /// [`AlignmentError::EmptySequence`] if any sequence in `seqs` has an empty sequence.
    ///
    /// [`AlignmentError::NonAsciiSequence`] if any sequence in `seqs` contains a byte outside ASCII.
    ///
    /// [`AlignmentError::LengthMismatch`] if the sequences in `seqs` do not all have the same length.
    pub fn new(seqs: impl IntoIterator<Item = RawSequence>) -> Result<Self, AlignmentError> {
        let data = data_from_raw_sequences(seqs)?;
        let detected = detect_alignment_type(&data);
        Ok(Self::from_data(data, detected))
    }

    /// Creates an alignment from raw sequences with an explicit type.
    ///
    /// This constructor skips type detection. The returned alignment starts with all rows and columns
    /// visible, and the supplied `kind` is recorded as both the detected kind and the active kind.
    ///
    /// # Errors
    ///
    /// [`AlignmentError::Empty`] if `seqs` is empty.
    ///
    /// [`AlignmentError::EmptySequence`] if any sequence in `seqs` has an empty sequence.
    ///
    /// [`AlignmentError::NonAsciiSequence`] if any sequence in `seqs` contains a byte outside ASCII.
    ///
    /// [`AlignmentError::LengthMismatch`] if the sequences in `seqs` do not all have the same length.
    #[cfg(test)]
    pub(crate) fn new_with_type(
        seqs: impl IntoIterator<Item = RawSequence>,
        kind: AlignmentType,
    ) -> Result<Self, AlignmentError> {
        let data = data_from_raw_sequences(seqs)?;
        Ok(Self::from_data(data, kind))
    }

    pub(crate) fn from_data(data: AlignmentData, alignment_type: AlignmentType) -> Self {
        Self {
            data: Arc::new(data),
            detected_type: alignment_type,
        }
    }

    /// Returns number of columns
    pub fn width(&self) -> usize {
        self.data.length
    }

    /// Returns the id for the given `row`
    pub fn id(&self, row: usize) -> &str {
        &self
            .data
            .sequences
            .get(row)
            .expect("row id should be below the row count")
            .id
    }

    /// Returns the number of visible sequences.
    ///
    /// This is the length of the alignment's current row projection. For a filtered alignment, it
    /// returns the number of rows that remain visible after filtering.
    pub fn row_count(&self) -> usize {
        todo!()
    }

    /// Returns the length in characters of the longest visible sequence identifier, or `0` if no sequences are visible.
    pub fn max_id_len(&self) -> usize {
        todo!()
    }
}

fn data_from_raw_sequences(
    sequences: impl IntoIterator<Item = RawSequence>,
) -> Result<AlignmentData, AlignmentError> {
    let sequences = sequences
        .into_iter()
        .map(TryInto::try_into)
        .collect::<Result<Vec<_>, _>>()?;
    AlignmentData::new(sequences)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alignment_type::AlignmentType;

    fn raw(id: &str, sequence: &[u8]) -> RawSequence {
        RawSequence {
            id: id.to_string(),
            sequence: sequence.to_vec(),
        }
    }

    #[test]
    fn constructs_valid_alignment() {
        let alignment = Alignment::new(vec![raw("seq-1", b"ACGT"), raw("seq-2", b"TGCA")]).unwrap();
        assert_eq!(alignment.width(), 4);
        assert_eq!(alignment.row_count(), 2);
        assert_eq!(alignment.detected_type(), AlignmentType::Dna);
    }

    #[test]
    fn new_with_kind_skips_detection() {
        let alignment = Alignment::new_with_type(
            vec![raw("seq-1", b"ACGT"), raw("seq-2", b"TGCA")],
            AlignmentType::Protein,
        )
        .unwrap();
        assert_eq!(alignment.detected_type(), AlignmentType::Protein);
    }

    #[test]
    fn rejects_empty_alignment() {
        let result = Alignment::new(vec![]);
        assert!(matches!(result, Err(AlignmentError::Empty)));
    }

    #[test]
    fn rejects_mismatched_lengths() {
        let result = Alignment::new(vec![raw("seq-1", b"ACGT"), raw("seq-2", b"ACG")]);
        assert!(matches!(result, Err(AlignmentError::LengthMismatch { .. })));
    }

    #[test]
    fn rejects_non_ascii_bytes_with_sequence_id() {
        for byte in [0x80, 0xc3, 0xff] {
            let result = Alignment::new(vec![raw("seq-1", b"ACGT"), raw("seq-2", &[b'A', byte])]);
            assert_eq!(
                result.unwrap_err(),
                AlignmentError::NonAsciiSequence {
                    id: "seq-2".to_string()
                }
            );
        }
    }

    #[test]
    fn accepts_every_ascii_byte() {
        let every_ascii: Vec<u8> = (0..=0x7f).collect();
        let alignment =
            Alignment::new(vec![raw("seq-1", &every_ascii), raw("seq-2", &every_ascii)]).unwrap();
        assert_eq!(alignment.width(), 128);
    }
}
