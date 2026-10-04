use std::str::FromStr;

use crate::error::AlignmentError;

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
    pub(crate) const fn conservation_alphabet_size(self) -> Option<u8> {
        match self {
            Self::Dna => Some(4),
            Self::Protein => Some(20),
            Self::Generic => None,
        }
    }

    /// Returns whether this alignment type can be translated.
    ///
    /// Only DNA alignments support translation. Protein and generic alignments
    /// return `false`.
    pub const fn supports_translation(self) -> bool {
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
