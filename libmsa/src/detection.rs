use rayon::prelude::*;

use crate::{alignment_type::AlignmentType, data::AlignmentData, residue};

const SAMPLE_SIZE: usize = 100;
const CLASSIFICATION_THRESHOLD: f32 = 0.5;

pub(crate) fn detect_alignment_type(alignment: &AlignmentData) -> AlignmentType {
    let step = alignment.sequences.len().div_ceil(SAMPLE_SIZE);
    let (protein_count, nucleotide_count, total_count) = alignment
        .sequences
        .par_iter()
        .step_by(step)
        .map(|sequence| count_residues(&sequence.sequence))
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

fn count_residues(sequence: &[u8]) -> (usize, usize, usize) {
    sequence
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
mod detect_alignment_type_tests {
    use super::detect_alignment_type;
    use crate::{AlignmentType, RawSequence, data::AlignmentData};

    fn raw(id: &str, sequence: &[u8]) -> RawSequence {
        RawSequence {
            id: id.to_string(),
            sequence: sequence.to_vec(),
        }
    }

    fn make_data(rows: &[(&str, &[u8])]) -> AlignmentData {
        let sequences = rows
            .iter()
            .map(|(id, seq)| raw(id, seq).try_into())
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        AlignmentData::new(sequences).unwrap()
    }

    #[test]
    fn detect_classifies_dna() {
        let data = make_data(&[("seq-1", b"ACG---T"), ("seq-2", b"TGC---A")]);
        assert_eq!(detect_alignment_type(&data), AlignmentType::Dna);
    }

    #[test]
    fn detect_classifies_protein() {
        let data = make_data(&[
            ("seq-1", b"ACDEFGHIKLMNPQRSTVWY"),
            ("seq-2", b"LMNPQRSTVWYACDEFGHIK"),
        ]);
        assert_eq!(detect_alignment_type(&data), AlignmentType::Protein);
    }

    #[test]
    fn detect_classifies_generic() {
        let data = make_data(&[("seq-1", b"<>VVV##VVV"), ("seq-2", b"<>VVV##VVV")]);
        assert_eq!(detect_alignment_type(&data), AlignmentType::Generic);
    }

    #[test]
    fn detect_is_case_insensitive() {
        let data = make_data(&[
            ("seq-1", b"acgtATCtgcataACTT"),
            ("seq-2", b"acgtATCtgcataACTT"),
        ]);
        assert_eq!(detect_alignment_type(&data), AlignmentType::Dna);
    }

    #[test]
    fn detect_returns_generic_for_all_gaps() {
        let data = make_data(&[("seq-1", b"--.."), ("seq-2", b".-.-")]);
        assert_eq!(detect_alignment_type(&data), AlignmentType::Generic);
    }

    #[test]
    fn detect_returns_generic_for_tie() {
        let data = make_data(&[("seq-1", b"AA"), ("seq-2", b"EE")]);
        assert_eq!(detect_alignment_type(&data), AlignmentType::Generic);
    }

    #[test]
    fn detect_accepts_threshold_fraction() {
        let data = make_data(&[("seq-1", b"ACGT"), ("seq-2", b"ZZZZ")]);
        assert_eq!(detect_alignment_type(&data), AlignmentType::Dna);
    }

    #[test]
    fn detect_rejects_below_threshold_fraction() {
        let data = make_data(&[("seq-1", b"ACGZ"), ("seq-2", b"ZZZZ")]);
        assert_eq!(detect_alignment_type(&data), AlignmentType::Generic);
    }

    #[test]
    fn detect_samples_across_all_rows() {
        let generic = vec![raw("generic", b"####"); 100];
        let dna = vec![raw("dna", b"ACGT"); 100];
        let sequences = generic
            .into_iter()
            .chain(dna)
            .map(|sequence| sequence.try_into().unwrap())
            .collect();
        let data = AlignmentData::new(sequences).unwrap();
        assert_eq!(detect_alignment_type(&data), AlignmentType::Dna);
    }
}
