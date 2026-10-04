use rayon::prelude::*;

use crate::{
    AlignmentType, Grid,
    counts::{add_counts, count_blocks},
    residue,
};

impl Grid<'_> {
    /// Returns a summary for each id in `cols`.
    ///
    /// Ids may be in any order and may repeat. With no `rows`, every summary
    /// has no consensus and zero conservation.
    pub fn summaries(
        self,
        rows: &[usize],
        cols: &[usize],
        method: ConsensusMethod,
    ) -> Vec<ColumnSummary> {
        let max_entropy = self
            .alignment_type()
            .conservation_alphabet_size()
            .map(|size| f64::from(size).log2());
        count_blocks(self, rows, cols)
            .flat_map_iter(|(_, counts)| counts)
            .map(|counts| ColumnSummary {
                consensus: consensus_from_counts(&counts, method),
                conservation: max_entropy
                    .map(|max_entropy| conservation_from_counts(&counts, max_entropy)),
            })
            .collect()
    }

    pub fn consensus(self, rows: &[usize], cols: &[usize], method: ConsensusMethod) -> Option<u8> {
        let pooled = count_blocks(self, rows, cols)
            .flat_map_iter(|(_, counts)| counts)
            .reduce(
                || [0; 128],
                |mut pooled, counts| {
                    add_counts(&mut pooled, &counts);
                    pooled
                },
            );
        consensus_from_counts(&pooled, method)
    }
}

/// Calculated values for a single alignment column.
#[derive(Debug, Clone, PartialEq)]
pub struct ColumnSummary {
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
        if count == 0 || residue::is_gap(symbol) || residue::is_unknown(symbol, kind) {
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

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use rstest::rstest;

    use super::{
        ConsensusMethod, consensus_from_counts, conservation_from_counts, gap_fraction_from_counts,
        max_counted_symbol_fraction_from_counts,
    };
    use crate::{Alignment, AlignmentType, RawSequence, ReadingFrame};

    const DNA_MAX_ENTROPY: f64 = 2.0;

    fn counts_for(symbols: &[u8]) -> [u32; 128] {
        let mut counts = [0u32; 128];
        for &symbol in symbols {
            counts[usize::from(symbol)] += 1;
        }
        counts
    }

    fn dna(sequences: &[&[u8]]) -> Alignment {
        Alignment::new_with_type(
            sequences
                .iter()
                .enumerate()
                .map(|(i, sequence)| RawSequence {
                    id: format!("s{i}"),
                    sequence: sequence.to_vec(),
                }),
            AlignmentType::Dna,
        )
        .unwrap()
    }

    fn sample() -> Alignment {
        dna(&[b"ACGT", b"ACGA", b"TTTA", b"--T-"])
    }

    #[rstest]
    #[case::all(&[0, 1, 2], &[0, 1, 2, 3], b"ACGA")]
    #[case::subset_rows(&[2], &[0, 1, 2, 3], b"TTTA")]
    #[case::cols_unsorted_repeated(&[0, 1, 2], &[3, 0, 3], b"AAA")]
    #[case::rows_repeated(&[2, 2, 0], &[0], b"T")]
    fn summaries_works(#[case] rows: &[usize], #[case] cols: &[usize], #[case] expected: &[u8]) {
        let alignment = sample();
        let consensus: Vec<_> = alignment
            .grid(AlignmentType::Dna)
            .summaries(rows, cols, ConsensusMethod::MajorityNonGap)
            .into_iter()
            .map(|summary| summary.consensus)
            .collect();
        let expected: Vec<_> = expected.iter().copied().map(Some).collect();
        assert_eq!(consensus, expected);
    }

    #[test]
    fn summaries_is_empty() {
        let alignment = sample();
        let summaries =
            alignment
                .grid(AlignmentType::Dna)
                .summaries(&[0, 1], &[], ConsensusMethod::Majority);
        assert!(summaries.is_empty());
    }

    #[test]
    fn summaries_without_rows_has_no_consensus() {
        let alignment = sample();
        let summaries =
            alignment
                .grid(AlignmentType::Dna)
                .summaries(&[], &[0, 1], ConsensusMethod::Majority);
        for summary in summaries {
            assert_eq!(summary.consensus, None);
            assert_eq!(summary.conservation, Some(0.0));
        }
    }

    #[rstest]
    #[case::dna(AlignmentType::Dna, Some(1.0))]
    #[case::generic(AlignmentType::Generic, None)]
    fn summaries_uses_grid_alignment_type(
        #[case] alignment_type: AlignmentType,
        #[case] expected: Option<f32>,
    ) {
        let alignment = dna(&[b"A", b"A"]);
        let summaries =
            alignment
                .grid(alignment_type)
                .summaries(&[0, 1], &[0], ConsensusMethod::Majority);
        assert_eq!(summaries[0].conservation, expected);
    }

    #[test]
    fn summaries_keeps_cols_order_across_blocks() {
        let row: Vec<u8> = b"ACGT".iter().copied().cycle().take(300).collect();
        let alignment = dna(&[&row]);
        let cols: Vec<usize> = (0..300).rev().collect();

        let consensus: Vec<_> = alignment
            .grid(AlignmentType::Dna)
            .summaries(&[0], &cols, ConsensusMethod::Majority)
            .into_iter()
            .map(|summary| summary.consensus)
            .collect();
        let expected: Vec<_> = cols.iter().map(|&col| Some(row[col])).collect();
        assert_eq!(consensus, expected);
    }

    #[test]
    fn summaries_reads_translated_grid() {
        let alignment = dna(&[b"ATGCCC"]);
        let summaries = alignment.translated_grid(ReadingFrame::Frame1).summaries(
            &[0],
            &[1, 0],
            ConsensusMethod::Majority,
        );
        let consensus: Vec<_> = summaries.iter().map(|summary| summary.consensus).collect();
        assert_eq!(consensus, [Some(b'P'), Some(b'M')]);
    }

    #[rstest]
    #[case::subset(&[2], &[0, 1, 2], ConsensusMethod::Majority, b'T')]
    #[case::pools_rows_and_cols(&[0, 1], &[0, 1, 2, 3], ConsensusMethod::Majority, b'A')]
    #[case::cols_repeated(&[0], &[1, 1, 0], ConsensusMethod::Majority, b'C')]
    #[case::majority(&[3], &[0, 1, 2, 3], ConsensusMethod::Majority, b'-')]
    #[case::majority_non_gap(&[3], &[0, 1, 2, 3], ConsensusMethod::MajorityNonGap, b'T')]
    fn consensus_works(
        #[case] rows: &[usize],
        #[case] cols: &[usize],
        #[case] method: ConsensusMethod,
        #[case] expected: u8,
    ) {
        let alignment = sample();
        assert_eq!(
            alignment
                .grid(AlignmentType::Dna)
                .consensus(rows, cols, method),
            Some(expected)
        );
    }

    #[test]
    fn consensus_pools_across_blocks() {
        let row = [vec![b'A'; 128], vec![b'C'; 129]].concat();
        let alignment = dna(&[&row]);
        let cols: Vec<usize> = (0..row.len()).collect();
        assert_eq!(
            alignment
                .grid(AlignmentType::Dna)
                .consensus(&[0], &cols, ConsensusMethod::Majority),
            Some(b'C')
        );
    }

    #[rstest]
    #[case::no_rows(&[], &[0, 1])]
    #[case::no_cols(&[0, 1], &[])]
    fn consensus_is_none(#[case] rows: &[usize], #[case] cols: &[usize]) {
        let alignment = sample();
        assert_eq!(
            alignment
                .grid(AlignmentType::Dna)
                .consensus(rows, cols, ConsensusMethod::Majority),
            None
        );
    }

    #[rstest]
    fn consensus_method_from_str_agrees_with_name(
        #[values(ConsensusMethod::Majority, ConsensusMethod::MajorityNonGap)]
        method: ConsensusMethod,
    ) {
        assert_eq!(method.name().parse(), Ok(method));
    }

    #[test]
    fn consensus_method_from_str_rejects_unknown() {
        assert_eq!("majority-gap".parse::<ConsensusMethod>(), Err(()));
    }

    #[rstest]
    #[case::single(ConsensusMethod::Majority, b"AAAA", b'A')]
    #[case::majority_counts_gaps(ConsensusMethod::Majority, b"---AT", b'-')]
    #[case::non_gap_skips_gaps(ConsensusMethod::MajorityNonGap, b"---AAT", b'A')]
    #[case::higher_count_beats_lower_byte(ConsensusMethod::Majority, b"ACCTTT", b'T')]
    #[case::tie_picks_lowest_byte(ConsensusMethod::Majority, b"TTCCAA", b'A')]
    #[case::tie_uppercase_beats_lowercase(ConsensusMethod::Majority, b"aaTT", b'T')]
    #[case::majority_tie_prefers_gap(ConsensusMethod::Majority, b"--AA", b'-')]
    #[case::non_gap_tie_ignores_gap(ConsensusMethod::MajorityNonGap, b"---GGTT", b'G')]
    fn consensus_from_counts_works(
        #[case] method: ConsensusMethod,
        #[case] symbols: &[u8],
        #[case] expected: u8,
    ) {
        assert_eq!(
            consensus_from_counts(&counts_for(symbols), method),
            Some(expected)
        );
    }

    #[rstest]
    #[case::empty_majority(ConsensusMethod::Majority, b"")]
    #[case::empty_non_gap(ConsensusMethod::MajorityNonGap, b"")]
    #[case::all_gaps_non_gap(ConsensusMethod::MajorityNonGap, b"---")]
    fn consensus_from_counts_is_none(#[case] method: ConsensusMethod, #[case] symbols: &[u8]) {
        assert_eq!(consensus_from_counts(&counts_for(symbols), method), None);
    }

    #[rstest]
    #[case::fully_conserved(b"AAAA", 1.0)]
    #[case::case_insensitive(b"AaAa", 1.0)]
    #[case::gap_penalty(b"AA--", 0.5)]
    #[case::mixed(b"AACT", 0.25)]
    fn conservation_from_counts_works(#[case] symbols: &[u8], #[case] expected: f32) {
        assert_eq!(
            conservation_from_counts(&counts_for(symbols), DNA_MAX_ENTROPY),
            expected
        );
    }

    #[rstest]
    #[case::empty(b"")]
    #[case::all_gaps(b"----")]
    #[case::max_entropy(b"ACGT")]
    #[case::clamped_above_max_entropy(b"ACGTN")]
    fn conservation_from_counts_is_zero(#[case] symbols: &[u8]) {
        assert_eq!(
            conservation_from_counts(&counts_for(symbols), DNA_MAX_ENTROPY),
            0.0
        );
    }

    #[rstest]
    #[case::no_gaps(b"AAAA", 0.0)]
    #[case::half_gaps(b"AA--", 0.5)]
    #[case::all_gaps(b"----", 1.0)]
    #[case::empty(b"", 0.0)]
    fn gap_fraction_from_counts_works(#[case] symbols: &[u8], #[case] expected: f32) {
        assert_eq!(gap_fraction_from_counts(&counts_for(symbols)), expected);
    }

    #[rstest]
    #[case::dna_ignores_gaps_and_n(AlignmentType::Dna, b"AANn--T", 2.0 / 3.0)]
    #[case::protein_ignores_gaps_and_x(AlignmentType::Protein, b"MMXx--K", 2.0 / 3.0)]
    #[case::generic_counts_n(AlignmentType::Generic, b"NN-A", 2.0 / 3.0)]
    #[case::constant(AlignmentType::Dna, b"AAAA", 1.0)]
    fn max_counted_symbol_fraction_from_counts_works(
        #[case] alignment_type: AlignmentType,
        #[case] symbols: &[u8],
        #[case] expected: f32,
    ) {
        assert_eq!(
            max_counted_symbol_fraction_from_counts(&counts_for(symbols), alignment_type),
            Some(expected)
        );
    }

    #[rstest]
    #[case::empty(AlignmentType::Dna, b"")]
    #[case::only_ignored(AlignmentType::Dna, b"-Nn")]
    fn max_counted_symbol_fraction_from_counts_is_none(
        #[case] alignment_type: AlignmentType,
        #[case] symbols: &[u8],
    ) {
        assert_eq!(
            max_counted_symbol_fraction_from_counts(&counts_for(symbols), alignment_type),
            None
        );
    }
}
