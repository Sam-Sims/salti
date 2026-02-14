use clap::Parser;
use std::path::PathBuf;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StartupState {
    /// Input file path
    pub file_path: Option<PathBuf>,
    /// Initial position in the file to jump to
    pub initial_position: usize,
}

#[derive(Parser, Debug)]
#[command(author, version, about="A modern, fast, multiple sequence alignment browser - built for the terminal.")]
pub struct Cli {
    /// Path to the FASTA alignment file
    #[arg(value_name = "FILE")]
    pub file: Option<PathBuf>,

    /// Initial position in the alignment to jump to (1-based index)
    #[arg(short, long, default_value_t = 1)]
    pub position: usize,

    /// Enable debug logging to `salti.log`
    #[arg(long)]
    pub debug: bool,
}

impl Cli {
    #[must_use]
    pub fn load_startup_sate(self) -> StartupState {
        StartupState {
            file_path: self.file,
            initial_position: self.position.saturating_sub(1),
        }
    }
}
