use rand::seq::IndexedRandom;
use std::{num::NonZeroU8, ops::Range};

use crate::AlignmentType;
use crate::data::AlignmentData;
use crate::error::AlignmentError;
use crate::model::Alignment;
use crate::projection::Projection;
use crate::translation::{ReadingFrame, TranslationTable, translated_byte_at};

/// Calculated values for a single alignment column.
#[derive(Debug, Clone, PartialEq)]
pub struct ColumnSummary {
    pub position: usize,
    pub consensus: Option<u8>,
    pub conservation: Option<f32>,
}

/// Selects how consensus bytes are chosen for alignment columns.
///
/// Different methods vary in whether gap characters are considered when
/// determining the representative byte for a column. Tied winning symbols are
/// resolved randomly.
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
        if range.is_empty() {
            return Err(AlignmentError::EmptyRange);
        }

        let columns = counted_columns_range(&self.data, &self.rows, &self.columns, range)?;
        let mut rng = rand::rng();
        Ok(summaries_from_columns(
            &columns,
            method,
            self.active_type().conservation_alphabet_size(),
            &mut rng,
        ))
    }
}

pub(crate) struct CountedColumn {
    pub position: usize,
    pub counts: [u32; 256],
}

pub(crate) fn counted_columns_range(
    data: &AlignmentData,
    rows: &Projection,
    columns: &Projection,
    range: Range<usize>,
) -> Result<Vec<CountedColumn>, AlignmentError> {
    if range.is_empty() {
        return Err(AlignmentError::EmptyRange);
    }

    if range.end > columns.len() {
        return Err(AlignmentError::ColumnOutOfBounds {
            index: range.end - 1,
            length: columns.len(),
        });
    }

    Ok(range
        .map(|rel_col| CountedColumn {
            position: rel_col,
            counts: column_byte_counts(
                data,
                rows,
                columns
                    .absolute(rel_col)
                    .expect("validated range positions map into the projection"),
            ),
        })
        .collect())
}

pub(crate) fn counted_translated_columns_range(
    data: &AlignmentData,
    rows: &Projection,
    range: Range<usize>,
    frame: ReadingFrame,
    table: &TranslationTable,
) -> Result<Vec<CountedColumn>, AlignmentError> {
    if range.is_empty() {
        return Err(AlignmentError::EmptyRange);
    }

    let translated_len = frame.translated_length(data.length);
    if range.end > translated_len {
        return Err(AlignmentError::ColumnOutOfBounds {
            index: range.end - 1,
            length: translated_len,
        });
    }

    Ok(range
        .map(|protein_col| CountedColumn {
            position: protein_col,
            counts: translated_column_byte_counts(data, rows, protein_col, frame, table),
        })
        .collect())
}

#[inline]
const fn is_gap_byte(byte: u8) -> bool {
    matches!(byte, b'-')
}

pub(crate) fn summaries_from_columns(
    columns: &[CountedColumn],
    method: ConsensusMethod,
    alphabet_size: Option<NonZeroU8>,
    rng: &mut impl rand::Rng,
) -> Vec<ColumnSummary> {
    let max_entropy = alphabet_size.map(|value| f64::from(value.get()).log2());

    columns
        .iter()
        .map(|column| ColumnSummary {
            position: column.position,
            consensus: consensus_from_counts(&column.counts, method, rng),
            conservation: max_entropy
                .map(|max_entropy| conservation_from_counts(&column.counts, max_entropy)),
        })
        .collect()
}

pub(crate) fn gap_fraction_from_counts(counts: &[u32; 256]) -> f32 {
    let (gap_count, total) = counts
        .iter()
        .enumerate()
        .filter(|&(_, &count)| count != 0)
        .fold((0u32, 0u32), |(gap_count, total), (symbol, &count)| {
            let gap_count = if is_gap_byte(symbol as u8) {
                gap_count + count
            } else {
                gap_count
            };

            (gap_count, total + count)
        });

    if total == 0 {
        0.0
    } else {
        gap_count as f32 / total as f32
    }
}

pub(crate) fn max_counted_symbol_fraction_from_counts(
    counts: &[u32; 256],
    kind: AlignmentType,
) -> Option<f32> {
    let mut counted_total = 0u32;
    let mut max_count = 0u32;

    for (symbol, &count) in counts.iter().enumerate() {
        if count == 0 {
            continue;
        }

        if is_ignored_constant_symbol(symbol as u8, kind) {
            continue;
        }

        counted_total += count;
        max_count = max_count.max(count);
    }

    (counted_total != 0).then_some(max_count as f32 / counted_total as f32)
}

fn consensus_from_counts(
    counts: &[u32; 256],
    method: ConsensusMethod,
    rng: &mut impl rand::Rng,
) -> Option<u8> {
    let exclude_gap = matches!(method, ConsensusMethod::MajorityNonGap);
    let mut max_count = 0u32;
    let mut candidates = [0u8; 256];
    let mut candidate_count = 0usize;

    for (index, &count) in counts.iter().enumerate() {
        if count == 0 {
            continue;
        }
        if exclude_gap && is_gap_byte(index as u8) {
            continue;
        }

        if count > max_count {
            max_count = count;
            candidate_count = 0;
            candidates[candidate_count] = index as u8;
            candidate_count += 1;
        } else if count == max_count {
            candidates[candidate_count] = index as u8;
            candidate_count += 1;
        }
    }

    candidates[..candidate_count].choose(rng).copied()
}

fn conservation_from_counts(counts: &[u32; 256], max_entropy: f64) -> f32 {
    let mut total = 0u32;
    let mut gap_count = 0u32;
    let mut merged_non_gap_counts = [0u32; 256];

    for (symbol, &count) in counts.iter().enumerate() {
        if count == 0 {
            continue;
        }
        total += count;

        if is_gap_byte(symbol as u8) {
            gap_count += count;
            continue;
        }

        let upper = usize::from((symbol as u8).to_ascii_uppercase());
        merged_non_gap_counts[upper] += count;
    }

    if total == 0 {
        return 0.0;
    }

    let non_gap_total = total.saturating_sub(gap_count);
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

#[inline]
const fn is_ignored_constant_symbol(byte: u8, kind: AlignmentType) -> bool {
    if is_gap_byte(byte) {
        return true;
    }

    match kind {
        AlignmentType::Dna => matches!(byte, b'N' | b'n'),
        AlignmentType::Protein => matches!(byte, b'X' | b'x'),
        AlignmentType::Generic => false,
    }
}

fn column_byte_counts(data: &AlignmentData, rows: &Projection, abs_col: usize) -> [u32; 256] {
    let mut counts = [0u32; 256];

    for abs_row in rows.iter() {
        let sequence = data
            .sequences
            .get(abs_row)
            .expect("selected row must exist");
        counts[usize::from(sequence.sequence[abs_col])] += 1;
    }

    counts
}

fn translated_column_byte_counts(
    data: &AlignmentData,
    rows: &Projection,
    protein_col: usize,
    frame: ReadingFrame,
    table: &TranslationTable,
) -> [u32; 256] {
    let mut counts = [0u32; 256];

    for abs_row in rows.iter() {
        let sequence = data
            .sequences
            .get(abs_row)
            .expect("selected row must exist");
        let byte = translated_byte_at(&sequence.sequence, protein_col, frame, table)
            .expect("validated translated range");
        counts[usize::from(byte)] += 1;
    }

    counts
}

#[cfg(test)]
mod consensus_count_tests {
    use rand::{SeedableRng, rngs::StdRng};

    use super::{ConsensusMethod, consensus_from_counts};

    fn counts_for(symbols: &[u8]) -> [u32; 256] {
        let mut counts = [0u32; 256];
        for &s in symbols {
            counts[usize::from(s)] += 1;
        }
        counts
    }

    #[test]
    fn consensus_same() {
        let counts = counts_for(b"AAAA");
        let mut rng = rand::rng();
        assert_eq!(
            consensus_from_counts(&counts, ConsensusMethod::Majority, &mut rng),
            Some(b'A')
        );
    }

    #[test]
    fn consensus_majority_gap() {
        let counts = counts_for(b"---AT");
        let mut rng = rand::rng();
        assert_eq!(
            consensus_from_counts(&counts, ConsensusMethod::Majority, &mut rng),
            Some(b'-')
        );
    }

    #[test]
    fn consensus_majority_nongap_excludes_gaps() {
        let counts = counts_for(b"---AAT");
        let mut rng = rand::rng();
        assert_eq!(
            consensus_from_counts(&counts, ConsensusMethod::MajorityNonGap, &mut rng),
            Some(b'A')
        );
    }

    #[test]
    fn consensus_no_candidates_returns_none() {
        let counts = [0u32; 256];
        let mut rng = rand::rng();
        assert_eq!(
            consensus_from_counts(&counts, ConsensusMethod::Majority, &mut rng),
            None
        );
    }

    #[test]
    fn consensus_tie_breaking_is_seeded() {
        let counts = counts_for(b"ACACACTT");
        let mut rng = StdRng::seed_from_u64(5);
        let result = consensus_from_counts(&counts, ConsensusMethod::Majority, &mut rng);
        assert!(matches!(result, Some(b'A') | Some(b'C')));
    }
}

#[cfg(test)]
mod derived_column_tests {
    use std::num::NonZeroU8;

    use rand::{SeedableRng, rngs::StdRng};

    use super::{ConsensusMethod, CountedColumn, summaries_from_columns};

    fn counted_column(position: usize, symbols: &[u8]) -> CountedColumn {
        let mut counts = [0u32; 256];
        for &symbol in symbols {
            counts[usize::from(symbol)] += 1;
        }

        CountedColumn { position, counts }
    }

    #[test]
    fn summaries_from_columns_return_none_for_all_gap_column() {
        let columns = vec![counted_column(3, b"---")];
        let mut rng = StdRng::seed_from_u64(8);
        let summaries = summaries_from_columns(
            &columns,
            ConsensusMethod::MajorityNonGap,
            Some(NonZeroU8::new(4).unwrap()),
            &mut rng,
        );

        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].position, 3);
        assert_eq!(summaries[0].consensus, None);
        assert_eq!(summaries[0].conservation, Some(0.0));
    }

    #[test]
    fn summaries_from_columns_report_conservation_extremes() {
        let columns = vec![counted_column(0, b"AAAA"), counted_column(1, b"----")];
        let mut rng = StdRng::seed_from_u64(9);
        let summaries = summaries_from_columns(
            &columns,
            ConsensusMethod::MajorityNonGap,
            Some(NonZeroU8::new(4).unwrap()),
            &mut rng,
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

    fn counts_for(symbols: &[u8]) -> [u32; 256] {
        let mut counts = [0u32; 256];
        for &s in symbols {
            counts[usize::from(s)] += 1;
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
        let counts = [0u32; 256];
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
    use crate::AlignmentType;

    use super::max_counted_symbol_fraction_from_counts;

    fn counts_for(symbols: &[u8]) -> [u32; 256] {
        let mut counts = [0u32; 256];
        for &s in symbols {
            counts[usize::from(s)] += 1;
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
