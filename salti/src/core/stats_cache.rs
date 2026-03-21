use std::ops::Range;

use crate::core::model::StatsView;

const CHUNK_SIZE: usize = 5000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ChunkState {
    Empty,
    Pending,
    Filled,
}

#[derive(Debug)]
struct ChunkedCache {
    chunks: Vec<ChunkState>,
    summaries: Vec<Option<libmsa::ColumnSummary>>,
}

#[derive(Debug)]
pub struct ColumnStatsCache {
    generation: u64,
    raw: ChunkedCache,
    translated: ChunkedCache,
    translated_frame: Option<libmsa::ReadingFrame>,
}

pub struct StatsJobRequest {
    pub alignment: libmsa::Alignment,
    pub view: StatsView,
    pub chunk_idx: usize,
    pub range: Range<usize>,
    pub method: libmsa::ConsensusMethod,
    pub generation: u64,
}

#[derive(Debug)]
pub struct StatsJobResult {
    pub generation: u64,
    pub chunk_idx: usize,
    pub view: StatsView,
    pub summaries: Result<Vec<libmsa::ColumnSummary>, String>,
}

impl ChunkedCache {
    fn new(total_columns: usize) -> Self {
        let n_chunks = total_columns.div_ceil(CHUNK_SIZE);
        Self {
            chunks: vec![ChunkState::Empty; n_chunks],
            summaries: vec![None; total_columns],
        }
    }

    fn empty() -> Self {
        Self {
            chunks: Vec::new(),
            summaries: Vec::new(),
        }
    }

    fn reset(&mut self, total_columns: usize) {
        let n_chunks = total_columns.div_ceil(CHUNK_SIZE);
        self.chunks = vec![ChunkState::Empty; n_chunks];
        self.summaries = vec![None; total_columns];
    }

    fn chunks_for_range(&self, range: &Range<usize>) -> Range<usize> {
        if range.is_empty() || self.chunks.is_empty() {
            return 0..0;
        }
        let start_chunk = range.start / CHUNK_SIZE;
        let end_chunk = (range.end.saturating_sub(1) / CHUNK_SIZE + 1).min(self.chunks.len());
        start_chunk..end_chunk
    }

    fn chunk_range(&self, chunk_idx: usize) -> Range<usize> {
        let start = chunk_idx * CHUNK_SIZE;
        let end = (start + CHUNK_SIZE).min(self.summaries.len());
        start..end
    }

    fn fill_chunk(&mut self, chunk_idx: usize, summaries: Vec<libmsa::ColumnSummary>) {
        let range = self.chunk_range(chunk_idx);
        for (offset, summary) in summaries.into_iter().enumerate() {
            let col = range.start + offset;
            if let Some(slot) = self.summaries.get_mut(col) {
                *slot = Some(summary);
            }
        }
        if let Some(state) = self.chunks.get_mut(chunk_idx) {
            *state = ChunkState::Filled;
        }
    }
}

impl Default for ColumnStatsCache {
    fn default() -> Self {
        Self {
            generation: 0,
            raw: ChunkedCache::empty(),
            translated: ChunkedCache::empty(),
            translated_frame: None,
        }
    }
}

impl ColumnStatsCache {
    pub fn init(&mut self, nucleotide_cols: usize) {
        self.generation += 1;
        self.raw = ChunkedCache::new(nucleotide_cols);
        self.translated = ChunkedCache::empty();
        self.translated_frame = None;
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn raw_summary_at(&self, col: usize) -> Option<&libmsa::ColumnSummary> {
        self.raw
            .summaries
            .get(col)
            .and_then(|summary| summary.as_ref())
    }

    pub fn translated_summary_at(
        &self,
        frame: libmsa::ReadingFrame,
        protein_col: usize,
    ) -> Option<&libmsa::ColumnSummary> {
        if self.translated_frame != Some(frame) {
            return None;
        }
        self.translated
            .summaries
            .get(protein_col)
            .and_then(|summary| summary.as_ref())
    }

    pub fn raw_chunks_to_spawn(&mut self, visible_col_range: &Range<usize>) -> Vec<usize> {
        self.raw
            .chunks_for_range(visible_col_range)
            .filter(|&idx| self.raw.chunks[idx] == ChunkState::Empty)
            .collect()
    }

    pub fn translated_chunks_to_spawn(
        &mut self,
        visible_protein_range: &Range<usize>,
        frame: libmsa::ReadingFrame,
        protein_cols: usize,
    ) -> Vec<usize> {
        if self.translated_frame != Some(frame) || self.translated.summaries.len() != protein_cols {
            self.translated_frame = Some(frame);
            self.translated = ChunkedCache::new(protein_cols);
        }

        self.translated
            .chunks_for_range(visible_protein_range)
            .filter(|&idx| self.translated.chunks[idx] == ChunkState::Empty)
            .collect()
    }

    pub fn mark_raw_pending(&mut self, chunk_idx: usize) {
        if let Some(state) = self.raw.chunks.get_mut(chunk_idx) {
            *state = ChunkState::Pending;
        }
    }

    pub fn mark_translated_pending(&mut self, chunk_idx: usize) {
        if let Some(state) = self.translated.chunks.get_mut(chunk_idx) {
            *state = ChunkState::Pending;
        }
    }

    pub fn raw_chunk_range(&self, chunk_idx: usize) -> Range<usize> {
        self.raw.chunk_range(chunk_idx)
    }

    pub fn translated_chunk_range(&self, chunk_idx: usize) -> Range<usize> {
        self.translated.chunk_range(chunk_idx)
    }

    pub fn store(&mut self, result: StatsJobResult) -> bool {
        if result.generation != self.generation {
            return false;
        }

        let summaries = match result.summaries {
            Ok(summaries) => summaries,
            Err(_) => {
                let cache = match result.view {
                    StatsView::Raw => &mut self.raw,
                    StatsView::Translated(frame) => {
                        if self.translated_frame != Some(frame) {
                            return false;
                        }
                        &mut self.translated
                    }
                };
                if let Some(state) = cache.chunks.get_mut(result.chunk_idx) {
                    *state = ChunkState::Empty;
                }
                return false;
            }
        };

        let cache = match result.view {
            StatsView::Raw => &mut self.raw,
            StatsView::Translated(frame) => {
                if self.translated_frame != Some(frame) {
                    return false;
                }
                &mut self.translated
            }
        };
        cache.fill_chunk(result.chunk_idx, summaries);
        true
    }

    pub fn invalidate_all(&mut self, nucleotide_cols: usize) {
        self.generation += 1;
        self.raw.reset(nucleotide_cols);
        self.translated = ChunkedCache::empty();
        self.translated_frame = None;
    }

    pub fn invalidate_translated(&mut self) {
        self.generation += 1;
        self.translated = ChunkedCache::empty();
        self.translated_frame = None;
    }
}

#[cfg(test)]
mod tests {
    use super::{CHUNK_SIZE, ChunkState, ChunkedCache, ColumnStatsCache, StatsJobResult};
    use crate::core::model::StatsView;

    fn summary(consensus: u8) -> libmsa::ColumnSummary {
        libmsa::ColumnSummary {
            position: 0,
            consensus: Some(consensus),
            conservation: Some(1.0),
            gap_fraction: 0.0,
        }
    }

    #[test]
    fn chunks_for_range_handles_chunk_boundaries() {
        let cache = ChunkedCache::new(CHUNK_SIZE * 3);

        assert_eq!(cache.chunks_for_range(&(0..0)), 0..0);
        assert_eq!(cache.chunks_for_range(&(0..1)), 0..1);
        assert_eq!(cache.chunks_for_range(&(CHUNK_SIZE - 1..CHUNK_SIZE)), 0..1);
        assert_eq!(cache.chunks_for_range(&(CHUNK_SIZE..CHUNK_SIZE + 1)), 1..2);
        assert_eq!(
            cache.chunks_for_range(&(CHUNK_SIZE - 1..CHUNK_SIZE + 1)),
            0..2
        );
        assert_eq!(cache.chunks_for_range(&(CHUNK_SIZE..CHUNK_SIZE * 3)), 1..3);
    }

    #[test]
    fn fill_chunk_writes_summaries_into_the_chunk_range() {
        let mut cache = ChunkedCache::new(CHUNK_SIZE + 3);

        cache.fill_chunk(1, vec![summary(b'A'), summary(b'C'), summary(b'G')]);

        assert_eq!(
            cache.summaries[CHUNK_SIZE]
                .as_ref()
                .and_then(|it| it.consensus),
            Some(b'A')
        );
        assert_eq!(
            cache.summaries[CHUNK_SIZE + 1]
                .as_ref()
                .and_then(|it| it.consensus),
            Some(b'C')
        );
        assert_eq!(
            cache.summaries[CHUNK_SIZE + 2]
                .as_ref()
                .and_then(|it| it.consensus),
            Some(b'G')
        );
        assert_eq!(cache.chunks[1], ChunkState::Filled);
    }

    #[test]
    fn raw_chunks_to_spawn_returns_only_empty_chunks() {
        let mut cache = ColumnStatsCache::default();
        cache.init(CHUNK_SIZE * 3);
        cache.mark_raw_pending(1);
        cache.raw.fill_chunk(2, vec![summary(b'A'); CHUNK_SIZE]);

        assert_eq!(cache.raw_chunks_to_spawn(&(0..CHUNK_SIZE * 3)), vec![0]);
    }

    #[test]
    fn store_discards_generation_mismatch() {
        let mut cache = ColumnStatsCache::default();
        cache.init(10);

        let stored = cache.store(StatsJobResult {
            generation: cache.generation() + 1,
            chunk_idx: 0,
            view: StatsView::Raw,
            summaries: Ok(vec![summary(b'A'); 10]),
        });

        assert!(!stored);
        assert!(cache.raw_summary_at(0).is_none());
    }

    #[test]
    fn store_discards_translated_frame_mismatch() {
        let mut cache = ColumnStatsCache::default();
        cache.init(10);
        let _ = cache.translated_chunks_to_spawn(&(0..2), libmsa::ReadingFrame::Frame1, 2);

        let stored = cache.store(StatsJobResult {
            generation: cache.generation(),
            chunk_idx: 0,
            view: StatsView::Translated(libmsa::ReadingFrame::Frame2),
            summaries: Ok(vec![summary(b'M'); 2]),
        });

        assert!(!stored);
        assert!(
            cache
                .translated_summary_at(libmsa::ReadingFrame::Frame1, 0)
                .is_none()
        );
    }

    #[test]
    fn store_fills_chunk_and_marks_it_filled() {
        let mut cache = ColumnStatsCache::default();
        cache.init(10);
        cache.mark_raw_pending(0);

        let stored = cache.store(StatsJobResult {
            generation: cache.generation(),
            chunk_idx: 0,
            view: StatsView::Raw,
            summaries: Ok(vec![summary(b'A'); 10]),
        });

        assert!(stored);
        assert_eq!(
            cache.raw_summary_at(0).and_then(|it| it.consensus),
            Some(b'A')
        );
        assert_eq!(cache.raw.chunks[0], ChunkState::Filled);
    }

    #[test]
    fn invalidate_all_resets_both_caches_and_bumps_generation() {
        let mut cache = ColumnStatsCache::default();
        cache.init(10);
        let previous_generation = cache.generation();
        let _ = cache.translated_chunks_to_spawn(&(0..2), libmsa::ReadingFrame::Frame1, 2);

        cache.invalidate_all(5);

        assert_eq!(cache.generation(), previous_generation + 1);
        assert_eq!(cache.raw.summaries.len(), 5);
        assert!(cache.translated.summaries.is_empty());
        assert_eq!(cache.translated_frame, None);
    }

    #[test]
    fn invalidate_translated_preserves_raw_cache_and_bumps_generation() {
        let mut cache = ColumnStatsCache::default();
        cache.init(10);
        cache.store(StatsJobResult {
            generation: cache.generation(),
            chunk_idx: 0,
            view: StatsView::Raw,
            summaries: Ok(vec![summary(b'A'); 10]),
        });
        let _ = cache.translated_chunks_to_spawn(&(0..2), libmsa::ReadingFrame::Frame1, 2);
        let previous_generation = cache.generation();

        cache.invalidate_translated();

        assert_eq!(cache.generation(), previous_generation + 1);
        assert_eq!(
            cache.raw_summary_at(0).and_then(|it| it.consensus),
            Some(b'A')
        );
        assert!(cache.translated.summaries.is_empty());
        assert_eq!(cache.translated_frame, None);
    }
}
