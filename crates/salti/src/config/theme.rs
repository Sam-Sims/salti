mod ansi;
mod everforest_dark;
mod everforest_light;
mod solarized_dark;
mod solarized_light;
mod tokyo_night;

use std::{iter, str::FromStr};

use ratatui::style::{Color, Style};
use strum::VariantArray;
use terminal_colorsaurus::{QueryOptions, ThemeMode};
use tracing::warn;

use super::residues::ResidueStyles;

#[derive(Debug)]
pub struct Theme {
    pub base: Style,
    pub text: Style,
    pub text_muted: Style,
    pub text_dim: Style,
    pub accent: Style,
    pub accent_alt: Style,
    pub success: Style,
    pub warning: Style,
    pub error: Style,
    pub diff_match: Style,
    pub border: Style,
    pub scrollbar: Style,
    pub bar: Style,
    pub popup: Style,
    pub popup_inset: Style,
    pub highlight: Style,
    pub selection_rows: Color,
    pub selection_columns: Color,
    pub selection_ids: Color,
    pub residues: ResidueStyles,
}

impl Theme {
    pub fn builtin(name: ThemeName) -> Self {
        match name {
            ThemeName::EverforestDark => everforest_dark::THEME,
            ThemeName::EverforestLight => everforest_light::THEME,
            ThemeName::SolarizedDark => solarized_dark::THEME,
            ThemeName::SolarizedLight => solarized_light::THEME,
            ThemeName::TokyoNight => tokyo_night::THEME,
            ThemeName::Ansi => ansi::THEME,
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::builtin(ThemeName::default())
    }
}

#[derive(
    Debug,
    Default,
    Clone,
    Copy,
    PartialEq,
    Eq,
    strum::EnumString,
    strum::IntoStaticStr,
    strum::VariantArray,
)]
#[strum(serialize_all = "kebab-case")]
pub enum ThemeName {
    #[default]
    EverforestDark,
    EverforestLight,
    SolarizedDark,
    SolarizedLight,
    TokyoNight,
    Ansi,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeChoice {
    Auto,
    Named(ThemeName),
}

impl ThemeChoice {
    pub fn names() -> impl Iterator<Item = &'static str> {
        iter::once("auto").chain(ThemeName::VARIANTS.iter().map(|&name| name.into()))
    }

    pub fn resolve(self, auto: Option<ThemeName>) -> ThemeName {
        match self {
            Self::Named(name) => name,
            Self::Auto => auto.unwrap_or_default(),
        }
    }
}

impl FromStr for ThemeChoice {
    type Err = strum::ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "auto" => Ok(Self::Auto),
            name => name.parse().map(Self::Named),
        }
    }
}

pub fn auto_theme(choice: ThemeChoice) -> Option<ThemeName> {
    match choice {
        ThemeChoice::Auto => terminal_colorsaurus::theme_mode(QueryOptions::default())
            .inspect_err(|error_value| {
                warn!(error = ?error_value, "Failed to detect the terminal theme mode");
            })
            .ok()
            .map(|mode| match mode {
                ThemeMode::Dark => ThemeName::EverforestDark,
                ThemeMode::Light => ThemeName::EverforestLight,
            }),
        ThemeChoice::Named(_) => None,
    }
}
