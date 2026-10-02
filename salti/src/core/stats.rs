use std::ops::Range;

use tracing::warn;

use crate::core::model::{AlignmentModel, StatsView};

#[derive(Debug)]
pub struct Stats {
    range: Range<usize>,
    summaries: Vec<libmsa::ColumnSummary>,
}

impl Stats {
    pub fn compute(alignment: &AlignmentModel, col_range: Range<usize>) -> Option<Self> {
        let ctx = alignment.stats_context(col_range)?;
        let method = alignment.consensus_method;
        let summaries = match ctx.view {
            StatsView::Raw => alignment
                .view()
                .column_summaries_range(ctx.range.clone(), method),
            StatsView::Translated(frame) => {
                alignment.view().translated(frame).and_then(|translated| {
                    translated.column_summaries_range(ctx.range.clone(), method)
                })
            }
        };

        match summaries {
            Ok(summaries) => Some(Self {
                range: ctx.range,
                summaries,
            }),
            Err(error) => {
                warn!(%error, "Failed to compute visible column stats");
                None
            }
        }
    }

    pub fn summary_at(&self, col: usize) -> Option<&libmsa::ColumnSummary> {
        if !self.range.contains(&col) {
            return None;
        }
        self.summaries.get(col - self.range.start)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::Stats;
    use crate::core::model::AlignmentModel;

    fn raw(id: &str, sequence: &[u8]) -> libmsa::RawSequence {
        libmsa::RawSequence {
            id: id.to_string(),
            sequence: sequence.to_vec(),
        }
    }

    fn alignment_model(sequences: Vec<libmsa::RawSequence>) -> AlignmentModel {
        let alignment = libmsa::Alignment::new(sequences).unwrap();
        AlignmentModel::new(alignment).unwrap()
    }

    pub(crate) fn from_consensus(start: usize, consensus: &[u8]) -> Stats {
        let summaries = consensus
            .iter()
            .enumerate()
            .map(|(offset, &byte)| libmsa::ColumnSummary {
                position: start + offset,
                consensus: Some(byte),
                conservation: Some(1.0),
            })
            .collect();
        Stats {
            range: start..start + consensus.len(),
            summaries,
        }
    }

    fn consensus_at(stats: &Stats, col: usize) -> Option<u8> {
        stats.summary_at(col).and_then(|summary| summary.consensus)
    }

    #[test]
    fn compute_raw_returns_visible_range() {
        let model = alignment_model(vec![raw("seq1", b"CATC"), raw("seq2", b"CATC")]);

        let stats = Stats::compute(&model, 1..3).unwrap();

        assert_eq!(consensus_at(&stats, 1), Some(b'A'));
        assert_eq!(consensus_at(&stats, 2), Some(b'T'));
        assert_eq!(stats.summary_at(1).map(|summary| summary.position), Some(1));
    }

    #[test]
    fn compute_translated_maps_to_protein_columns() {
        let mut model = alignment_model(vec![raw("seq1", b"ATGAAATTT")]);
        model
            .set_translation(Some(libmsa::ReadingFrame::Frame2))
            .unwrap();

        let stats = Stats::compute(&model, 0..9).unwrap();
        assert_eq!(consensus_at(&stats, 0), Some(b'*'));
        assert_eq!(consensus_at(&stats, 1), Some(b'N'));
        assert!(stats.summary_at(2).is_none());
    }

    #[test]
    fn compute_returns_none_without_visible_columns() {
        let model = alignment_model(vec![raw("seq1", b"CATC")]);
        assert!(Stats::compute(&model, 0..0).is_none());

        let mut translated = alignment_model(vec![raw("seq1", b"AT")]);
        translated
            .set_translation(Some(libmsa::ReadingFrame::Frame1))
            .unwrap();
        assert!(Stats::compute(&translated, 0..2).is_none());
    }

    #[test]
    fn summary_at_returns_none_outside_range() {
        let stats = from_consensus(2, b"AC");

        assert!(stats.summary_at(1).is_none());
        assert_eq!(consensus_at(&stats, 2), Some(b'A'));
        assert_eq!(consensus_at(&stats, 3), Some(b'C'));
        assert!(stats.summary_at(4).is_none());
    }
}
