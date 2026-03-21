use std::path::Path;

use anyhow::{Result, format_err};
use libmsa::RawSequence;
use paraseq::fasta;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info};

pub fn parse_fasta_file(input: &str, cancel: &CancellationToken) -> Result<Vec<RawSequence>> {
    info!(input = %input, "Starting fasta parse");
    let mut reader =
        open_fasta_reader(input).map_err(|error| format_err!("Failed to open input: {error}"))?;
    let mut record_set = reader.new_record_set();
    let mut sequences = Vec::new();
    let mut expected_length: Option<usize> = None;

    while record_set
        .fill(&mut reader)
        .map_err(|error| format_err!("Error reading records: {error}"))?
    {
        for record in record_set.iter() {
            if cancel.is_cancelled() {
                return Err(format_err!("Cancelled fasta parse"));
            }

            let record = record.map_err(|error| format_err!("Error reading record: {error}"))?;
            let id = std::str::from_utf8(record.id())
                .map_err(|error| format_err!("Invalid sequence ID: {error}"))?
                .to_string();
            let sequence = record.seq().to_vec();
            let sequence_length = sequence.len();

            if let Some(length) = expected_length {
                if sequence_length != length {
                    return Err(format_err!(
                        "Sequence length mismatch: expected {}, found {} for id {}",
                        length,
                        sequence_length,
                        id
                    ));
                }
            } else if sequence_length == 0 {
                return Err(format_err!("Sequence has zero length for id {}", id));
            } else {
                expected_length = Some(sequence_length);
            }

            sequences.push(RawSequence { id, sequence });
        }
    }

    if sequences.is_empty() {
        return Err(format_err!("No valid FASTA records found in input"));
    }

    debug!(
        input = %input,
        sequence_count = sequences.len(),
        expected_length = expected_length.unwrap_or(0),
        "Completed fasta parse"
    );

    Ok(sequences)
}

fn is_http_url(input: &str) -> bool {
    input.starts_with("http://") || input.starts_with("https://")
}

fn is_ssh_path(input: &str) -> bool {
    input.starts_with("ssh://")
}

fn open_fasta_reader(input: &str) -> Result<fasta::Reader<paraseq::BoxedReader>> {
    if is_http_url(input) {
        return fasta::Reader::from_url(input).map_err(Into::into);
    }
    if is_ssh_path(input) {
        return fasta::Reader::from_ssh(input).map_err(Into::into);
    }
    fasta::Reader::from_path(Path::new(input)).map_err(Into::into)
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
        let sequences = result.expect("parse should succeed");
        assert_eq!(sequences.len(), 2);
        assert_eq!(sequences[0].id.as_str(), "seq1");
        assert_eq!(sequences[0].sequence.as_slice(), b"A-CG");
        assert_eq!(sequences[1].id.as_str(), "seq2");
        assert_eq!(sequences[1].sequence.as_slice(), b"TGCA");
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
}
