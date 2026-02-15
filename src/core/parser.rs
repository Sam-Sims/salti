use color_eyre::{eyre::eyre, Result};
use needletail::parser::parse_fastx_file;
use rand::seq::IndexedRandom;
use std::path::PathBuf;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

/// minimum amino-acid character fraction required to classify input as amino acid.
const AMINO_ACID_CLASSIFICATION_THRESHOLD: f32 = 0.5;

/// Type of sequences in the alignment: either DNA or amino acid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SequenceType {
    Dna,
    AminoAcid,
}

/// One parsed fasta record.
#[derive(Debug, Clone)]
pub struct Alignment {
    pub id: Arc<str>,
    pub sequence: Arc<[u8]>,
}

/// Tries to classify alignments as DNA or amino acid.
///
/// Samples up to 100 random alignments, counts suspected amino acid chars, and returns `AminoAcid`
/// when their fraction is at least `AMINO_ACID_CLASSIFICATION_THRESHOLD`, otherwise falls back to `Dna`.
/// Should work well enough, unless its a tiny input or lots of ambiguity
#[must_use]
pub fn detect_sequence_type(alignments: &[Alignment]) -> SequenceType {
    let amino_acid_chars = b"DEFHIKLMNPQRSVWY";
    let mut rng = rand::rng();
    let (sampled_alignment_count, amino_acid_count, total_count) =
        alignments.sample(&mut rng, 100).fold(
            (0, 0, 0),
            |(sampled_alignment_count, amino_acid_count, total_count), alignment| {
                let sequence = alignment.sequence.as_ref();
                let amino_acid_in_sequence = sequence
                    .iter()
                    .filter(|&&byte| amino_acid_chars.contains(&byte))
                    .count();
                let total_in_sequence = sequence.len();

                (
                    sampled_alignment_count + 1,
                    amino_acid_count + amino_acid_in_sequence,
                    total_count + total_in_sequence,
                )
            },
        );

    if total_count == 0 {
        debug!(
            sampled_alignment_count,
            amino_acid_count,
            total_count,
            sequence_type = ?SequenceType::Dna,
            "defaulted sequence type to DNA because sampled sequences had zero total length"
        );
        return SequenceType::Dna;
    }

    let amino_acid_fraction = amino_acid_count as f32 / total_count as f32;
    if amino_acid_fraction >= AMINO_ACID_CLASSIFICATION_THRESHOLD {
        debug!(
            sampled_alignment_count,
            amino_acid_count,
            total_count,
            amino_acid_fraction,
            sequence_type = ?SequenceType::AminoAcid,
            "detected sequence type"
        );
        SequenceType::AminoAcid
    } else {
        debug!(
            sampled_alignment_count,
            amino_acid_count,
            total_count,
            amino_acid_fraction,
            sequence_type = ?SequenceType::Dna,
            "detected sequence type"
        );
        SequenceType::Dna
    }
}

/// Parses a fasta file into `Alignment`s with cooperative cancellation.
///
/// Intended to run on a blocking worker thread (via `tokio::task::spawn_blocking`).
/// Returns an error when the file is missing, a record is invalid, or sequence lengths differ.
pub fn parse_fasta_file(path: PathBuf, cancel: &CancellationToken) -> Result<Vec<Alignment>> {
    info!(path = ?path, "starting fasta parse");
    let mut parser = parse_fastx_file(&path).map_err(|e| {
        error!(path = ?path, error = %e, "failed to initialise fastx parser");
        eyre!("Failed to parse file: {}", e)
    })?;
    let mut alignments = Vec::new();
    let mut expected_length: Option<usize> = None;

    while let Some(record) = parser.next() {
        if cancel.is_cancelled() {
            debug!(path = ?path, "cancelled fasta parse");
            return Err(eyre!("Cancelled fasta parse"));
        }

        let record = record.map_err(|e| {
            error!(path = ?path, error = %e, "error reading fasta record");
            eyre!("Error reading record: {}", e)
        })?;
        let id = Arc::from(std::str::from_utf8(record.id()).map_err(|e| {
            error!(path = ?path, error = %e, "invalid fasta sequence id");
            eyre!("Invalid sequence ID: {}", e)
        })?);

        let sequence = Arc::from(record.seq().to_vec());
        let sequence_length = record.seq().len();
        if let Some(length) = expected_length {
            if sequence_length != length {
                warn!(
                    path = ?path,
                    expected_length = length,
                    found_length = sequence_length,
                    id = %id,
                    "sequence length mismatch while parsing fasta"
                );
                return Err(eyre!(
                    "Sequence length mismatch: expected {}, found {} for id {}",
                    length,
                    sequence_length,
                    id
                ));
            }
        } else {
            expected_length = Some(sequence_length);
        }

        alignments.push(Alignment { id, sequence });
    }
    debug!(
        path = ?path,
        alignment_count = alignments.len(),
        expected_length = expected_length.unwrap_or(0),
        "completed fasta parse"
    );
    Ok(alignments)
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;
    use tempfile::NamedTempFile;

    fn create_temp_fasta(content: &str) -> NamedTempFile {
        let temp_file = NamedTempFile::new().unwrap();
        std::fs::write(temp_file.path(), content).unwrap();
        temp_file
    }

    #[test]
    fn test_parse_valid() {
        let content = ">seq1\nA-CG\n>seq2\nTGCA\n";
        let temp_file = create_temp_fasta(content);
        let result = parse_fasta_file(temp_file.path().to_path_buf(), &CancellationToken::new());
        assert!(result.is_ok());
        let alignments = result.unwrap();
        assert_eq!(alignments.len(), 2);
        assert_eq!(alignments[0].id.as_ref(), "seq1");
        assert_eq!(alignments[0].sequence.as_ref(), b"A-CG");
        assert_eq!(alignments[1].id.as_ref(), "seq2");
        assert_eq!(alignments[1].sequence.as_ref(), b"TGCA");
    }

    #[test]
    fn test_parse_nonexistant() {
        let result = parse_fasta_file(
            PathBuf::from_str("idontexist.fasta").unwrap(),
            &CancellationToken::new(),
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_empty() {
        let content = "";
        let temp_file = create_temp_fasta(content);
        let result = parse_fasta_file(temp_file.path().to_path_buf(), &CancellationToken::new());
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_no_seqs() {
        let content = ">seq1\n>seq2\n";
        let temp_file = create_temp_fasta(content);
        let result = parse_fasta_file(temp_file.path().to_path_buf(), &CancellationToken::new());
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_length_mismatch() {
        let content = ">seq1\nATCG\n>seq2\nTGCAAA\n";
        let temp_file = create_temp_fasta(content);
        let result = parse_fasta_file(temp_file.path().to_path_buf(), &CancellationToken::new());
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_invalid() {
        let content = "imaninvalidfasta\nfile\n";
        let temp_file = create_temp_fasta(content);
        let result = parse_fasta_file(temp_file.path().to_path_buf(), &CancellationToken::new());
        assert!(result.is_err());
    }

    #[test]
    fn test_detect_sequence_type_dna() {
        let alignments = vec![
            Alignment {
                id: Arc::from("seq1"),
                sequence: Arc::from(b"ACGTACGT".to_vec()),
            },
            Alignment {
                id: Arc::from("seq2"),
                sequence: Arc::from(b"TGCA".to_vec()),
            },
        ];
        let result = detect_sequence_type(&alignments);
        assert_eq!(result, SequenceType::Dna);
    }

    #[test]
    fn test_detect_sequence_type_aa() {
        let alignments = vec![
            Alignment {
                id: Arc::from("seq1"),
                sequence: Arc::from(b"ACDEFGHIKLMNPQRSTVWY".to_vec()),
            },
            Alignment {
                id: Arc::from("seq2"),
                sequence: Arc::from(b"ACDEFGHIKLMNPQRSTVWY".to_vec()),
            },
        ];
        let result = detect_sequence_type(&alignments);
        assert_eq!(result, SequenceType::AminoAcid);
    }

    #[test]
    fn test_detect_sequence_type_zero_length_sequences_default_to_dna() {
        let alignments = vec![
            Alignment {
                id: Arc::from("seq1"),
                sequence: Arc::from(Vec::<u8>::new()),
            },
            Alignment {
                id: Arc::from("seq2"),
                sequence: Arc::from(Vec::<u8>::new()),
            },
        ];
        let result = detect_sequence_type(&alignments);
        assert_eq!(result, SequenceType::Dna);
    }
}
