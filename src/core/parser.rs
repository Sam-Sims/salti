use color_eyre::{Result, eyre::eyre};
use paraseq::fasta;
use rand::seq::IndexedRandom;
use std::path::Path;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info};

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

/// Returns `true` when the input looks like an HTTP or HTTPS URL.
fn is_http_url(input: &str) -> bool {
    input.starts_with("http://") || input.starts_with("https://")
}

/// Returns `true` when the input looks like an SSH path.
fn is_ssh_path(input: &str) -> bool {
    input.starts_with("ssh://")
}

/// Opens a FASTA reader for the given input source.
///
/// Supports HTTP/HTTPS URLs, SSH paths, and local file paths.
/// Transparent decompression (gzip, bzip2, xz, zstd) is handled by paraseq via niffler.
fn open_fasta_reader(input: &str) -> Result<fasta::Reader<paraseq::BoxedReader>> {
    if is_http_url(input) {
        return Ok(fasta::Reader::from_url(input)?);
    }
    if is_ssh_path(input) {
        return Ok(fasta::Reader::from_ssh(input)?);
    }
    Ok(fasta::Reader::from_path(Path::new(input))?)
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

    let amino_acid_fraction = amino_acid_count as f32 / total_count as f32;
    if amino_acid_fraction >= AMINO_ACID_CLASSIFICATION_THRESHOLD {
        debug!(
            sampled_alignment_count,
            amino_acid_count,
            total_count,
            amino_acid_fraction,
            sequence_type = ?SequenceType::AminoAcid,
            "Detected sequence type"
        );
        SequenceType::AminoAcid
    } else {
        debug!(
            sampled_alignment_count,
            amino_acid_count,
            total_count,
            amino_acid_fraction,
            sequence_type = ?SequenceType::Dna,
            "Detected sequence type"
        );
        SequenceType::Dna
    }
}

/// Parses a FASTA input into `Alignment`s with cooperative cancellation.
///
/// `input` can be a local file path, an HTTP/HTTPS URL, or an SSH path.
/// Intended to run on a blocking worker thread (via `tokio::task::spawn_blocking`).
/// Returns an error when the input is missing, a record is invalid, or sequence lengths differ.
pub fn parse_fasta_file(input: &str, cancel: &CancellationToken) -> Result<Vec<Alignment>> {
    info!(input = %input, "Starting fasta parse");
    let mut reader = open_fasta_reader(input).map_err(|e| eyre!("Failed to open input: {}", e))?;
    let mut record_set = reader.new_record_set();
    let mut alignments = Vec::new();
    let mut expected_length: Option<usize> = None;

    while record_set
        .fill(&mut reader)
        .map_err(|e| eyre!("Error reading records: {}", e))?
    {
        for record in record_set.iter() {
            if cancel.is_cancelled() {
                return Err(eyre!("Cancelled fasta parse"));
            }

            let record = record.map_err(|e| eyre!("Error reading record: {}", e))?;
            let id = Arc::from(
                std::str::from_utf8(record.id())
                    .map_err(|e| eyre!("Invalid sequence ID: {}", e))?,
            );

            let sequence = Arc::from(record.seq().to_vec());
            let sequence_length = record.seq().len();
            if let Some(length) = expected_length {
                if sequence_length != length {
                    return Err(eyre!(
                        "Sequence length mismatch: expected {}, found {} for id {}",
                        length,
                        sequence_length,
                        id
                    ));
                }
            } else {
                if sequence_length == 0 {
                    return Err(eyre!("Sequence has zero length for id {}", id));
                }
                expected_length = Some(sequence_length);
            }

            alignments.push(Alignment { id, sequence });
        }
    }

    if alignments.is_empty() {
        return Err(eyre!("No valid FASTA records found in input"));
    }
    debug!(
        input = %input,
        alignment_count = alignments.len(),
        expected_length = expected_length.unwrap_or(0),
        "Completed fasta parse"
    );
    Ok(alignments)
}

#[cfg(test)]
mod tests {
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
        let input = temp_file.path().to_str().unwrap();
        let result = parse_fasta_file(input, &CancellationToken::new());
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
        let result = parse_fasta_file("idontexist.fasta", &CancellationToken::new());
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_empty() {
        let content = "";
        let temp_file = create_temp_fasta(content);
        let input = temp_file.path().to_str().unwrap();
        let result = parse_fasta_file(input, &CancellationToken::new());
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_no_seqs() {
        let content = ">seq1\n>seq2\n";
        let temp_file = create_temp_fasta(content);
        let input = temp_file.path().to_str().unwrap();
        let result = parse_fasta_file(input, &CancellationToken::new());
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_length_mismatch() {
        let content = ">seq1\nATCG\n>seq2\nTGCAAA\n";
        let temp_file = create_temp_fasta(content);
        let input = temp_file.path().to_str().unwrap();
        let result = parse_fasta_file(input, &CancellationToken::new());
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_invalid() {
        let content = "imaninvalidfasta\nfile\n";
        let temp_file = create_temp_fasta(content);
        let input = temp_file.path().to_str().unwrap();
        let result = parse_fasta_file(input, &CancellationToken::new());
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
}
