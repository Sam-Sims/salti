use crate::config::options::Options;
use crate::config::schemes::ColorSchemeType;
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

    /// Color scheme to use for nucleotide visualization
    #[arg(short, long, default_value = "standard")]
    pub color_scheme: String,
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

        // Parse color scheme
        options.color_scheme = match self.color_scheme.to_lowercase().as_str() {
            "standard" => ColorSchemeType::Standard,
            "background" => ColorSchemeType::Background,
            _ => {
                return Err(eyre!(
                    "Invalid color scheme '{}'. Available options: standard, background",
                    self.color_scheme
                ));
            }
        };

        Ok(options)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_options() {
        let cli = Cli {
            file: Some(PathBuf::from("idontexist.fasta")),
            position: 0,
            color_scheme: "standard".to_string(),
        };

        let result = cli.to_options();
        assert!(result.is_ok());

        let options = result.unwrap();
        assert_eq!(options.file_path, PathBuf::from("idontexist.fasta"));
        assert_eq!(options.initial_position, 0);
        assert_eq!(options.fps, 25.0);
        assert_eq!(options.color_scheme, ColorSchemeType::Standard);
    }

    #[test]
    fn test_no_file() {
        let cli = Cli {
            file: None,
            position: 1,
            color_scheme: "standard".to_string(),
        };

        let result = cli.to_options();
        assert!(result.is_err());
    }

    #[test]
    fn test_position_conversion() {
        let cli = Cli {
            file: Some(PathBuf::from("test.fasta")),
            position: 10,
            color_scheme: "background".to_string(),
        };

        let options = cli.to_options().unwrap();
        assert_eq!(options.initial_position, 9);
        assert_eq!(options.color_scheme, ColorSchemeType::Background);
    }

    #[test]
    fn test_invalid_color_scheme() {
        let cli = Cli {
            file: Some(PathBuf::from("test.fasta")),
            position: 1,
            color_scheme: "invalid".to_string(),
        };

        let result = cli.to_options();
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Invalid color scheme")
        );
    }
}
