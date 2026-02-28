use crate::core::parser::{Alignment, SequenceType};
use std::sync::Arc;

/// Represents one alignment row plus metadata.
#[derive(Debug, Clone)]
pub struct SequenceRecord {
    /// Index identifying the sequence, assigned in absolute order during loading.
    pub sequence_id: usize,
    /// Parsed sequence identifier and residue/gap bytes.
    pub alignment: Alignment,
}

/// Holds the loaded `SequenceRecord`s and derived metadata.
#[derive(Debug, Default)]
pub struct AlignmentData {
    /// Sequence records in absolute alignment order.
    pub sequences: Arc<Vec<SequenceRecord>>,
    /// Shared alignment length inferred from the first record, or `0` when empty.
    pub sequence_length: usize,
    /// Maximum displayed character width among sequence identifiers.
    pub max_sequence_id_len: usize,
    /// Detected or user-selected sequence type for rendering/analysis.
    pub sequence_type: Option<SequenceType>,
}

impl AlignmentData {
    /// Loads alignments into `SequenceRecord`s
    ///
    /// `sequence_length` is set from the first alignment, or `0` if `alignments` is empty.
    pub fn load_alignments(&mut self, alignments: Vec<Alignment>) {
        self.sequence_length = alignments.first().map_or(0, |a| a.sequence.len());
        self.max_sequence_id_len = alignments
            .iter()
            .map(|alignment| alignment.id.chars().count())
            .max()
            .unwrap_or(0);
        self.sequences = Arc::new(
            alignments
                .into_iter()
                .enumerate()
                .map(|(index, alignment)| SequenceRecord {
                    sequence_id: index,
                    alignment,
                })
                .collect(),
        );
    }
}
