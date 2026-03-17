use std::num::NonZeroUsize;

use rand::seq::IndexedRandom;

use crate::alignment_type::AlignmentType;
use crate::data::AlignmentData;
use crate::error::AlignmentError;

// cant be zero
const DEFAULT_SAMPLE_SIZE: usize = 100;
const DEFAULT_CLASSIFICATION_THRESHOLD: f32 = 0.5;
const NUCLEOTIDE_BYTES: &[u8] = b"ACGTURYSWKMBDHVN-.";
const PROTEIN_BYTES: &[u8] = b"DEFHIKLMNPQRSVWYX-.";

/// Options that control alignment type detection.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DetectionOptions {
    /// The maximum number of sequences to sample when classifying an alignment.
    sample_size: NonZeroUsize,
    /// The minimum fraction of observed non-gap symbols that must match a
    /// before that classification is accepted.
    classification_threshold: f32,
}

impl Default for DetectionOptions {
    fn default() -> Self {
        Self {
            sample_size: NonZeroUsize::new(DEFAULT_SAMPLE_SIZE).unwrap(),
            classification_threshold: DEFAULT_CLASSIFICATION_THRESHOLD,
        }
    }
}

impl DetectionOptions {
    /// Creates a new [`DetectionOptions`] with the given parameters.
    /// 
    /// # Errors
    ///
    /// [`AlignmentError::InvalidClassificationThreshold`] if
    /// `classification_threshold` lies outside `0.0..=1.0`.
    pub fn new(
        sample_size: NonZeroUsize,
        classification_threshold: f32,
    ) -> Result<Self, AlignmentError> {
        if !(0.0..=1.0).contains(&classification_threshold)
        {
            return Err(AlignmentError::InvalidClassificationThreshold(
                classification_threshold,
            ));
        }

        Ok(Self {
            sample_size,
            classification_threshold,
        })
    }

    pub const fn sample_size(self) -> usize {
        self.sample_size.get()
    }

    pub const fn classification_threshold(self) -> f32 {
        self.classification_threshold
    }
}

pub(crate) fn detect_alignment_type(
    alignment: &AlignmentData,
    options: DetectionOptions,
    rng: &mut impl rand::Rng,
) -> AlignmentType {
    let (protein_count, nucleotide_count, total_count) = alignment
        .sequences
        .choose_multiple(rng, options.sample_size())
        .flat_map(|sequence| sequence.sequence().iter().copied())
        .filter(|byte| !matches!(byte, b'-' | b'.'))
        .map(|byte| byte.to_ascii_uppercase())
        .fold(
            (0usize, 0usize, 0usize),
            |(protein, nucleotide, total), byte| {
                (
                    protein + usize::from(PROTEIN_BYTES.contains(&byte)),
                    nucleotide + usize::from(NUCLEOTIDE_BYTES.contains(&byte)),
                    total + 1,
                )
            },
        );

    if total_count == 0 {
        return AlignmentType::Generic;
    }

    let protein_fraction = protein_count as f32 / total_count as f32;
    let nucleotide_fraction = nucleotide_count as f32 / total_count as f32;
    let protein_matches = protein_fraction >= options.classification_threshold();
    let nucleotide_matches = nucleotide_fraction >= options.classification_threshold();
    
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

#[cfg(test)]
mod detect_alignment_type_tests {
    use std::num::NonZeroUsize;

    use rand::{SeedableRng, rngs::StdRng};

    use super::{DetectionOptions, detect_alignment_type};
    use crate::data::AlignmentData;
    use crate::{AlignmentError, AlignmentType, RawSequence};

    fn raw(id: &str, sequence: &[u8]) -> RawSequence {
        RawSequence {
            id: id.to_string(),
            sequence: sequence.to_vec(),
        }
    }

    fn make_data(rows: &[(&str, &[u8])]) -> AlignmentData {
        AlignmentData::from_raw(rows.iter().map(|(id, seq)| raw(id, seq)).collect()).unwrap()
    }

    fn detect_with_seed(
        data: &AlignmentData,
        options: DetectionOptions,
        seed: u64,
    ) -> AlignmentType {
        let mut rng = StdRng::seed_from_u64(seed);
        detect_alignment_type(data, options, &mut rng)
    }

    #[test]
    fn detect_classifies_dna() {
        let data = make_data(&[("seq-1", b"ACG---T"), ("seq-2", b"TGC---A")]);
        assert_eq!(
            detect_with_seed(&data, DetectionOptions::default(), 1),
            AlignmentType::Dna
        );
    }

    #[test]
    fn detect_classifies_protein() {
        let data = make_data(&[
            ("seq-1", b"ACDEFGHIKLMNPQRSTVWY"),
            ("seq-2", b"LMNPQRSTVWYACDEFGHIK"),
        ]);
        assert_eq!(
            detect_with_seed(&data, DetectionOptions::default(), 2),
            AlignmentType::Protein
        );
    }

    #[test]
    fn detect_classifies_generic() {
        let data = make_data(&[("seq-1", b"<>VVV##VVV"), ("seq-2", b"<>VVV##VVV")]);
        assert_eq!(
            detect_with_seed(&data, DetectionOptions::default(), 3),
            AlignmentType::Generic
        );
    }

    #[test]
    fn detect_is_case_insensitive() {
        let data = make_data(&[
            ("seq-1", b"acgtATCtgcataACTT"),
            ("seq-2", b"acgtATCtgcataACTT"),
        ]);
        assert_eq!(
            detect_with_seed(&data, DetectionOptions::default(), 4),
            AlignmentType::Dna
        );
    }

    #[test]
    fn detect_returns_generic_for_all_gaps() {
        let data = make_data(&[("seq-1", b"--.."), ("seq-2", b".-.-")]);
        assert_eq!(
            detect_with_seed(&data, DetectionOptions::default(), 5),
            AlignmentType::Generic
        );
    }

    #[test]
    fn detect_returns_generic_for_tie() {
        let data = make_data(&[("seq-1", b"AA"), ("seq-2", b"EE")]);
        assert_eq!(
            detect_with_seed(&data, DetectionOptions::default(), 6),
            AlignmentType::Generic
        );
    }

    #[test]
    fn detect_override_with_options() {
        let data = make_data(&[("seq-1", b"ACGT"), ("seq-2", b"ATCZ"), ("seq-3", b"ZZZZ")]);

        let default_kind = detect_with_seed(&data, DetectionOptions::default(), 7);
        let strict_kind = detect_with_seed(
            &data,
            DetectionOptions::new(NonZeroUsize::new(100).unwrap(), 0.95).unwrap(),
            7,
        );

        assert_eq!(default_kind, AlignmentType::Dna);
        assert_eq!(strict_kind, AlignmentType::Generic);
    }

    #[test]
    fn rejects_out_of_range_threshold() {
        assert_eq!(
            DetectionOptions::new(NonZeroUsize::new(1).unwrap(), 1.5),
            Err(AlignmentError::InvalidClassificationThreshold(1.5))
        );
    }

    #[test]
    fn rejects_nan_threshold() {
        assert!(matches!(
            DetectionOptions::new(NonZeroUsize::new(1).unwrap(), f32::NAN),
            Err(AlignmentError::InvalidClassificationThreshold(value)) if value.is_nan()
        ));
    }

    #[test]
    fn detect_sampling_respects_requested_row_limit() {
        let data = make_data(&[
            ("dna-1", b"TTTT"),
            ("dna-2", b"TTTT"),
            ("protein-1", b"EEEE"),
            ("protein-2", b"EEEE"),
            ("protein-3", b"EEEE"),
            ("protein-4", b"EEEE"),
            ("protein-5", b"EEEE"),
            ("protein-6", b"EEEE"),
        ]);
        let sample_two_options =
            DetectionOptions::new(NonZeroUsize::new(2).unwrap(), 0.75).unwrap();
        let sample_all_options =
            DetectionOptions::new(NonZeroUsize::new(8).unwrap(), 0.75).unwrap();
        let seed = (0..256)
            .find(|&seed| {
                detect_with_seed(&data, sample_two_options, seed) == AlignmentType::Dna
                    && detect_with_seed(&data, sample_all_options, seed) == AlignmentType::Protein
            })
            .expect("a deterministic seed should expose the sample-size difference");

        assert_eq!(
            detect_with_seed(&data, sample_two_options, seed),
            AlignmentType::Dna
        );
        assert_eq!(
            detect_with_seed(&data, sample_all_options, seed),
            AlignmentType::Protein
        );
    }
}
