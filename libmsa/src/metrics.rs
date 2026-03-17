use std::num::NonZeroU8;
use std::ops::Range;

use rand::seq::IndexedRandom;

use crate::data::AlignmentData;
use crate::error::AlignmentError;
use crate::model::Alignment;
use crate::projection::Projection;
use crate::translation::{ReadingFrame, TranslationTable, translated_byte_at, translated_length};

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

/// Calculated values for a single alignment column.
#[derive(Debug, Clone, PartialEq)]
pub struct ColumnSummary {
    pub position: usize,
    pub consensus: Option<u8>,
    pub conservation: Option<f32>,
    pub gap_fraction: f32,
}

pub(crate) struct CountedColumn {
    pub position: usize,
    pub counts: [u32; 256],
}

// alignment metrics
impl Alignment {
    /// Returns the consensus byte for each requested relative column.
    ///
    /// Each position is resolved against the alignment's current column projection.
    /// The returned vector keeps the requested relative positions and contains one
    /// consensus byte for each visible column named in `positions`.
    ///
    /// # Errors
    ///
    /// [`AlignmentError::ColumnOutOfBounds`] if any value in `positions` is not a
    /// valid index in the current column projection.
    pub fn consensus_positions(
        &self,
        positions: &[usize],
        method: ConsensusMethod,
    ) -> Result<Vec<(usize, Option<u8>)>, AlignmentError> {
        let columns = counted_columns_positions(&self.data, &self.rows, &self.columns, positions)?;
        let mut rng = rand::rng();
        Ok(consensus_from_columns(&columns, method, &mut rng))
    }

    /// Returns the conservation score for each requested relative column.
    ///
    /// Each position is resolved against the alignment's current column projection.
    /// The returned vector keeps the requested relative positions and contains one
    /// conservation score for each visible column named in `positions`.
    ///
    /// # Errors
    ///
    /// [`AlignmentError::ColumnOutOfBounds`] if any value in `positions` is not a
    /// valid index in the current column projection.
    ///
    /// [`AlignmentError::ConservationUndefined`] if the active alignment kind does not
    /// define a conservation alphabet size.
    pub fn conservation_positions(
        &self,
        positions: &[usize],
    ) -> Result<Vec<(usize, f32)>, AlignmentError> {
        let columns = counted_columns_positions(&self.data, &self.rows, &self.columns, positions)?;
        conservation_from_columns(&columns, self.active_type().conservation_alphabet_size())
    }

    /// Returns the gap fraction for each requested relative column.
    ///
    /// Each position is resolved against the alignment's current column projection.
    /// The returned vector keeps the requested relative positions and contains one
    /// gap fraction for each visible column named in `positions`.
    ///
    /// # Errors
    ///
    /// [`AlignmentError::ColumnOutOfBounds`] if any value in `positions` is not a
    /// valid index in the current column projection.
    pub fn gap_fraction_positions(
        &self,
        positions: &[usize],
    ) -> Result<Vec<(usize, f32)>, AlignmentError> {
        let columns = counted_columns_positions(&self.data, &self.rows, &self.columns, positions)?;
        Ok(gap_fraction_from_columns(&columns))
    }

    /// Returns a derived summary for each requested relative column.
    ///
    /// Each position is resolved against the alignment's current column projection.
    /// The returned vector keeps the requested relative positions and contains a
    /// [`ColumnSummary`] with consensus, gap fraction, and conservation when that
    /// measure is defined for the active alignment kind.
    ///
    /// # Errors
    ///
    /// [`AlignmentError::ColumnOutOfBounds`] if any value in `positions` is not a
    /// valid index in the current column projection.
    pub fn column_summaries_positions(
        &self,
        positions: &[usize],
        method: ConsensusMethod,
    ) -> Result<Vec<ColumnSummary>, AlignmentError> {
        let columns = counted_columns_positions(&self.data, &self.rows, &self.columns, positions)?;
        let mut rng = rand::rng();
        Ok(summaries_from_columns(
            &columns,
            method,
            self.active_type().conservation_alphabet_size(),
            &mut rng,
        ))
    }
}

pub(crate) fn counted_columns_positions(
    data: &AlignmentData,
    rows: &Projection,
    columns: &Projection,
    relative_positions: &[usize],
) -> Result<Vec<CountedColumn>, AlignmentError> {
    relative_positions
        .iter()
        .copied()
        .map(|rel_col| {
            let abs_col = columns
                .absolute(rel_col)
                .ok_or(AlignmentError::ColumnOutOfBounds {
                    index: rel_col,
                    length: columns.len(),
                })?;

            Ok(CountedColumn {
                position: rel_col,
                counts: column_byte_counts(data, rows, abs_col),
            })
        })
        .collect()
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

    let translated_len = translated_length(data.length, frame);
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

pub(crate) fn consensus_from_columns(
    columns: &[CountedColumn],
    method: ConsensusMethod,
    rng: &mut impl rand::Rng,
) -> Vec<(usize, Option<u8>)> {
    columns
        .iter()
        .map(|column| {
            (
                column.position,
                consensus_from_counts(&column.counts, method, rng),
            )
        })
        .collect()
}

pub(crate) fn conservation_from_columns(
    columns: &[CountedColumn],
    alphabet_size: Option<NonZeroU8>,
) -> Result<Vec<(usize, f32)>, AlignmentError> {
    let max_entropy = alphabet_size
        .map(|value| f64::from(value.get()).log2())
        .ok_or(AlignmentError::ConservationUndefined)?;

    Ok(columns
        .iter()
        .map(|column| {
            (
                column.position,
                conservation_from_counts(&column.counts, max_entropy),
            )
        })
        .collect())
}

pub(crate) fn gap_fraction_from_columns(columns: &[CountedColumn]) -> Vec<(usize, f32)> {
    columns
        .iter()
        .map(|column| (column.position, gap_fraction_from_counts(&column.counts)))
        .collect()
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
            gap_fraction: gap_fraction_from_counts(&column.counts),
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

    let gap_fraction = f64::from(gap_fraction_from_counts(counts));
    let conservation = (1.0 - entropy / max_entropy).max(0.0);
    (conservation * (1.0 - gap_fraction)) as f32
}

fn column_byte_counts(data: &AlignmentData, rows: &Projection, abs_col: usize) -> [u32; 256] {
    let mut counts = [0u32; 256];

    for abs_row in rows.iter() {
        let sequence = data
            .sequences
            .get(abs_row)
            .expect("selected row must exist");
        counts[usize::from(sequence.sequence()[abs_col])] += 1;
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
        let byte = translated_byte_at(sequence.sequence(), protein_col, frame, table)
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

    use super::{ConsensusMethod, CountedColumn, consensus_from_columns, summaries_from_columns};

    fn counted_column(position: usize, symbols: &[u8]) -> CountedColumn {
        let mut counts = [0u32; 256];
        for &symbol in symbols {
            counts[usize::from(symbol)] += 1;
        }

        CountedColumn { position, counts }
    }

    #[test]
    fn consensus_from_columns_returns_known_consensus() {
        let columns = vec![counted_column(2, b"AAAA"), counted_column(4, b"CCCC")];
        let mut rng = StdRng::seed_from_u64(6);

        assert_eq!(
            consensus_from_columns(&columns, ConsensusMethod::MajorityNonGap, &mut rng),
            vec![(2, Some(b'A')), (4, Some(b'C'))]
        );
    }

    #[test]
    fn consensus_from_columns_respects_gap_handling() {
        let columns = vec![counted_column(1, b"--A")];
        let mut majority_rng = StdRng::seed_from_u64(7);
        let mut nongap_rng = StdRng::seed_from_u64(7);

        assert_eq!(
            consensus_from_columns(&columns, ConsensusMethod::Majority, &mut majority_rng),
            vec![(1, Some(b'-'))]
        );
        assert_eq!(
            consensus_from_columns(&columns, ConsensusMethod::MajorityNonGap, &mut nongap_rng,),
            vec![(1, Some(b'A'))]
        );
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
        assert_eq!(summaries[0].gap_fraction, 1.0);
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
        assert_eq!(summaries[0].gap_fraction, 0.0);
        assert_eq!(summaries[1].position, 1);
        assert_eq!(summaries[1].consensus, None);
        assert_eq!(summaries[1].conservation, Some(0.0));
        assert_eq!(summaries[1].gap_fraction, 1.0);
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
mod tests {
    use crate::{Alignment, AlignmentError, AlignmentType, ConsensusMethod, RawSequence};

    fn raw(id: &str, sequence: &[u8]) -> RawSequence {
        RawSequence {
            id: id.to_string(),
            sequence: sequence.to_vec(),
        }
    }

    #[test]
    fn consensus_positions_returns_single_column() {
        let alignment = Alignment::new_with_type(
            vec![raw("s1", b"ACGT"), raw("s2", b"ACGT")],
            AlignmentType::Dna,
        )
        .unwrap();

        assert_eq!(
            alignment
                .consensus_positions(&[1], ConsensusMethod::MajorityNonGap)
                .unwrap(),
            vec![(1, Some(b'C'))]
        );
    }

    #[test]
    fn consensus_positions_returns_correct_positions() {
        let alignment = Alignment::new_with_type(
            vec![raw("s1", b"ACGT"), raw("s2", b"ACGT")],
            AlignmentType::Dna,
        )
        .unwrap();

        assert_eq!(
            alignment
                .consensus_positions(&[1, 2], ConsensusMethod::MajorityNonGap)
                .unwrap(),
            vec![(1, Some(b'C')), (2, Some(b'G'))]
        );
    }

    #[test]
    fn translated_consensus_range_returns_protein_positions() {
        let alignment = Alignment::new_with_type(
            vec![raw("s1", b"ATGAAA"), raw("s2", b"ATGAAG")],
            AlignmentType::Dna,
        )
        .unwrap();

        assert_eq!(
            alignment
                .translated(crate::ReadingFrame::Frame1)
                .unwrap()
                .consensus_range(0..2, ConsensusMethod::MajorityNonGap)
                .unwrap(),
            vec![(0, Some(b'M')), (1, Some(b'K'))]
        );
    }

    #[test]
    fn conservation_positions_returns_score() {
        let alignment =
            Alignment::new_with_type(vec![raw("s1", b"A"), raw("s2", b"A")], AlignmentType::Dna)
                .unwrap();

        assert_eq!(
            alignment.conservation_positions(&[0]).unwrap(),
            vec![(0, 1.0)]
        );
    }

    #[test]
    fn conservation_positions_is_undefined_for_generic() {
        let alignment = Alignment::new_with_type(
            vec![raw("s1", b"A"), raw("s2", b"A")],
            AlignmentType::Generic,
        )
        .unwrap();

        assert_eq!(
            alignment.conservation_positions(&[0]),
            Err(AlignmentError::ConservationUndefined)
        );
    }

    #[test]
    fn gap_fraction_positions_returns_values() {
        let alignment = Alignment::new_with_type(
            vec![raw("s1", b"A-"), raw("s2", b"--"), raw("s3", b"AA")],
            AlignmentType::Dna,
        )
        .unwrap();

        let fractions = alignment.gap_fraction_positions(&[0, 1]).unwrap();
        assert!((fractions[0].1 - (1.0 / 3.0)).abs() < f32::EPSILON);
        assert!((fractions[1].1 - (2.0 / 3.0)).abs() < f32::EPSILON);
    }

    #[test]
    fn column_summaries_positions_returns_all_metrics() {
        let alignment = Alignment::new_with_type(
            vec![raw("s1", b"A-"), raw("s2", b"--"), raw("s3", b"AA")],
            AlignmentType::Dna,
        )
        .unwrap();

        let summaries = alignment
            .column_summaries_positions(&[0, 1], ConsensusMethod::MajorityNonGap)
            .unwrap();

        assert_eq!(summaries.len(), 2);
        assert_eq!(summaries[0].position, 0);
        assert_eq!(summaries[0].consensus, Some(b'A'));
        assert!(summaries[0].conservation.is_some());
        assert!((summaries[0].gap_fraction - (1.0 / 3.0)).abs() < f32::EPSILON);
        assert_eq!(summaries[1].position, 1);
        assert_eq!(summaries[1].consensus, Some(b'A'));
        assert!(summaries[1].conservation.is_some());
        assert!((summaries[1].gap_fraction - (2.0 / 3.0)).abs() < f32::EPSILON);
    }
}
