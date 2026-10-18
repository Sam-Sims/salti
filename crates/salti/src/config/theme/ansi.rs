#![allow(dead_code, clippy::unreadable_literal)]

use ratatui::style::{Color, Modifier, Style};

use crate::config::{residues::ResidueStyles, theme::Theme};

pub(super) const THEME: Theme = Theme {
    base: Style::new().fg(DEFAULT).bg(DEFAULT),
    text: Style::new().fg(DEFAULT),
    text_muted: Style::new().fg(DEFAULT).add_modifier(Modifier::DIM),
    text_dim: Style::new().fg(DEFAULT).add_modifier(Modifier::DIM),
    accent: Style::new().fg(CYAN).add_modifier(Modifier::BOLD),
    accent_alt: Style::new().fg(BLUE),
    success: Style::new().fg(GREEN).add_modifier(Modifier::BOLD),
    warning: Style::new().fg(YELLOW).add_modifier(Modifier::BOLD),
    error: Style::new().fg(RED).add_modifier(Modifier::BOLD),
    diff_match: Style::new().fg(DEFAULT).add_modifier(Modifier::DIM),
    border: Style::new().fg(DEFAULT).add_modifier(Modifier::DIM),
    scrollbar: Style::new().fg(BLUE),
    bar: Style::new().fg(DEFAULT).bg(DEFAULT),
    popup: Style::new().fg(DEFAULT).bg(DEFAULT),
    popup_inset: Style::new().fg(DEFAULT).bg(DEFAULT),
    highlight: Style::new()
        .add_modifier(Modifier::REVERSED)
        .add_modifier(Modifier::BOLD),
    selection_rows: DEFAULT,
    selection_columns: DEFAULT,
    selection_ids: DEFAULT,
    residues: ResidueStyles {
        a: Style::new().fg(BLACK).bg(GREEN),
        t: Style::new().fg(BLACK).bg(RED),
        c: Style::new().fg(BLACK).bg(BLUE),
        g: Style::new().fg(BLACK).bg(YELLOW),
        ambiguity: Style::new().fg(BLACK).bg(MAGENTA),
        hydrophobic: Style::new().fg(BLACK).bg(BLUE),
        cysteine: Style::new().fg(BLACK).bg(BRIGHT_MAGENTA),
        positive: Style::new().fg(BLACK).bg(RED),
        negative: Style::new().fg(BLACK).bg(MAGENTA),
        polar: Style::new().fg(BLACK).bg(GREEN),
        glycine: Style::new().fg(BLACK).bg(CYAN),
        proline: Style::new().fg(BLACK).bg(YELLOW),
        aromatic: Style::new().fg(BLACK).bg(BRIGHT_GREEN),
        gap: Style::new().fg(BLACK).bg(BRIGHT_BLACK),
        unknown: Style::new().fg(DEFAULT).add_modifier(Modifier::DIM),
    },
};

const DEFAULT: Color = Color::Reset;

const BLACK: Color = Color::Black;
const RED: Color = Color::Red;
const GREEN: Color = Color::Green;
const YELLOW: Color = Color::Yellow;
const BLUE: Color = Color::Blue;
const MAGENTA: Color = Color::Magenta;
const CYAN: Color = Color::Cyan;
const WHITE: Color = Color::Gray;

const BRIGHT_BLACK: Color = Color::DarkGray;
const BRIGHT_RED: Color = Color::LightRed;
const BRIGHT_GREEN: Color = Color::LightGreen;
const BRIGHT_YELLOW: Color = Color::LightYellow;
const BRIGHT_BLUE: Color = Color::LightBlue;
const BRIGHT_MAGENTA: Color = Color::LightMagenta;
const BRIGHT_CYAN: Color = Color::LightCyan;
const BRIGHT_WHITE: Color = Color::White;
