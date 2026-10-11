use std::path::Path;

use anyhow::{Result, ensure, format_err};
use libmsa::{Alignment, Sequence};
use paraseq::fasta;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info};

pub fn parse_fasta_file(input: &str, cancel: &CancellationToken) -> Result<Alignment> {
    info!(input = %input, "Starting fasta parse");
    let mut reader =
        open_fasta_reader(input).map_err(|error| format_err!("Failed to open input: {error}"))?;
    let mut record_set = reader.new_record_set();
    let mut sequences = Vec::new();

    while record_set
        .fill(&mut reader)
        .map_err(|error| format_err!("Error reading records: {error}"))?
    {
        ensure!(!cancel.is_cancelled(), "Cancelled fasta parse");
        for record in record_set.iter() {
            let record = record.map_err(|error| format_err!("Error reading record: {error}"))?;
            let id = std::str::from_utf8(record.id())
                .map_err(|error| format_err!("Invalid sequence ID: {error}"))?
                .to_string();
            sequences.push(Sequence {
                id,
                residues: record.seq().to_vec(),
            });
        }
    }

    debug!(
        input = %input,
        sequence_count = sequences.len(),
        "Completed fasta parse"
    );

    ensure!(!cancel.is_cancelled(), "Cancelled fasta parse");
    Ok(Alignment::new(sequences)?)
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
    use rstest::rstest;
    use tempfile::NamedTempFile;

    use super::*;

    fn fasta_file(content: &[u8]) -> NamedTempFile {
        let file = NamedTempFile::new().unwrap();
        std::fs::write(file.path(), content).unwrap();
        file
    }

    fn parse(file: &NamedTempFile, cancel: &CancellationToken) -> Result<Alignment> {
        parse_fasta_file(file.path().to_str().unwrap(), cancel)
    }

    #[test]
    fn parse_fasta_file_works() {
        let file = fasta_file(b">seq1\nA-CG\n>seq2\nTGCA\n");

        let alignment = parse(&file, &CancellationToken::new()).unwrap();

        assert_eq!((alignment.row_count(), alignment.width()), (2, 4));
        assert_eq!([alignment.id(0), alignment.id(1)], ["seq1", "seq2"]);
    }

    #[rstest]
    #[case::empty(b"")]
    #[case::not_fasta(b"notfasta\nfile\n")]
    #[case::invalid_utf8_id(b">\xff\nACGT\n")]
    fn parse_fasta_file_rejects(#[case] content: &[u8]) {
        let file = fasta_file(content);

        assert!(parse(&file, &CancellationToken::new()).is_err());
    }

    #[test]
    fn parse_fasta_file_rejects_missing_file() {
        assert!(parse_fasta_file("missing.fasta", &CancellationToken::new()).is_err());
    }

    #[test]
    fn parse_fasta_file_rejects_cancelled() {
        let file = fasta_file(b">seq1\nACGT\n");
        let cancel = CancellationToken::new();
        cancel.cancel();

        assert!(parse(&file, &cancel).is_err());
    }
}
