use color_eyre::{Result, eyre::eyre};
use needletail::parse_fastx_stdin;
use needletail::parser::parse_fastx_file;
use rand::seq::IndexedRandom;
use std::path::{Path, PathBuf};
use std::sync::Arc;

const SEQUENCE_TYPE_THRESHOLD: f32 = 0.5;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SequenceType {
    Dna,
    AminoAcid,
}

#[derive(Debug, Clone)]
pub struct Alignment {
    pub id: Arc<str>,
    pub sequence: Arc<[u8]>,
}

pub fn detect_sequence_type(alignments: &[Alignment]) -> SequenceType {
    let aa_chars = b"DEFHIKLMNPQRSVWY";
    let sampled_alignments: Vec<_> = alignments.choose_multiple(&mut rand::rng(), 100).collect();

    let (aa_count, total_count) =
        sampled_alignments
            .iter()
            .fold((0, 0), |(aa_count, total_count), alignment| {
                let seq = alignment.sequence.as_ref();
                let aa_in_seq = seq.iter().filter(|&&c| aa_chars.contains(&c)).count();
                let total_in_seq = seq.len();

                (aa_count + aa_in_seq, total_count + total_in_seq)
            });

    if aa_count as f32 / total_count as f32 >= SEQUENCE_TYPE_THRESHOLD {
        SequenceType::AminoAcid
    } else {
        SequenceType::Dna
    }
}

pub async fn parse_fasta_file(path: PathBuf) -> Result<Vec<Alignment>> {
    tokio::task::spawn_blocking(move || -> Result<Vec<Alignment>> {
        if !Path::new(&path).exists() {
            return Err(eyre!("File not found: {:?}", path));
        }

        let mut parser =
            parse_fastx_file(&path).map_err(|e| eyre!("Failed to parse file: {}", e))?;
        let mut alignments = Vec::new();
        let mut expected_len: Option<usize> = None;

        while let Some(rec) = parser.next() {
            let rec = rec.map_err(|e| eyre!("Error reading record: {}", e))?;
            let id = Arc::from(
                std::str::from_utf8(rec.id()).map_err(|e| eyre!("Invalid sequence ID: {}", e))?,
            );

            let seq = Arc::from(rec.seq().to_vec());
            let seq_len = rec.seq().len();
            if let Some(len) = expected_len {
                if seq_len != len {
                    return Err(eyre!(
                        "Sequence length mismatch: expected {}, found {} for id {}",
                        len,
                        seq_len,
                        id
                    ));
                }
            } else {
                expected_len = Some(seq_len);
            }

            alignments.push(Alignment { id, sequence: seq });
        }

        // dont think we can reach this - as parse_fastx_file should return an error if no records are found
        if alignments.is_empty() {
            return Err(eyre!("No valid alignments found in file"));
        }

        Ok(alignments)
    })
    .await?
}

pub async fn parse_fasta_stdin() -> Result<Vec<Alignment>> {
    tokio::task::spawn_blocking(move || -> Result<Vec<Alignment>> {
        let mut parser = parse_fastx_stdin().map_err(|e| eyre!("Failed to parse file: {}", e))?;
        let mut alignments = Vec::new();
        let mut expected_len: Option<usize> = None;

        while let Some(rec) = parser.next() {
            let rec = rec.map_err(|e| eyre!("Error reading record: {}", e))?;
            let id = Arc::from(
                std::str::from_utf8(rec.id()).map_err(|e| eyre!("Invalid sequence ID: {}", e))?,
            );

            let seq = Arc::from(rec.seq().to_vec());
            let seq_len = rec.seq().len();
            if let Some(len) = expected_len {
                if seq_len != len {
                    return Err(eyre!(
                        "Sequence length mismatch: expected {}, found {} for id {}",
                        len,
                        seq_len,
                        id
                    ));
                }
            } else {
                expected_len = Some(seq_len);
            }

            alignments.push(Alignment { id, sequence: seq });
        }

        // dont think we can reach this - as parse_fastx_file should return an error if no records are found
        if alignments.is_empty() {
            return Err(eyre!("No valid alignments found in file"));
        }

        Ok(alignments)
    })
    .await?
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

    #[tokio::test]
    async fn test_parse_valid() {
        let content = ">seq1\nA-CG\n>seq2\nTGCA\n";
        let temp_file = create_temp_fasta(content);
        let result = parse_fasta_file(temp_file.path().to_path_buf()).await;
        assert!(result.is_ok());
        let alignments = result.unwrap();
        assert_eq!(alignments.len(), 2);
        assert_eq!(alignments[0].id.as_ref(), "seq1");
        assert_eq!(alignments[0].sequence.as_ref(), b"A-CG");
        assert_eq!(alignments[1].id.as_ref(), "seq2");
        assert_eq!(alignments[1].sequence.as_ref(), b"TGCA");
    }

    #[tokio::test]
    async fn test_parse_nonexistant() {
        let result = parse_fasta_file(PathBuf::from_str("idontexist.fasta").unwrap()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_parse_empty() {
        let content = "";
        let temp_file = create_temp_fasta(content);
        let result = parse_fasta_file(temp_file.path().to_path_buf()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_parse_no_seqs() {
        let content = ">seq1\n>seq2\n";
        let temp_file = create_temp_fasta(content);
        let result = parse_fasta_file(temp_file.path().to_path_buf()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_parse_length_mismatch() {
        let content = ">seq1\nATCG\n>seq2\nTGCAAA\n";
        let temp_file = create_temp_fasta(content);
        let result = parse_fasta_file(temp_file.path().to_path_buf()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_parse_invalid() {
        let content = "imaninvalidfasta\nfile\n";
        let temp_file = create_temp_fasta(content);
        let result = parse_fasta_file(temp_file.path().to_path_buf()).await;
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
