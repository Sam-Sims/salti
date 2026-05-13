use crate::error::AlignmentError;

/// Stores a raw sequence before it has been validated into an internal row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawSequence {
    pub id: String,
    pub sequence: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Sequence {
    pub(crate) id: String,
    pub(crate) sequence: Box<[u8]>,
}

impl TryFrom<RawSequence> for Sequence {
    type Error = AlignmentError;

    fn try_from(raw_sequence: RawSequence) -> Result<Self, Self::Error> {
        if raw_sequence.sequence.is_empty() {
            return Err(AlignmentError::EmptySequence {
                id: raw_sequence.id,
            });
        }

        Ok(Self {
            id: raw_sequence.id,
            sequence: raw_sequence.sequence.into_boxed_slice(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AlignmentData {
    pub(crate) sequences: Vec<Sequence>,
    pub(crate) length: usize,
}

impl AlignmentData {
    pub(crate) fn new(sequences: Vec<Sequence>) -> Result<Self, AlignmentError> {
        let mut sequences = sequences.into_iter();
        let Some(first) = sequences.next() else {
            return Err(AlignmentError::Empty);
        };

        let length = first.sequence.len();
        let mut normalised = Vec::with_capacity(1 + sequences.len());
        normalised.push(first);

        for sequence in sequences {
            let actual = sequence.sequence.len();
            if actual != length {
                return Err(AlignmentError::LengthMismatch {
                    expected: length,
                    actual,
                    id: sequence.id,
                });
            }

            normalised.push(sequence);
        }

        Ok(Self {
            sequences: normalised,
            length,
        })
    }
}
