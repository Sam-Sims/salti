use crate::alignment_type::AlignmentType;

pub const GAP: u8 = b'-';

/// The amino acid symbol for a residue that is unknown or cannot be translated.
pub const UNKNOWN_AMINO_ACID: u8 = b'X';

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

/// Returns `true` if `byte` is a gap.
#[inline]
pub const fn is_gap(byte: u8) -> bool {
    byte == GAP
}

/// Returns `true` if `byte` is unknown symbol for `kind`.
///
/// DNA uses `N`/`n` and protein uses `X`/`x`. Generic alignments have no
/// unknown symbol.
#[inline]
pub const fn is_unknown(byte: u8, kind: AlignmentType) -> bool {
    match kind {
        AlignmentType::Dna => matches!(byte, b'N' | b'n'),
        AlignmentType::Protein => matches!(byte, b'X' | b'x'),
        AlignmentType::Generic => false,
    }
}

/// Returns the codon index of a nucleotide, case-insensitive.
///
/// `A` = 0, `T`/`U` = 1, `C` = 2, `G` = 3. Any other byte returns `None`.
#[inline]
pub const fn nucleotide_index(byte: u8) -> Option<usize> {
    match NUCLEOTIDE_INDEX_TABLE[byte as usize] {
        INVALID_NUCLEOTIDE => None,
        index => Some(index as usize),
    }
}

#[cfg(test)]
mod tests {
    use super::{GAP, UNKNOWN_AMINO_ACID, is_gap, is_unknown, nucleotide_index};
    use crate::alignment_type::AlignmentType;

    const fn test_is_gap(byte: u8) -> bool {
        matches!(byte, b'-')
    }

    fn test_norm_nuc(byte: u8) -> Option<u8> {
        let byte = byte.to_ascii_uppercase();

        match byte {
            b'A' | b'C' | b'G' | b'T' => Some(byte),
            b'U' => Some(b'T'),
            _ => None,
        }
    }

    fn test_index_nuc(base: u8) -> Option<usize> {
        match base {
            b'A' | b'a' => Some(0),
            b'T' | b't' | b'U' | b'u' => Some(1),
            b'C' | b'c' => Some(2),
            b'G' | b'g' => Some(3),
            _ => None,
        }
    }

    const fn test_us_unknown(byte: u8, kind: AlignmentType) -> bool {
        match kind {
            AlignmentType::Dna => matches!(byte, b'N' | b'n'),
            AlignmentType::Protein => matches!(byte, b'X' | b'x'),
            AlignmentType::Generic => false,
        }
    }

    #[test]
    fn gap_symbol_is_dash() {
        assert_eq!(GAP, b'-');
        assert!(!is_gap(b'.'));
    }

    #[test]
    fn unknown_amino_acid_is_protein_unknown() {
        assert!(is_unknown(UNKNOWN_AMINO_ACID, AlignmentType::Protein));
    }

    #[test]
    fn is_gap_matches_ref() {
        for byte in 0..=u8::MAX {
            assert_eq!(is_gap(byte), test_is_gap(byte), "byte {byte}");
        }
    }

    #[test]
    fn is_unknown_matches_ref() {
        for kind in [
            AlignmentType::Dna,
            AlignmentType::Protein,
            AlignmentType::Generic,
        ] {
            for byte in 0..=u8::MAX {
                assert_eq!(
                    is_unknown(byte, kind),
                    test_us_unknown(byte, kind),
                    "byte {byte}, kind {kind:?}"
                );
            }
        }
    }

    #[test]
    fn nucleotide_index_matches_ref() {
        for byte in 0..=u8::MAX {
            let expected = test_norm_nuc(byte).and_then(test_index_nuc);
            assert_eq!(nucleotide_index(byte), expected, "byte {byte}");
        }
    }

    #[test]
    fn test_u_is_t() {
        assert_eq!(nucleotide_index(b'U'), nucleotide_index(b'T'));
        assert_eq!(nucleotide_index(b'u'), nucleotide_index(b'T'));
    }
}
