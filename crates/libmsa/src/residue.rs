//! Rules for single residue bytes

use crate::alignment_type::AlignmentType;

const GAP: u8 = b'-';

pub(crate) const UNKNOWN_AMINO_ACID: u8 = b'X';

const INVALID_NUCLEOTIDE: u8 = 4;

const NUCLEOTIDE_INDEX_TABLE: [u8; 256] = build_nucleotide_index_table();

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

/// Returns true if `byte` is a gap. Only `-` is a gap
#[inline]
pub const fn is_gap(byte: u8) -> bool {
    byte == GAP
}

/// Returns true if `byte` is the unknown symbol for `kind`
///
/// DNA uses `N` or `n`, protein uses `X` or `x`, and generic has none
#[inline]
pub const fn is_unknown(byte: u8, kind: AlignmentType) -> bool {
    match kind {
        AlignmentType::Dna => matches!(byte, b'N' | b'n'),
        AlignmentType::Protein => matches!(byte, b'X' | b'x'),
        AlignmentType::Generic => false,
    }
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
pub(crate) const fn is_detection_nucleotide(byte: u8) -> bool {
    matches!(
        byte.to_ascii_uppercase(),
        b'A' | b'C'
            | b'G'
            | b'T'
            | b'U'
            | b'R'
            | b'Y'
            | b'S'
            | b'W'
            | b'K'
            | b'M'
            | b'B'
            | b'D'
            | b'H'
            | b'V'
            | b'N'
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

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::{
        UNKNOWN_AMINO_ACID, is_detection_nucleotide, is_detection_protein, is_gap, is_unknown,
        nucleotide_index,
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
    #[case::dna_upper(AlignmentType::Dna, b'N')]
    #[case::dna_lower(AlignmentType::Dna, b'n')]
    #[case::protein_upper(AlignmentType::Protein, b'X')]
    #[case::protein_lower(AlignmentType::Protein, b'x')]
    #[case::translated_unknown(AlignmentType::Protein, UNKNOWN_AMINO_ACID)]
    fn is_unknown_works(#[case] kind: AlignmentType, #[case] byte: u8) {
        assert!(is_unknown(byte, kind));
    }

    #[rstest]
    #[case::dna_x(AlignmentType::Dna, b'X')]
    #[case::protein_n(AlignmentType::Protein, b'N')]
    #[case::generic_n(AlignmentType::Generic, b'N')]
    #[case::generic_x(AlignmentType::Generic, b'X')]
    #[case::gap(AlignmentType::Dna, b'-')]
    fn is_unknown_rejects(#[case] kind: AlignmentType, #[case] byte: u8) {
        assert!(!is_unknown(byte, kind));
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
}
