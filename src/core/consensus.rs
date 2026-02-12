use crate::core::data::SequenceRecord;
use rand::seq::IteratorRandom;
use tracing::{debug, trace};

/// extra columns on each side of the viewport used as precomputed context.
pub(crate) const CONSENSUS_BUFFER_COLS: usize = 500;
/// minimum distance from the cached window edge before triggering recomputation.
pub(crate) const CONSENSUS_RECALC_MARGIN_COLS: usize = 25;

/// Strategy used to select the consensus nucleotide for each alignment column.
///
/// `Majority` includes all observed symbols, including gaps (`-`).
/// `MajorityNonGap` excludes gaps when choosing the winning symbol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsensusMethod {
    Majority,
    MajorityNonGap,
}

/// Returns positions in `[start, end)` that still need consensus calculation.
///
/// A position is treated as already calculated when `existing` contains a
/// non-space byte at that index. Missing indices or space bytes are treated as
/// not yet calculated.
pub(crate) fn subset_missing_positions(
    start: usize,
    end: usize,
    existing: Option<&[u8]>,
) -> Vec<usize> {
    (start..end)
        .filter(|&position| {
            existing
                .and_then(|consensus| consensus.get(position))
                .is_none_or(|&byte| byte == b' ')
        })
        .collect()
}

/// Applies per-position consensus updates into the consensus.
pub(crate) fn apply_consensus_updates(
    consensus: &mut Option<Vec<u8>>,
    sequence_length: usize,
    updates: Vec<(usize, u8)>,
) {
    if updates.is_empty() {
        return;
    }

    let consensus = consensus.get_or_insert_with(|| vec![b' '; sequence_length]);

    for (position, nucleotide) in updates {
        if position < consensus.len() {
            consensus[position] = nucleotide;
        }
    }
}

/// Computes consensus bytes for the requested alignment positions.
///
/// For each position, counts observed bytes across all sequences and selects a
/// consensus byte using `method`.
///
/// Can be cancelled via `cancel` token to stop processing.
pub(crate) fn compute_consensus(
    sequences: &[SequenceRecord],
    positions: &[usize],
    method: ConsensusMethod,
    cancel: &tokio_util::sync::CancellationToken,
) -> Vec<(usize, u8)> {
    if positions.is_empty() || cancel.is_cancelled() {
        trace!(
            position_count = positions.len(),
            cancelled = cancel.is_cancelled(),
            "skipping consensus compute"
        );
        return Vec::new();
    }
    trace!(
        sequence_count = sequences.len(),
        position_count = positions.len(),
        method = ?method,
        "starting consensus compute"
    );

    let mut updates = Vec::with_capacity(positions.len());
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
        updates.push((position, consensus_nucleotide));
    }

    debug!(
        sequence_count = sequences.len(),
        requested_positions = positions.len(),
        updated_positions = updates.len(),
        method = ?method,
        cancelled = cancel.is_cancelled(),
        "completed consensus compute"
    );

    updates
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

    for (nucleotide_index, count) in counts.iter().enumerate() {
        if *count == 0 {
            continue;
        }
        if exclude_gap && nucleotide_index == b'-' as usize {
            continue;
        }

        if *count > max_count {
            max_count = *count;
            candidates.clear();
            candidates.push(nucleotide_index as u8);
        } else if *count == max_count {
            candidates.push(nucleotide_index as u8);
        }
    }

    candidates.into_iter().choose(rng).unwrap_or(b'?')
}
