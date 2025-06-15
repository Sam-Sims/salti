use crate::config::options::Options;
use clap::Parser;
use color_eyre::{Result, eyre::eyre};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// Path to the FASTA alignment file
    #[arg(value_name = "FILE")]
    pub file: Option<PathBuf>,

    /// Initial position in the alignment to jump to (1-based index)
    #[arg(short, long, default_value_t = 1)]
    pub position: usize,
}

impl Cli {
    pub fn to_options(&self) -> Result<Options> {
        let mut options = Options::default();

        if let Some(path) = &self.file {
            options.file_path.clone_from(path);
        } else {
            return Err(eyre!(
                "No input file provided. Please specify a FASTA alignment."
            ));
        }
        options.initial_position = self.position.saturating_sub(1);
        Ok(options)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::schemes::ColorScheme;

    #[test]
    fn test_to_options() {
        let cli = Cli {
            file: Some(PathBuf::from("idontexist.fasta")),
            position: 0,
        };

        let result = cli.to_options();
        assert!(result.is_ok());

        let options = result.unwrap();
        assert_eq!(options.file_path, PathBuf::from("idontexist.fasta"));
        assert_eq!(options.initial_position, 0);
        assert_eq!(options.fps, 25.0);
        assert_eq!(options.color_scheme, ColorScheme::Dna);
    }

    #[test]
    fn test_no_file() {
        let cli = Cli {
            file: None,
            position: 1,
        };

        let result = cli.to_options();
        assert!(result.is_err());
    }

    #[test]
    fn test_position_conversion() {
        let cli = Cli {
            file: Some(PathBuf::from("test.fasta")),
            position: 10,
        };

        let options = cli.to_options().unwrap();
        assert_eq!(options.initial_position, 9);
        assert_eq!(options.color_scheme, ColorScheme::Dna);
    }
}
