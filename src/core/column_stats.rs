use crate::core::data::SequenceRecord;
use crate::core::parser::SequenceType;
use rand::seq::IteratorRandom;
use std::sync::Arc;
use tracing::{debug, trace};

/// extra columns on each side of the viewport used as precomputed context.
pub(crate) const COLUMN_STATS_BUFFER_COLS: usize = 500;
/// minimum distance from the cached window edge before triggering recomputation.
pub(crate) const COLUMN_STATS_RECALC_MARGIN_COLS: usize = 25;

/// Strategy used to select the consensus nucleotide for each alignment column.
///
/// `Majority` includes all observed symbols, including gaps (`-`).
/// `MajorityNonGap` excludes gaps when choosing the winning symbol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsensusMethod {
    Majority,
    MajorityNonGap,
}

/// Per-position updates for both column-derived outputs.
///
/// Consensus and conservation always share the same requested position set and
/// per-column counts, so both outputs are computed together.
#[derive(Debug, Default)]
pub struct ColumnStats {
    pub consensus: Vec<(usize, u8)>,
    pub conservation: Vec<(usize, f32)>,
}

/// Request for update
#[derive(Debug, Clone)]
pub struct ColumnStatsRequest {
    pub sequences: Arc<Vec<SequenceRecord>>,
    pub positions: Vec<usize>,
    pub method: ConsensusMethod,
    pub sequence_type: SequenceType,
}

/// Applies per-position updates.
pub(crate) fn apply_positional_updates<T: Copy>(
    cache: &mut Option<Vec<T>>,
    sequence_length: usize,
    default_value: T,
    updates: &[(usize, T)],
) {
    if updates.is_empty() {
        return;
    }

    let cache = cache.get_or_insert_with(|| vec![default_value; sequence_length]);

    for &(position, value) in updates {
        if position < cache.len() {
            cache[position] = value;
        }
    }
}

/// Computes consensus bytes and conservation scores for the requested alignment
/// positions.
///
/// For each position, counts observed bytes across all sequences once, then
/// derives the consensus byte via `method` and conservation score via
/// `sequence_type`.
///
/// Can be cancelled via `cancel` token to stop processing.
pub(crate) fn compute_column_stats(
    sequences: &[SequenceRecord],
    positions: &[usize],
    method: ConsensusMethod,
    sequence_type: SequenceType,
    cancel: &tokio_util::sync::CancellationToken,
) -> ColumnStats {
    if positions.is_empty() || cancel.is_cancelled() {
        trace!(
            position_count = positions.len(),
            cancelled = cancel.is_cancelled(),
            "skipping column stats compute"
        );
        return ColumnStats::default();
    }

    let max_entropy = match sequence_type {
        SequenceType::AminoAcid => 20f64.log2(),
        SequenceType::Dna => 4f64.log2(),
    };

    trace!(
        sequence_count = sequences.len(),
        position_count = positions.len(),
        method = ?method,
        sequence_type = ?sequence_type,
        "starting column stats compute"
    );

    let mut stats = ColumnStats {
        consensus: Vec::with_capacity(positions.len()),
        conservation: Vec::with_capacity(positions.len()),
    };
    let mut rng = rand::rng();

    for position in positions.iter().copied() {
        if cancel.is_cancelled() {
            break;
        }

        let mut counts = [0u32; 256];
        for sequence in sequences {
            if let Some(&nucleotide) = sequence.alignment.sequence.get(position) {
                counts[nucleotide as usize] += 1;
            }
        }

        let consensus_nucleotide = select_consensus_char(&counts, method, &mut rng);
        stats.consensus.push((position, consensus_nucleotide));
        stats
            .conservation
            .push((position, conservation_from_counts(&counts, max_entropy)));
    }

    debug!(
        sequence_count = sequences.len(),
        requested_positions = positions.len(),
        consensus_updates = stats.consensus.len(),
        conservation_updates = stats.conservation.len(),
        method = ?method,
        sequence_type = ?sequence_type,
        cancelled = cancel.is_cancelled(),
        "completed column stats compute"
    );

    stats
}

/// Selects the consensus char (in bytes) from the calculated frequencies.
///
/// When multiple symbols share the highest count, one is chosen at random.
/// If `method` excludes gaps, `-` is ignored as a candidate.
/// Returns `b'?'` when no candidate is available.
fn select_consensus_char<R: rand::Rng>(
    counts: &[u32; 256],
    method: ConsensusMethod,
    rng: &mut R,
) -> u8 {
    let mut max_count = 0u32;
    let mut candidates = Vec::new();
    let exclude_gap = matches!(method, ConsensusMethod::MajorityNonGap);

    for (nucleotide_index, &count) in counts.iter().enumerate() {
        if count == 0 {
            continue;
        }
        if exclude_gap && nucleotide_index == b'-' as usize {
            continue;
        }

        if count > max_count {
            max_count = count;
            candidates.clear();
            candidates.push(nucleotide_index as u8);
        } else if count == max_count {
            candidates.push(nucleotide_index as u8);
        }
    }

    candidates.into_iter().choose(rng).unwrap_or(b'?')
}

/// Calculates a conservation score between 0 and 1 for an alignment column
///
/// Uses shannon entropy, normalised by a max value (4 for NT, 20 for AA).
/// This is an implementation of the conservation calculation used in JBrowse MSA
fn conservation_from_counts(counts: &[u32; 256], max_entropy: f64) -> f32 {
    // Ported from JBrowseMSA (https://github.com/GMOD/JBrowseMSA
    // Original author: [Colin Diesh] — MIT License (https://github.com/GMOD/JBrowseMSA/blob/main/LICENSE)
    let mut total = 0u32;
    let mut gap_count = 0u32;
    let mut merged_non_gap_counts = [0u32; 256];

    for (symbol, &count) in counts.iter().enumerate() {
        if count == 0 {
            continue;
        }
        total += count;

        if symbol == b'-' as usize || symbol == b'.' as usize {
            gap_count += count;
            continue;
        }

        let upper = (symbol as u8).to_ascii_uppercase() as usize;
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
    let non_gap_total = f64::from(non_gap_total);
    for &count in &merged_non_gap_counts {
        if count == 0 {
            continue;
        }
        let frequency = f64::from(count) / non_gap_total;
        entropy -= frequency * frequency.log2();
    }

    let gap_fraction = f64::from(gap_count) / f64::from(total);
    let conservation = (1.0 - entropy / max_entropy).max(0.0);
    (conservation * (1.0 - gap_fraction)) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn counts_for_symbols(symbols: &[u8]) -> [u32; 256] {
        let mut counts = [0u32; 256];
        for &symbol in symbols {
            counts[symbol as usize] += 1;
        }
        counts
    }

    #[test]
    fn conservation_for_fully_conserved_non_gap() {
        let counts = counts_for_symbols(b"AAAA");
        let conservation = conservation_from_counts(&counts, 4f64.log2());
        assert_eq!(conservation, 1.0);
    }

    #[test]
    fn conservation_gap_penalty() {
        let counts = counts_for_symbols(b"AA--");
        let conservation = conservation_from_counts(&counts, 4f64.log2());
        assert_eq!(conservation, 0.5);
    }

    #[test]
    fn conservation_zero_for_all_gap() {
        let counts = counts_for_symbols(b"--..");
        let conservation = conservation_from_counts(&counts, 4f64.log2());
        assert_eq!(conservation, 0.0);
    }

    #[test]
    fn conservation_handles_case() {
        let counts = counts_for_symbols(b"AaAa");
        let conservation = conservation_from_counts(&counts, 4f64.log2());
        assert_eq!(conservation, 1.0);
    }
}
