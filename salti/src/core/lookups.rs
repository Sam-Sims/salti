/// Lookup table that maps each byte value (`0-255`) to a str for display.
///
/// All printable ASCII bytes map to themselves
/// Any byte not mapped to one of those outputs is rendered as `"?"`.
// TODO: This was originally for quick rendering of IUPAC bases and handling any chars outside of those
// Revist this now we have full ascii mapping
pub const BYTE_TO_CHAR: [&str; 256] = [
    "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?",
    "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", " ", "!", "\"", "#", "$", "%",
    "&", "'", "(", ")", "*", "+", ",", "-", ".", "/", "0", "1", "2", "3", "4", "5", "6", "7", "8",
    "9", ":", ";", "<", "=", ">", "?", "@", "A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K",
    "L", "M", "N", "O", "P", "Q", "R", "S", "T", "U", "V", "W", "X", "Y", "Z", "[", "\\", "]", "^",
    "_", "`", "a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "l", "m", "n", "o", "p", "q",
    "r", "s", "t", "u", "v", "w", "x", "y", "z", "{", "|", "}", "~", "?", "?", "?", "?", "?", "?",
    "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?",
    "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?",
    "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?",
    "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?",
    "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?",
    "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?",
    "?", "?", "?", "?", "?", "?", "?", "?", "?",
];

/// amino acid lookup table mapping each codon to an amino acid byte.
///
/// indexing layout is `[first][second][third]` nucleotide where each axis uses:
/// `a = 0`, `t = 1`, `c = 2`, `g = 3`.
pub const CODON_TO_AMINO_ACID: [[[u8; 4]; 4]; 4] = [
    [
        [b'K', b'N', b'N', b'K'],
        [b'I', b'I', b'I', b'M'],
        [b'T', b'T', b'T', b'T'],
        [b'R', b'S', b'S', b'R'],
    ],
    [
        [b'*', b'Y', b'Y', b'*'],
        [b'L', b'F', b'F', b'L'],
        [b'S', b'S', b'S', b'S'],
        [b'*', b'C', b'C', b'W'],
    ],
    [
        [b'Q', b'H', b'H', b'Q'],
        [b'L', b'L', b'L', b'L'],
        [b'P', b'P', b'P', b'P'],
        [b'R', b'R', b'R', b'R'],
    ],
    [
        [b'E', b'D', b'D', b'E'],
        [b'V', b'V', b'V', b'V'],
        [b'A', b'A', b'A', b'A'],
        [b'G', b'G', b'G', b'G'],
    ],
];

/// Normalises a nucleotide byte (uppercase and U->T).
///
/// Returns `None` for any non-canonical or ambiguous base.
#[inline]
#[must_use]
pub fn normalise_nucleotide(byte: u8) -> Option<u8> {
    match byte {
        b'A' | b'a' => Some(b'A'),
        b'C' | b'c' => Some(b'C'),
        b'G' | b'g' => Some(b'G'),
        b'T' | b't' | b'U' | b'u' => Some(b'T'),
        _ => None,
    }
}

/// Translates a codon to amino acid.
///
/// Expects codon bases in uppercase `A/T/C/G`.
/// Returns `b'X'` when any base is outside that alphabet.
#[inline]
#[must_use]
pub fn codon_to_amino_acid(codon: [u8; 3]) -> u8 {
    let index = |base| match base {
        b'A' => Some(0usize),
        b'T' => Some(1usize),
        b'C' => Some(2usize),
        b'G' => Some(3usize),
        _ => None,
    };

    let Some(first) = index(codon[0]) else {
        return b'X';
    };
    let Some(second) = index(codon[1]) else {
        return b'X';
    };
    let Some(third) = index(codon[2]) else {
        return b'X';
    };

    CODON_TO_AMINO_ACID[first][second][third]
}

/// Translates a codon from `sequence` of bytes starting at `start`.
///
/// The three input bases are normalised with `normalise_nucleotide` before
/// translation. Returns `b'X'` if the codon is incomplete or contains an
/// unsupported base.
#[inline]
#[must_use]
pub fn translate_codon(sequence: &[u8], start: usize) -> u8 {
    let mut codon = [0u8; 3];
    for (offset, slot) in codon.iter_mut().enumerate() {
        let Some(raw) = sequence.get(start + offset).copied() else {
            return b'X';
        };
        let Some(normalised) = normalise_nucleotide(raw) else {
            return b'X';
        };
        *slot = normalised;
    }

    codon_to_amino_acid(codon)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_translate_codon_with_ambiguous_base() {
        let seq = b"ATN";
        assert_eq!(translate_codon(seq, 0), b'X');
    }
}
