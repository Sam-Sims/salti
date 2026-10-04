use std::str::FromStr;

use rayon::prelude::*;

use crate::{error::AlignmentError, model::Sequence, residue};

const SAMPLE_SIZE: usize = 100;
const CLASSIFICATION_THRESHOLD: f32 = 0.5;

/// The kind of residues in an alignment
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AlignmentType {
    Dna,
    Protein,
    /// Neither DNA nor protein. Has no unknown symbol and no conservation
    Generic,
}

impl AlignmentType {
    pub(crate) const fn conservation_alphabet_size(self) -> Option<u8> {
        match self {
            Self::Dna => Some(4),
            Self::Protein => Some(20),
            Self::Generic => None,
        }
    }

    /// Returns true if the type can be translated, which only DNA can
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

pub(crate) fn detect_alignment_type(sequences: &[Sequence]) -> AlignmentType {
    let step = sequences.len().div_ceil(SAMPLE_SIZE);
    let (protein_count, nucleotide_count, total_count) = sequences
        .par_iter()
        .step_by(step)
        .map(|sequence| count_residues(&sequence.residues))
        .reduce(
            || (0, 0, 0),
            |(protein_a, nucleotide_a, total_a), (protein_b, nucleotide_b, total_b)| {
                (
                    protein_a + protein_b,
                    nucleotide_a + nucleotide_b,
                    total_a + total_b,
                )
            },
        );

    if total_count == 0 {
        return AlignmentType::Generic;
    }

    let protein_fraction = protein_count as f32 / total_count as f32;
    let nucleotide_fraction = nucleotide_count as f32 / total_count as f32;
    let protein_matches = protein_fraction >= CLASSIFICATION_THRESHOLD;
    let nucleotide_matches = nucleotide_fraction >= CLASSIFICATION_THRESHOLD;

    match (protein_matches, nucleotide_matches) {
        (true, false) => AlignmentType::Protein,
        (false, true) => AlignmentType::Dna,
        (false, false) => AlignmentType::Generic,
        (true, true) => match protein_fraction.total_cmp(&nucleotide_fraction) {
            std::cmp::Ordering::Greater => AlignmentType::Protein,
            std::cmp::Ordering::Less => AlignmentType::Dna,
            std::cmp::Ordering::Equal => AlignmentType::Generic,
        },
    }
}

fn count_residues(residues: &[u8]) -> (usize, usize, usize) {
    residues
        .iter()
        .fold((0, 0, 0), |(protein, nucleotide, total), &byte| {
            (
                protein + usize::from(residue::is_detection_protein(byte)),
                nucleotide + usize::from(residue::is_detection_nucleotide(byte)),
                total + usize::from(!residue::is_gap(byte)),
            )
        })
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::detect_alignment_type;
    use crate::{AlignmentType, Sequence};

    fn sequences(rows: &[&str]) -> Vec<Sequence> {
        rows.iter()
            .enumerate()
            .map(|(i, sequence)| Sequence {
                id: format!("s{i}"),
                residues: sequence.as_bytes().to_vec(),
            })
            .collect()
    }

    #[rstest]
    #[case::dna(&["ACG---T", "TGC---A"], AlignmentType::Dna)]
    #[case::protein(&["ACDEFGHIKLMNPQRSTVWY", "LMNPQRSTVWYACDEFGHIK"], AlignmentType::Protein)]
    #[case::generic(&["<>VVV##VVV", "<>VVV##VVV"], AlignmentType::Generic)]
    #[case::case_insensitive(&["acgtATCtgcataACTT", "acgtATCtgcataACTT"], AlignmentType::Dna)]
    #[case::all_gaps(&["----", "----"], AlignmentType::Generic)]
    #[case::tie(&["AA", "EE"], AlignmentType::Generic)]
    #[case::at_threshold(&["ACGT", "ZZZZ"], AlignmentType::Dna)]
    #[case::below_threshold(&["ACGZ", "ZZZZ"], AlignmentType::Generic)]
    fn detect_alignment_type_works(#[case] rows: &[&str], #[case] expected: AlignmentType) {
        assert_eq!(detect_alignment_type(&sequences(rows)), expected);
    }

    #[test]
    fn detect_alignment_type_samples_across_all_rows() {
        let mut rows = vec!["####"; 100];
        rows.extend(vec!["ACGT"; 100]);
        assert_eq!(detect_alignment_type(&sequences(&rows)), AlignmentType::Dna);
    }
}
