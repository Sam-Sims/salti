use clap::Parser;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StartupState {
    /// Input source (file path, URL, or SSH path)
    pub file_path: Option<String>,
}

#[derive(Parser, Debug)]
#[command(
    author,
    version,
    about = "A modern, fast, multiple sequence alignment browser - built for the terminal."
)]
pub struct Cli {
    /// Input source: file path, URL (http/https), or SSH path (ssh://)
    #[arg(value_name = "INPUT")]
    pub file: Option<String>,

    /// Enable debug logging to `salti.log`
    #[arg(long)]
    pub debug: bool,
}

impl Cli {
    pub fn load_startup_sate(self) -> StartupState {
        StartupState {
            file_path: self.file,
        }
    }
}
