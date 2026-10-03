use std::{num::NonZeroU8, ops::Range};

use crate::{
    AlignmentType,
    counts::count_columns,
    error::AlignmentError,
    model::{Alignment, validate_column_range},
    residue,
};

/// Calculated values for a single alignment column.
#[derive(Debug, Clone, PartialEq)]
pub struct ColumnSummary {
    pub position: usize,
    /// The most frequent byte in the column under the chosen [`ConsensusMethod`].
    ///
    /// Ties go to the lowest byte; see [`ConsensusMethod`]. `None` when no byte
    /// was counted, such as an all-gap column under [`ConsensusMethod::MajorityNonGap`].
    pub consensus: Option<u8>,
    pub conservation: Option<f32>,
}

/// Selects how consensus bytes are chosen for alignment columns.
///
/// Different methods vary in whether gap characters are considered when
/// determining the representative byte for a column.
///
/// # Ties
///
/// When several bytes share the highest count, the one with the lowest ASCII
/// value wins, so the result is deterministic. Bytes are compared as-is, which
/// means:
///
/// - `A` beats `C`, and `C` beats `T`.
/// - Uppercase beats lowercase, so `T` beats `a`.
/// - Under [`ConsensusMethod::Majority`], gaps (`-`, `.`) and `*` beat every letter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ConsensusMethod {
    /// Chooses the most frequent byte, including gap characters.
    Majority,
    /// Chooses the most frequent non-gap byte.
    #[default]
    MajorityNonGap,
}

impl ConsensusMethod {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Majority => "majority",
            Self::MajorityNonGap => "majority-non-gap",
        }
    }

    pub const fn all() -> [Self; 2] {
        [Self::Majority, Self::MajorityNonGap]
    }
}

impl std::fmt::Display for ConsensusMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

impl std::str::FromStr for ConsensusMethod {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::all()
            .into_iter()
            .find(|method| method.name() == value)
            .ok_or(())
    }
}

impl Alignment {
    /// Returns a derived summary for each column in `range`.
    ///
    /// Each position is resolved against the alignment's current column projection.
    /// The returned vector keeps the requested relative positions and contains a
    /// [`ColumnSummary`] with consensus, gap fraction, and conservation when that
    /// measure is defined for the active alignment kind.
    ///
    /// # Errors
    ///
    /// [`AlignmentError::EmptyRange`] if `range` is empty.
    ///
    /// [`AlignmentError::ColumnOutOfBounds`] if `range.end` is greater than the
    /// current column projection width.
    pub fn column_summaries_range(
        &self,
        range: Range<usize>,
        method: ConsensusMethod,
    ) -> Result<Vec<ColumnSummary>, AlignmentError> {
        validate_column_range(&range, self.columns.len())?;

        let counts = count_columns(&self.data, &self.rows, &self.columns, range.clone());
        Ok(summaries_from_counts(
            range,
            &counts,
            method,
            self.active_type().conservation_alphabet_size(),
        ))
    }
}

pub(crate) fn summaries_from_counts(
    positions: Range<usize>,
    counts: &[[u32; 128]],
    method: ConsensusMethod,
    alphabet_size: Option<NonZeroU8>,
) -> Vec<ColumnSummary> {
    debug_assert_eq!(positions.len(), counts.len());
    let max_entropy = alphabet_size.map(|value| f64::from(value.get()).log2());

    positions
        .zip(counts)
        .map(|(position, counts)| ColumnSummary {
            position,
            consensus: consensus_from_counts(counts, method),
            conservation: max_entropy
                .map(|max_entropy| conservation_from_counts(counts, max_entropy)),
        })
        .collect()
}

pub(crate) fn gap_fraction_from_counts(counts: &[u32; 128]) -> f32 {
    let total: u32 = counts.iter().sum();
    let gap_count: u32 = symbol_counts(counts)
        .filter(|&(symbol, _)| residue::is_gap(symbol))
        .map(|(_, count)| count)
        .sum();

    if total == 0 {
        0.0
    } else {
        gap_count as f32 / total as f32
    }
}

pub(crate) fn max_counted_symbol_fraction_from_counts(
    counts: &[u32; 128],
    kind: AlignmentType,
) -> Option<f32> {
    let mut counted_total = 0u32;
    let mut max_count = 0u32;

    for (symbol, count) in symbol_counts(counts) {
        if count == 0 || is_ignored_constant_symbol(symbol, kind) {
            continue;
        }

        counted_total += count;
        max_count = max_count.max(count);
    }

    (counted_total != 0).then_some(max_count as f32 / counted_total as f32)
}

fn consensus_from_counts(counts: &[u32; 128], method: ConsensusMethod) -> Option<u8> {
    let exclude_gap = matches!(method, ConsensusMethod::MajorityNonGap);
    let mut max_count = 0u32;
    let mut consensus = None;

    for (symbol, count) in symbol_counts(counts) {
        if count > max_count && !(exclude_gap && residue::is_gap(symbol)) {
            max_count = count;
            consensus = Some(symbol);
        }
    }

    consensus
}

fn conservation_from_counts(counts: &[u32; 128], max_entropy: f64) -> f32 {
    let mut total = 0u32;
    let mut gap_count = 0u32;
    let mut merged_non_gap_counts = [0u32; 128];

    for (symbol, count) in symbol_counts(counts) {
        if count == 0 {
            continue;
        }

        total += count;
        if residue::is_gap(symbol) {
            gap_count += count;
        } else {
            merged_non_gap_counts[usize::from(symbol.to_ascii_uppercase())] += count;
        }
    }

    let non_gap_total = total - gap_count;
    if non_gap_total == 0 {
        return 0.0;
    }

    let mut entropy = 0.0f64;
    let non_gap_total_f = f64::from(non_gap_total);
    for &count in &merged_non_gap_counts {
        if count == 0 {
            continue;
        }
        let frequency = f64::from(count) / non_gap_total_f;
        entropy -= frequency * frequency.log2();
    }

    let gap_fraction = f64::from(gap_count) / f64::from(total);
    let conservation = (1.0 - entropy / max_entropy).max(0.0);
    (conservation * (1.0 - gap_fraction)) as f32
}

fn symbol_counts(counts: &[u32; 128]) -> impl Iterator<Item = (u8, u32)> + '_ {
    (0..128).zip(counts.iter().copied())
}

#[inline]
const fn is_ignored_constant_symbol(byte: u8, kind: AlignmentType) -> bool {
    residue::is_gap(byte) || residue::is_unknown(byte, kind)
}

#[cfg(test)]
mod consensus_count_tests {
    use super::{ConsensusMethod, consensus_from_counts};

    fn counts_for(symbols: &[u8]) -> [u32; 128] {
        let mut counts = [0u32; 128];
        for &symbol in symbols {
            counts[usize::from(symbol)] += 1;
        }
        counts
    }

    #[test]
    fn consensus_same() {
        let counts = counts_for(b"AAAA");
        assert_eq!(
            consensus_from_counts(&counts, ConsensusMethod::Majority),
            Some(b'A')
        );
    }

    #[test]
    fn consensus_majority_gap() {
        let counts = counts_for(b"---AT");
        assert_eq!(
            consensus_from_counts(&counts, ConsensusMethod::Majority),
            Some(b'-')
        );
    }

    #[test]
    fn consensus_majority_nongap_excludes_gaps() {
        let counts = counts_for(b"---AAT");
        assert_eq!(
            consensus_from_counts(&counts, ConsensusMethod::MajorityNonGap),
            Some(b'A')
        );
    }

    #[test]
    fn consensus_no_candidates_returns_none() {
        let counts = [0u32; 128];
        assert_eq!(
            consensus_from_counts(&counts, ConsensusMethod::Majority),
            None
        );
    }

    #[test]
    fn consensus_tie_picks_lowest_byte() {
        let counts = counts_for(b"TTCCAA");
        assert_eq!(
            consensus_from_counts(&counts, ConsensusMethod::Majority),
            Some(b'A')
        );
    }

    #[test]
    fn consensus_majority_tie_prefers_gap() {
        let counts = counts_for(b"--AA");
        assert_eq!(
            consensus_from_counts(&counts, ConsensusMethod::Majority),
            Some(b'-')
        );
    }

    #[test]
    fn consensus_majority_nongap_tie_ignores_gap() {
        let counts = counts_for(b"---GGTT");
        assert_eq!(
            consensus_from_counts(&counts, ConsensusMethod::MajorityNonGap),
            Some(b'G')
        );
    }

    #[test]
    fn consensus_higher_count_beats_lower_byte() {
        let counts = counts_for(b"ACCTTT");
        assert_eq!(
            consensus_from_counts(&counts, ConsensusMethod::Majority),
            Some(b'T')
        );
    }
}

#[cfg(test)]
mod derived_column_tests {
    use std::num::NonZeroU8;

    use super::{ConsensusMethod, summaries_from_counts};

    fn counts_for(symbols: &[u8]) -> [u32; 128] {
        let mut counts = [0u32; 128];
        for &symbol in symbols {
            counts[usize::from(symbol)] += 1;
        }
        counts
    }

    #[test]
    fn summaries_from_counts_return_none_for_all_gap_column() {
        let counts = [counts_for(b"---")];
        let summaries = summaries_from_counts(
            3..4,
            &counts,
            ConsensusMethod::MajorityNonGap,
            Some(NonZeroU8::new(4).unwrap()),
        );

        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].position, 3);
        assert_eq!(summaries[0].consensus, None);
        assert_eq!(summaries[0].conservation, Some(0.0));
    }

    #[test]
    fn summaries_from_counts_report_conservation_extremes() {
        let counts = [counts_for(b"AAAA"), counts_for(b"----")];
        let summaries = summaries_from_counts(
            0..2,
            &counts,
            ConsensusMethod::MajorityNonGap,
            Some(NonZeroU8::new(4).unwrap()),
        );

        assert_eq!(summaries.len(), 2);
        assert_eq!(summaries[0].position, 0);
        assert_eq!(summaries[0].consensus, Some(b'A'));
        assert_eq!(summaries[0].conservation, Some(1.0));
        assert_eq!(summaries[1].position, 1);
        assert_eq!(summaries[1].consensus, None);
        assert_eq!(summaries[1].conservation, Some(0.0));
    }
}

#[cfg(test)]
mod conservation_count_tests {
    use super::conservation_from_counts;

    fn counts_for(symbols: &[u8]) -> [u32; 128] {
        let mut counts = [0u32; 128];
        for &symbol in symbols {
            counts[usize::from(symbol)] += 1;
        }
        counts
    }

    const DNA_MAX_ENTROPY: f64 = 2.0;

    #[test]
    fn fully_conserved() {
        let counts = counts_for(b"AAAA");
        assert_eq!(conservation_from_counts(&counts, DNA_MAX_ENTROPY), 1.0);
    }

    #[test]
    fn all_gaps() {
        let counts = counts_for(b"----");
        assert_eq!(conservation_from_counts(&counts, DNA_MAX_ENTROPY), 0.0);
    }

    #[test]
    fn gap_penalty() {
        let counts = counts_for(b"AA--");
        assert_eq!(conservation_from_counts(&counts, DNA_MAX_ENTROPY), 0.5);
    }

    #[test]
    fn case_insensitive() {
        let counts = counts_for(b"AaAa");
        assert_eq!(conservation_from_counts(&counts, DNA_MAX_ENTROPY), 1.0);
    }

    #[test]
    fn empty_column() {
        let counts = [0u32; 128];
        assert_eq!(conservation_from_counts(&counts, DNA_MAX_ENTROPY), 0.0);
    }

    #[test]
    fn mixed_symbols_reduces_conservation() {
        let conserved = conservation_from_counts(&counts_for(b"AAAA"), DNA_MAX_ENTROPY);
        let mixed = conservation_from_counts(&counts_for(b"AACT"), DNA_MAX_ENTROPY);
        assert!(mixed < conserved);
        assert!(mixed > 0.0);
    }
}

#[cfg(test)]
mod constant_fraction_count_tests {
    use super::max_counted_symbol_fraction_from_counts;
    use crate::AlignmentType;

    fn counts_for(symbols: &[u8]) -> [u32; 128] {
        let mut counts = [0u32; 128];
        for &symbol in symbols {
            counts[usize::from(symbol)] += 1;
        }
        counts
    }

    #[test]
    fn dna_constant_fraction_ignores_gaps_and_ns() {
        let counts = counts_for(b"AANn--T");
        let fraction = max_counted_symbol_fraction_from_counts(&counts, AlignmentType::Dna);

        assert_eq!(fraction, Some(2.0 / 3.0));
    }

    #[test]
    fn protein_constant_fraction_ignores_gaps_and_xs() {
        let counts = counts_for(b"MMXx--K");
        let fraction = max_counted_symbol_fraction_from_counts(&counts, AlignmentType::Protein);

        assert_eq!(fraction, Some(2.0 / 3.0));
    }

    #[test]
    fn generic_constant_fraction_counts_n() {
        let counts = counts_for(b"NN-A");
        let fraction = max_counted_symbol_fraction_from_counts(&counts, AlignmentType::Generic);

        assert_eq!(fraction, Some(2.0 / 3.0));
    }

    #[test]
    fn constant_fraction_returns_none_when_all_symbols_are_ignored() {
        let counts = counts_for(b"-Nn");
        let fraction = max_counted_symbol_fraction_from_counts(&counts, AlignmentType::Dna);

        assert_eq!(fraction, None);
    }
}

#[cfg(test)]
mod tests {
    use crate::{Alignment, AlignmentType, ConsensusMethod, RawSequence};

    fn raw(id: &str, sequence: &[u8]) -> RawSequence {
        RawSequence {
            id: id.to_string(),
            sequence: sequence.to_vec(),
        }
    }

    #[test]
    fn column_summaries_range_returns_requested_positions() {
        let alignment =
            Alignment::new_with_type(vec![raw("s1", b"AC"), raw("s2", b"AT")], AlignmentType::Dna)
                .unwrap();

        assert_eq!(
            alignment
                .column_summaries_range(0..2, ConsensusMethod::MajorityNonGap)
                .unwrap()
                .into_iter()
                .map(|summary| summary.position)
                .collect::<Vec<_>>(),
            vec![0, 1]
        );
    }
}
