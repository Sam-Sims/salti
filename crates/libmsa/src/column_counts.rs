use strum::VariantArray;

use crate::{
    AlignmentType,
    residue::{self, Class},
};

/// The byte counts of one column of a [`Grid`](crate::Grid)
#[derive(Debug)]
pub struct ColumnCounts {
    /// How often each ASCII byte appears in the column
    counts: [u32; 128],
    /// The type of the grid the counts came from, which define what rules apply
    alignment_type: AlignmentType,
}

impl ColumnCounts {
    #[expect(clippy::large_types_passed_by_value)]
    pub(crate) const fn new(counts: [u32; 128], alignment_type: AlignmentType) -> Self {
        Self {
            counts,
            alignment_type,
        }
    }

    /// Returns the most common byte under `method`
    ///
    /// For DNA and protein the vote merges case first, and then the most common case wins.
    ///
    /// `None` when nothing is counted, such as an all-gap column under
    /// [`ConsensusMethod::MajorityNonGap`]
    pub fn consensus(&self, method: ConsensusMethod) -> Option<u8> {
        let folded = most_common((0..128).zip(self.counts_in(method.classes())))?;
        most_common(
            self.symbol_counts()
                .filter(|&(symbol, _)| residue::fold(symbol, self.alignment_type) == folded),
        )
    }

    /// Returns the Shannon entropy of the column's residues
    ///
    /// Gaps, unknown symbols and ambiguity codes are left out, DNA and protein ignore case.
    /// 0 when there are no residues
    pub fn entropy(&self) -> f64 {
        self.residue_frequencies()
            .fold(0.0, |entropy, (_, frequency)| {
                entropy - frequency * frequency.ln()
            })
    }

    /// Returns how conserved the column is, from 0 to 1
    ///
    /// One minus the residues' [entropy](Self::entropy) over the alphabet's maximum, times
    /// the fraction of residues and gaps that are residues. Unknown symbols and ambiguity codes
    /// are left out.
    ///
    /// 0 when there are no residues, `None` for generic grids.
    pub fn conservation(&self) -> Option<f64> {
        let max_entropy = f64::from(self.alignment_type.conservation_alphabet_size()?).ln();
        let conservation = (1.0 - self.entropy() / max_entropy).max(0.0);
        Some(conservation * self.fraction(&[Class::Residue], &[Class::Gap, Class::Residue]))
    }

    /// Returns the fraction of the column's cells that are gaps, unknown symbols or ambiguity
    /// codes
    ///
    /// 0 when the column is empty
    pub fn missing_fraction(&self) -> f64 {
        self.fraction(
            &[Class::Gap, Class::Unknown, Class::Ambiguous],
            Class::VARIANTS,
        )
    }

    /// Returns the fraction of the column's residues that aren't the same residue as
    /// `reference`
    ///
    /// Gaps, unknown symbols and ambiguity codes are left out, DNA and protein ignore case.
    /// 0 when there are no residues or `reference` isn't a residue
    pub fn differing_fraction(&self, reference: u8) -> f64 {
        if residue::classify(reference, self.alignment_type) != Class::Residue {
            return 0.0;
        }
        self.residue_frequencies()
            .filter(|&(symbol, _)| !residue::same_residue(symbol, reference, self.alignment_type))
            .fold(0.0, |differing, (_, frequency)| differing + frequency)
    }

    pub(crate) fn gap_fraction(&self) -> f64 {
        self.fraction(&[Class::Gap], Class::VARIANTS)
    }

    pub(crate) fn constant_fraction(&self) -> Option<f64> {
        self.residue_frequencies()
            .map(|(_, frequency)| frequency)
            .max_by(f64::total_cmp)
    }

    fn counts_in(&self, classes: &[Class]) -> [u32; 128] {
        let mut counts = [0u32; 128];
        for (symbol, count) in self.symbol_counts() {
            if classes.contains(&residue::classify(symbol, self.alignment_type)) {
                counts[usize::from(residue::fold(symbol, self.alignment_type))] += count;
            }
        }
        counts
    }

    fn fraction(&self, part: &[Class], whole: &[Class]) -> f64 {
        let whole: u32 = self.counts_in(whole).iter().sum();
        if whole == 0 {
            return 0.0;
        }
        f64::from(self.counts_in(part).iter().sum::<u32>()) / f64::from(whole)
    }

    fn residue_frequencies(&self) -> impl Iterator<Item = (u8, f64)> {
        let counts = self.counts_in(&[Class::Residue]);
        let total = f64::from(counts.iter().sum::<u32>());
        (0..128)
            .zip(counts)
            .filter(|&(_, count)| count != 0)
            .map(move |(residue, count)| (residue, f64::from(count) / total))
    }

    fn symbol_counts(&self) -> impl Iterator<Item = (u8, u32)> + '_ {
        (0..128)
            .zip(self.counts.iter().copied())
            .filter(|&(_, count)| count != 0)
    }
}

/// How the consensus byte of a column is chosen
///
/// # Ties
///
/// When several bytes share the highest count, the lowest byte wins. For DNA and protein this
/// applies twice: first between residues with case merged, then between the winner's two
/// cases.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Default,
    strum::Display,
    strum::EnumString,
    strum::VariantArray,
)]
#[strum(serialize_all = "kebab-case")]
pub enum ConsensusMethod {
    /// The most common byte, gaps included
    Majority,
    /// The most common byte that isn't a gap
    #[default]
    MajorityNonGap,
}

impl ConsensusMethod {
    const fn classes(self) -> &'static [Class] {
        match self {
            Self::Majority => Class::VARIANTS,
            Self::MajorityNonGap => &[Class::Unknown, Class::Ambiguous, Class::Residue],
        }
    }
}

fn most_common(counts: impl Iterator<Item = (u8, u32)>) -> Option<u8> {
    let mut max_count = 0;
    let mut most_common = None;
    for (symbol, count) in counts {
        if count > max_count {
            max_count = count;
            most_common = Some(symbol);
        }
    }
    most_common
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use rstest::rstest;

    use super::{ColumnCounts, ConsensusMethod};
    use crate::AlignmentType;

    fn column(alignment_type: AlignmentType, symbols: &[u8]) -> ColumnCounts {
        let mut counts = [0u32; 128];
        for &symbol in symbols {
            counts[usize::from(symbol)] += 1;
        }
        ColumnCounts::new(counts, alignment_type)
    }

    #[rstest]
    #[case::single(AlignmentType::Dna, ConsensusMethod::Majority, b"AAAA", b'A')]
    #[case::majority_counts_gaps(AlignmentType::Dna, ConsensusMethod::Majority, b"---AT", b'-')]
    #[case::non_gap_skips_gaps(
        AlignmentType::Dna,
        ConsensusMethod::MajorityNonGap,
        b"---AAT",
        b'A'
    )]
    #[case::higher_count_beats_lower_byte(
        AlignmentType::Dna,
        ConsensusMethod::Majority,
        b"ACCTTT",
        b'T'
    )]
    #[case::tie_picks_lowest_byte(AlignmentType::Dna, ConsensusMethod::Majority, b"TTCCAA", b'A')]
    #[case::tie_compares_folded_bytes(AlignmentType::Dna, ConsensusMethod::Majority, b"aaTT", b'a')]
    #[case::majority_tie_prefers_gap(AlignmentType::Dna, ConsensusMethod::Majority, b"--AA", b'-')]
    #[case::non_gap_tie_ignores_gap(
        AlignmentType::Dna,
        ConsensusMethod::MajorityNonGap,
        b"---GGTT",
        b'G'
    )]
    #[case::split_case_vote(AlignmentType::Dna, ConsensusMethod::Majority, b"AAaaGGG", b'A')]
    #[case::soft_masked(AlignmentType::Dna, ConsensusMethod::Majority, b"aaaA", b'a')]
    #[case::case_tie_picks_upper(AlignmentType::Dna, ConsensusMethod::Majority, b"AAaa", b'A')]
    #[case::protein_merges_case(AlignmentType::Protein, ConsensusMethod::Majority, b"mmK", b'm')]
    #[case::generic_keeps_case(AlignmentType::Generic, ConsensusMethod::Majority, b"AAaaGGG", b'G')]
    #[case::generic_tie_compares_bytes(
        AlignmentType::Generic,
        ConsensusMethod::Majority,
        b"aaTT",
        b'T'
    )]
    fn consensus_works(
        #[case] alignment_type: AlignmentType,
        #[case] method: ConsensusMethod,
        #[case] symbols: &[u8],
        #[case] expected: u8,
    ) {
        assert_eq!(
            column(alignment_type, symbols).consensus(method),
            Some(expected)
        );
    }

    #[rstest]
    #[case::empty_majority(ConsensusMethod::Majority, b"")]
    #[case::empty_non_gap(ConsensusMethod::MajorityNonGap, b"")]
    #[case::all_gaps_non_gap(ConsensusMethod::MajorityNonGap, b"---")]
    fn consensus_is_none(#[case] method: ConsensusMethod, #[case] symbols: &[u8]) {
        assert_eq!(column(AlignmentType::Dna, symbols).consensus(method), None);
    }

    #[rstest]
    #[case::constant(AlignmentType::Dna, b"AAAA", 0.0)]
    #[case::two_even(AlignmentType::Dna, b"AACC", 2f64.ln())]
    #[case::four_even(AlignmentType::Dna, b"ACGT", 4f64.ln())]
    #[case::leaves_out_gaps_and_no_information(AlignmentType::Dna, b"AA--NNRY", 0.0)]
    #[case::merges_case(AlignmentType::Dna, b"AaAa", 0.0)]
    #[case::empty(AlignmentType::Dna, b"", 0.0)]
    #[case::no_residues(AlignmentType::Dna, b"NN--", 0.0)]
    #[case::outside_alphabet(AlignmentType::Dna, b"????", 0.0)]
    #[case::protein(AlignmentType::Protein, b"MMKK", 2f64.ln())]
    #[case::generic_keeps_case(AlignmentType::Generic, b"Aa", 2f64.ln())]
    fn entropy_works(
        #[case] alignment_type: AlignmentType,
        #[case] symbols: &[u8],
        #[case] expected: f64,
    ) {
        // bits, so a -0.0 fails
        assert_eq!(
            column(alignment_type, symbols).entropy().to_bits(),
            expected.to_bits()
        );
    }

    #[rstest]
    #[case::fully_conserved(b"AAAA", 1.0)]
    #[case::gap_penalty(b"AA--", 0.5)]
    #[case::mixed(b"AACT", 0.25)]
    #[case::leaves_out_unknown(b"AAAN", 1.0)]
    #[case::leaves_out_ambiguous(b"AAAR", 1.0)]
    #[case::outside_alphabet(b"????", 1.0)]
    fn conservation_works(#[case] symbols: &[u8], #[case] expected: f64) {
        assert_eq!(
            column(AlignmentType::Dna, symbols).conservation(),
            Some(expected)
        );
    }

    #[rstest]
    #[case::empty(b"")]
    #[case::all_gaps(b"----")]
    #[case::max_entropy(b"ACGT")]
    #[case::clamped_above_max_entropy(b"ACGT?")]
    #[case::all_unknown(b"NNNN")]
    #[case::all_ambiguous(b"RY")]
    #[case::unknown_and_gaps(b"NN--")]
    fn conservation_is_zero(#[case] symbols: &[u8]) {
        assert_eq!(
            column(AlignmentType::Dna, symbols).conservation(),
            Some(0.0)
        );
    }

    #[test]
    fn conservation_is_none_for_generic() {
        assert_eq!(column(AlignmentType::Generic, b"AAAA").conservation(), None);
    }

    #[rstest]
    #[case::none_missing(AlignmentType::Dna, b"AAAA", 0.0)]
    #[case::gaps(AlignmentType::Dna, b"AA--", 0.5)]
    #[case::unknown(AlignmentType::Dna, b"AANn", 0.5)]
    #[case::ambiguous(AlignmentType::Dna, b"AARY", 0.5)]
    #[case::all_kinds(AlignmentType::Dna, b"-NRA", 0.75)]
    #[case::empty(AlignmentType::Dna, b"", 0.0)]
    #[case::protein_unknown(AlignmentType::Protein, b"MMXx", 0.5)]
    #[case::generic_counts_only_gaps(AlignmentType::Generic, b"NN-A", 0.25)]
    fn missing_fraction_works(
        #[case] alignment_type: AlignmentType,
        #[case] symbols: &[u8],
        #[case] expected: f64,
    ) {
        assert_eq!(column(alignment_type, symbols).missing_fraction(), expected);
    }

    #[rstest]
    #[case::all_same(AlignmentType::Dna, b"AAAA", b'A', 0.0)]
    #[case::half(AlignmentType::Dna, b"AACC", b'A', 0.5)]
    #[case::all_differ(AlignmentType::Dna, b"CCGG", b'A', 1.0)]
    #[case::merges_case(AlignmentType::Dna, b"aaCC", b'A', 0.5)]
    #[case::lower_case_reference(AlignmentType::Dna, b"AACC", b'a', 0.5)]
    #[case::leaves_out_gaps_and_no_information(AlignmentType::Dna, b"AC--NR", b'A', 0.5)]
    #[case::reference_unknown(AlignmentType::Dna, b"ACGT", b'N', 0.0)]
    #[case::reference_ambiguous(AlignmentType::Dna, b"ACGT", b'R', 0.0)]
    #[case::reference_gap(AlignmentType::Dna, b"ACGT", b'-', 0.0)]
    #[case::no_residues(AlignmentType::Dna, b"NN--", b'A', 0.0)]
    #[case::generic_keeps_case(AlignmentType::Generic, b"aaAA", b'A', 0.5)]
    fn differing_fraction_works(
        #[case] alignment_type: AlignmentType,
        #[case] symbols: &[u8],
        #[case] reference: u8,
        #[case] expected: f64,
    ) {
        assert_eq!(
            column(alignment_type, symbols).differing_fraction(reference),
            expected
        );
    }

    #[rstest]
    #[case::no_gaps(b"AAAA", 0.0)]
    #[case::half_gaps(b"AA--", 0.5)]
    #[case::all_gaps(b"----", 1.0)]
    #[case::empty(b"", 0.0)]
    fn gap_fraction_works(#[case] symbols: &[u8], #[case] expected: f64) {
        assert_eq!(column(AlignmentType::Dna, symbols).gap_fraction(), expected);
    }

    #[rstest]
    #[case::dna_ignores_gaps_and_n(AlignmentType::Dna, b"AANn--T", 2.0 / 3.0)]
    #[case::protein_ignores_gaps_and_x(AlignmentType::Protein, b"MMXx--K", 2.0 / 3.0)]
    #[case::generic_counts_n(AlignmentType::Generic, b"NN-A", 2.0 / 3.0)]
    #[case::constant(AlignmentType::Dna, b"AAAA", 1.0)]
    #[case::merges_case(AlignmentType::Dna, b"AaAa", 1.0)]
    #[case::leaves_out_ambiguous(AlignmentType::Dna, b"CYCC", 1.0)]
    #[case::generic_keeps_case(AlignmentType::Generic, b"Aa", 0.5)]
    fn constant_fraction_works(
        #[case] alignment_type: AlignmentType,
        #[case] symbols: &[u8],
        #[case] expected: f64,
    ) {
        assert_eq!(
            column(alignment_type, symbols).constant_fraction(),
            Some(expected)
        );
    }

    #[rstest]
    #[case::empty(b"")]
    #[case::only_ignored(b"-Nn")]
    #[case::only_ambiguous(b"RY")]
    fn constant_fraction_is_none(#[case] symbols: &[u8]) {
        assert_eq!(
            column(AlignmentType::Dna, symbols).constant_fraction(),
            None
        );
    }
}
