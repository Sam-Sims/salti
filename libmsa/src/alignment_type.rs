use crate::error::AlignmentError;
use std::num::NonZeroU8;
use std::str::FromStr;

/// Describes the alignment type used by an alignment.
///
/// Alignments can either be DNA, protein, or generic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AlignmentType {
    Dna,
    Protein,
    Generic,
}

impl AlignmentType {
    /// Returns the alphabet size used for conservation calculations.
    pub const fn conservation_alphabet_size(self) -> Option<NonZeroU8> {
        match self {
            Self::Dna => NonZeroU8::new(4),
            Self::Protein => NonZeroU8::new(20),
            Self::Generic => None,
        }
    }

    /// Returns whether this alignment type can be translated.
    ///
    /// Only DNA alignments support translation. Protein and generic alignments
    /// return `false`.
    pub fn supports_translation(self) -> bool {
        matches!(self, Self::Dna)
    }
}

impl std::fmt::Display for AlignmentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Dna => f.write_str("dna"),
            Self::Protein => f.write_str("protein"),
            Self::Generic => f.write_str("generic"),
        }
    }
}

impl FromStr for AlignmentType {
    type Err = AlignmentError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "dna" => Ok(Self::Dna),
            "protein" => Ok(Self::Protein),
            "generic" => Ok(Self::Generic),
            _ => Err(AlignmentError::InvalidAlignmentType),
        }
    }
}