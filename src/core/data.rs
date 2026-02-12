use crate::core::parser::{Alignment, SequenceType};
use std::path::PathBuf;
use std::sync::Arc;

/// Represents one alignment row plus metadata.
#[derive(Debug, Clone)]
pub struct SequenceRecord {
    pub sequence_id: usize,
    pub hidden: bool,
    pub alignment: Alignment,
}

/// Holds the loaded alignment including the `SequenceRecords` and metadata
#[derive(Debug, Default)]
pub struct AlignmentData {
    pub sequences: Arc<Vec<SequenceRecord>>,
    pub file_path: Option<PathBuf>,
    // assumes all sequences have the same length, (should be for alignments) so is taken
    // from the first seq when loading
    pub sequence_length: usize,
    pub max_sequence_id_len: usize,
    pub sequence_type: Option<SequenceType>,
}

impl AlignmentData {
    /// Loads the given alignments into `SequenceRecord`s and sets metadata.
    ///
    /// `sequence_length` is taken from the first alignment (or `0` when empty),
    /// and each sequence is assigned a stable `sequence_id` based on its index.
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
                    hidden: false,
                    alignment,
                })
                .collect(),
        );
    }
}
