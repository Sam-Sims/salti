use clap::{Parser, builder::FalseyValueParser};

#[derive(Debug)]
pub struct StartupState {
    /// Input source (file path, URL, or SSH path)
    pub file_path: Option<String>,
    pub update_check: bool,
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

    /// Don't check crates.io for a newer version at startup
    #[arg(long, env = "SALTI_SKIP_UPDATE_CHECK", value_parser = FalseyValueParser::new())]
    pub skip_update_check: bool,
}

impl Cli {
    pub fn startup_state(self) -> StartupState {
        StartupState {
            file_path: self.file,
            update_check: !self.skip_update_check,
        }
    }
}
