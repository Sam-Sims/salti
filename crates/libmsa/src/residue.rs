//! Rules for single residue bytes

use crate::alignment_type::AlignmentType;

pub(crate) const GAP: u8 = b'-';

pub(crate) const UNKNOWN_AMINO_ACID: u8 = b'X';

const INVALID_NUCLEOTIDE: u8 = 4;

const NUCLEOTIDE_INDEX_TABLE: [u8; 256] = build_nucleotide_index_table();

// reverse complement translation relies on being able to XOR this so never change the order
const fn build_nucleotide_index_table() -> [u8; 256] {
    let mut table = [INVALID_NUCLEOTIDE; 256];

    table[b'A' as usize] = 0;
    table[b'a' as usize] = 0;
    table[b'T' as usize] = 1;
    table[b't' as usize] = 1;
    table[b'U' as usize] = 1;
    table[b'u' as usize] = 1;
    table[b'C' as usize] = 2;
    table[b'c' as usize] = 2;
    table[b'G' as usize] = 3;
    table[b'g' as usize] = 3;

    table
}

/// Biological meaning of a byte
#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::VariantArray)]
pub enum Class {
    /// `-`
    Gap,
    /// The unknown symbol, which says nothing about the residue: `N` for DNA, `X` for
    /// protein, either case
    Unknown,
    /// A DNA ambiguity code, which could be one of several bases: `R Y S W K M B D H V`,
    /// either case
    Ambiguous,
    /// Anything else, including bytes outside the alphabet
    Residue,
}

/// Returns what `byte` is in an alignment of `kind`
///
/// `-` is a gap in DNA and protein. DNA has an unknown symbol and ambiguity codes, and protein
/// has only an unknown symbol. Generic has none of these, so every byte is a residue. Case is
/// ignored
#[inline]
pub const fn classify(byte: u8, kind: AlignmentType) -> Class {
    match (kind, byte.to_ascii_uppercase()) {
        (AlignmentType::Generic, _) => Class::Residue,
        (_, GAP) => Class::Gap,
        (AlignmentType::Dna, b'N') | (AlignmentType::Protein, UNKNOWN_AMINO_ACID) => Class::Unknown,
        (
            AlignmentType::Dna,
            b'R' | b'Y' | b'S' | b'W' | b'K' | b'M' | b'B' | b'D' | b'H' | b'V',
        ) => Class::Ambiguous,
        _ => Class::Residue,
    }
}

/// Returns true if `a` and `b` are the same residue for `kind`
///
/// DNA and protein ignore case, and DNA treats `U` as `T`. Generic compares bytes exactly
#[inline]
pub const fn same_residue(a: u8, b: u8, kind: AlignmentType) -> bool {
    fold(a, kind) == fold(b, kind)
}

/// Returns the index of a nucleotide in a codon, case-insensitive
///
/// `A` is 0, `T` and `U` are 1, `C` is 2 and `G` is 3. Any other byte is `None`
#[inline]
pub const fn nucleotide_index(byte: u8) -> Option<usize> {
    match NUCLEOTIDE_INDEX_TABLE[byte as usize] {
        INVALID_NUCLEOTIDE => None,
        index => Some(index as usize),
    }
}

#[inline]
pub(crate) const fn is_gap(byte: u8) -> bool {
    byte == GAP
}

#[inline]
pub(crate) const fn is_detection_nucleotide(byte: u8) -> bool {
    nucleotide_index(byte).is_some()
        || matches!(
            classify(byte, AlignmentType::Dna),
            Class::Unknown | Class::Ambiguous
        )
}

#[inline]
pub(crate) const fn is_detection_protein(byte: u8) -> bool {
    matches!(
        byte.to_ascii_uppercase(),
        b'D' | b'E'
            | b'F'
            | b'H'
            | b'I'
            | b'K'
            | b'L'
            | b'M'
            | b'N'
            | b'P'
            | b'Q'
            | b'R'
            | b'S'
            | b'V'
            | b'W'
            | b'Y'
            | b'X'
    )
}

#[inline]
pub(crate) const fn fold(byte: u8, kind: AlignmentType) -> u8 {
    match (kind, byte.to_ascii_uppercase()) {
        (AlignmentType::Dna, b'U') => b'T',
        (AlignmentType::Dna | AlignmentType::Protein, upper) => upper,
        (AlignmentType::Generic, _) => byte,
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::{
        Class, UNKNOWN_AMINO_ACID, classify, is_detection_nucleotide, is_detection_protein, is_gap,
        nucleotide_index, same_residue,
    };
    use crate::alignment_type::AlignmentType;

    #[test]
    fn is_gap_works() {
        assert!(is_gap(b'-'));
    }

    #[rstest]
    #[case::dot(b'.')]
    #[case::space(b' ')]
    #[case::residue(b'A')]
    fn is_gap_rejects(#[case] byte: u8) {
        assert!(!is_gap(byte));
    }

    #[rstest]
    #[case::dna_same(AlignmentType::Dna, b'A', b'A')]
    #[case::dna_case(AlignmentType::Dna, b'a', b'A')]
    #[case::dna_unknown_case(AlignmentType::Dna, b'n', b'N')]
    #[case::dna_u_is_t(AlignmentType::Dna, b'u', b'T')]
    #[case::protein_case(AlignmentType::Protein, b'l', b'L')]
    #[case::generic_same(AlignmentType::Generic, b'a', b'a')]
    fn same_residue_works(#[case] kind: AlignmentType, #[case] a: u8, #[case] b: u8) {
        assert!(same_residue(a, b, kind));
    }

    #[rstest]
    #[case::dna_different(AlignmentType::Dna, b'A', b'G')]
    #[case::unknown_vs_base(AlignmentType::Dna, b'N', b'A')]
    #[case::gap_vs_residue(AlignmentType::Dna, b'-', b'A')]
    #[case::generic_case(AlignmentType::Generic, b'a', b'A')]
    #[case::protein_u_is_not_t(AlignmentType::Protein, b'U', b'T')]
    #[case::generic_u_is_not_t(AlignmentType::Generic, b'U', b'T')]
    fn same_residue_rejects(#[case] kind: AlignmentType, #[case] a: u8, #[case] b: u8) {
        assert!(!same_residue(a, b, kind));
    }

    #[rstest]
    #[case::a(b'A', 0)]
    #[case::t(b'T', 1)]
    #[case::c(b'C', 2)]
    #[case::g(b'G', 3)]
    #[case::lowercase(b'g', 3)]
    #[case::u_is_t(b'U', 1)]
    #[case::lowercase_u_is_t(b'u', 1)]
    fn nucleotide_index_works(#[case] byte: u8, #[case] expected: usize) {
        assert_eq!(nucleotide_index(byte), Some(expected));
    }

    #[rstest]
    #[case::ambiguity(b'N')]
    #[case::gap(b'-')]
    #[case::non_ascii(0x80)]
    fn nucleotide_index_is_none(#[case] byte: u8) {
        assert_eq!(nucleotide_index(byte), None);
    }

    #[rstest]
    #[case::base(b'A')]
    #[case::rna(b'U')]
    #[case::ambiguity(b'R')]
    #[case::unknown(b'N')]
    #[case::lowercase(b'y')]
    fn is_detection_nucleotide_works(#[case] byte: u8) {
        assert!(is_detection_nucleotide(byte));
    }

    #[rstest]
    #[case::gap(b'-')]
    #[case::protein_only(b'E')]
    #[case::non_ascii(0x80)]
    fn is_detection_nucleotide_rejects(#[case] byte: u8) {
        assert!(!is_detection_nucleotide(byte));
    }

    #[rstest]
    #[case::protein_only(b'E')]
    #[case::shared_with_ambiguity(b'R')]
    #[case::unknown(b'X')]
    #[case::lowercase(b'l')]
    fn is_detection_protein_works(#[case] byte: u8) {
        assert!(is_detection_protein(byte));
    }

    #[rstest]
    #[case::gap(b'-')]
    #[case::nucleotide_a(b'A')]
    #[case::nucleotide_c(b'C')]
    #[case::non_ascii(0x80)]
    fn is_detection_protein_rejects(#[case] byte: u8) {
        assert!(!is_detection_protein(byte));
    }

    #[rstest]
    #[case::dna_gap(AlignmentType::Dna, b'-', Class::Gap)]
    #[case::dna_unknown(AlignmentType::Dna, b'N', Class::Unknown)]
    #[case::dna_unknown_lower(AlignmentType::Dna, b'n', Class::Unknown)]
    #[case::dna_ambiguous(AlignmentType::Dna, b'R', Class::Ambiguous)]
    #[case::dna_ambiguous_lower(AlignmentType::Dna, b'y', Class::Ambiguous)]
    #[case::dna_base(AlignmentType::Dna, b'a', Class::Residue)]
    #[case::dna_outside_alphabet(AlignmentType::Dna, b'?', Class::Residue)]
    #[case::protein_gap(AlignmentType::Protein, b'-', Class::Gap)]
    #[case::protein_unknown(AlignmentType::Protein, b'x', Class::Unknown)]
    #[case::protein_r_is_residue(AlignmentType::Protein, b'R', Class::Residue)]
    #[case::protein_n_is_residue(AlignmentType::Protein, b'N', Class::Residue)]
    #[case::generic_gap_is_residue(AlignmentType::Generic, b'-', Class::Residue)]
    #[case::generic_n_is_residue(AlignmentType::Generic, b'N', Class::Residue)]
    #[case::generic_ambiguous_is_residue(AlignmentType::Generic, b'R', Class::Residue)]
    #[case::dna_three_base_ambiguous(AlignmentType::Dna, b'V', Class::Ambiguous)]
    #[case::dna_ambiguous_k_lower(AlignmentType::Dna, b'k', Class::Ambiguous)]
    #[case::dna_rna_is_residue(AlignmentType::Dna, b'U', Class::Residue)]
    #[case::dna_protein_only_is_residue(AlignmentType::Dna, b'E', Class::Residue)]
    #[case::dna_x_is_residue(AlignmentType::Dna, b'X', Class::Residue)]
    #[case::dna_dot_is_residue(AlignmentType::Dna, b'.', Class::Residue)]
    #[case::protein_unknown_upper(AlignmentType::Protein, b'X', Class::Unknown)]
    #[case::translated_unknown(AlignmentType::Protein, UNKNOWN_AMINO_ACID, Class::Unknown)]
    #[case::generic_x_is_residue(AlignmentType::Generic, b'X', Class::Residue)]
    fn classify_works(#[case] kind: AlignmentType, #[case] byte: u8, #[case] expected: Class) {
        assert_eq!(classify(byte, kind), expected);
    }
}
