use crate::error::AlignmentError;

/// Stores a raw sequence, before it has been validated into a [`Sequence`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawSequence {
    pub id: String,
    pub sequence: Vec<u8>,
}

/// Represents a validated sequence in an alignment.
///
/// A `Sequence` will always have a non-empty sequence of bytes,
/// and all sequences in an alignment will have the same length
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sequence {
    id: String,
    sequence: Box<[u8]>,
}

impl Sequence {
    /// Returns the identifier (FASTA header).
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Returns sequence in bytes.
    pub fn sequence(&self) -> &[u8] {
        &self.sequence
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AlignmentData {
    pub(crate) sequences: Vec<Sequence>,
    pub(crate) length: usize,
}

impl AlignmentData {
    pub(crate) fn from_raw(sequences: Vec<RawSequence>) -> Result<Self, AlignmentError> {
        let mut raw_iter = sequences.into_iter();
        let Some(first) = raw_iter.next() else {
            return Err(AlignmentError::Empty);
        };

        if first.sequence.is_empty() {
            return Err(AlignmentError::EmptySequence { id: first.id });
        }

        let width = first.sequence.len();
        let first = Sequence {
            id: first.id,
            sequence: first.sequence.into_boxed_slice(),
        };

        let mut normalised = Vec::with_capacity(1 + raw_iter.len());
        normalised.push(first);

        let normalised = raw_iter.try_fold(normalised, |mut normalised, raw| {
            if raw.sequence.is_empty() {
                return Err(AlignmentError::EmptySequence { id: raw.id });
            }

            let actual = raw.sequence.len();
            if actual != width {
                return Err(AlignmentError::LengthMismatch {
                    expected: width,
                    actual,
                    id: raw.id,
                });
            }

            normalised.push(Sequence {
                id: raw.id,
                sequence: raw.sequence.into_boxed_slice(),
            });
            Ok(normalised)
        })?;

        Ok(Self {
            sequences: normalised,
            length: width,
        })
    }
}
