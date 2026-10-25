use clap::{
    Parser,
    builder::{FalseyValueParser, PossibleValuesParser, TypedValueParser},
};

use crate::config::theme::{self, Theme, ThemeChoice, ThemeName};

#[derive(Parser, Debug)]
#[command(
    author,
    version,
    about = "A modern, fast, multiple sequence alignment browser - built for the terminal."
)]
pub struct Cli {
    /// Input source: file path, URL (http/https), or SSH path (ssh://)
    pub input: Option<String>,

    /// Enable debug logging to salti.log
    #[arg(long)]
    pub debug: bool,

    /// Don't check crates.io for a newer version at startup
    #[arg(long, env = "SALTI_SKIP_UPDATE_CHECK", value_parser = FalseyValueParser::new())]
    pub skip_update_check: bool,

    /// Theme to start with: auto picks a light or dark theme to match the terminal
    #[arg(long, env = "SALTI_THEME", default_value = "auto", value_parser = theme_parser())]
    pub theme: ThemeChoice,
}

impl Cli {
    pub fn into_startup_state(self) -> StartupState {
        let auto_theme = theme::auto_theme();
        StartupState {
            input: self.input,
            update_check: !self.skip_update_check,
            theme: Theme::builtin(self.theme.resolve(auto_theme)),
            auto_theme,
        }
    }
}

#[derive(Debug)]
pub struct StartupState {
    /// a file path, URL or SSH path
    pub input: Option<String>,
    pub update_check: bool,
    pub theme: Theme,
    pub auto_theme: Option<ThemeName>,
}

fn theme_parser() -> impl TypedValueParser<Value = ThemeChoice> {
    PossibleValuesParser::new(ThemeChoice::names())
        .map(|name| name.parse().expect("possible values are theme choices"))
}
