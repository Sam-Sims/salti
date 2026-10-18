// Everforest light (Medium). Colours from sainnhe/everforest
// Licence: licenses/everforest.LICENSE
#![allow(dead_code, clippy::unreadable_literal)]

use ratatui::style::{Color, Modifier, Style};

use crate::config::{residues::ResidueStyles, theme::Theme};

pub(super) const THEME: Theme = Theme {
    base: Style::new().fg(FG).bg(BG0),
    text: Style::new().fg(FG),
    text_muted: Style::new().fg(GREY2),
    text_dim: Style::new().fg(GREY1),
    accent: Style::new().fg(AQUA).add_modifier(Modifier::BOLD),
    accent_alt: Style::new().fg(BLUE),
    success: Style::new().fg(GREEN).add_modifier(Modifier::BOLD),
    warning: Style::new().fg(ORANGE).add_modifier(Modifier::BOLD),
    error: Style::new().fg(RED).add_modifier(Modifier::BOLD),
    diff_match: Style::new().fg(GREY1),
    border: Style::new().fg(GREY0),
    scrollbar: Style::new().fg(GREY0),
    bar: Style::new().fg(FG).bg(BG2),
    popup: Style::new().fg(FG).bg(BG2),
    popup_inset: Style::new().fg(FG).bg(BG1),
    highlight: Style::new()
        .fg(BG0)
        .bg(STATUSLINE1)
        .add_modifier(Modifier::BOLD),
    selection_rows: BG1,
    selection_columns: BG3,
    selection_ids: AQUA,
    minimap_window: Style::new().fg(BLUE),
    residues: ResidueStyles {
        a: Style::new().fg(DARK_BG0).bg(GREEN),
        t: Style::new().fg(DARK_BG0).bg(RED),
        c: Style::new().fg(DARK_BG0).bg(BLUE),
        g: Style::new().fg(DARK_BG0).bg(YELLOW),
        ambiguity: Style::new().fg(DARK_BG0).bg(PURPLE),
        hydrophobic: Style::new().fg(DARK_BG0).bg(BLUE),
        cysteine: Style::new().fg(PURPLE).bg(BG_RED),
        positive: Style::new().fg(DARK_BG0).bg(RED),
        negative: Style::new().fg(DARK_BG0).bg(PURPLE),
        polar: Style::new().fg(DARK_BG0).bg(GREEN),
        glycine: Style::new().fg(DARK_BG0).bg(ORANGE),
        proline: Style::new().fg(DARK_BG0).bg(YELLOW),
        aromatic: Style::new().fg(DARK_BG0).bg(AQUA),
        gap: Style::new().fg(DARK_BG0).bg(GREY0),
        unknown: Style::new().fg(DARK_BG0).bg(GREY1),
    },
};

const BG_DIM: Color = Color::from_u32(0xefebd4);
const BG0: Color = Color::from_u32(0xfdf6e3);
const BG1: Color = Color::from_u32(0xf4f0d9);
const BG2: Color = Color::from_u32(0xefebd4);
const BG3: Color = Color::from_u32(0xe6e2cc);
const BG4: Color = Color::from_u32(0xe0dcc7);
const BG5: Color = Color::from_u32(0xbdc3af);
const BG_VISUAL: Color = Color::from_u32(0xeaedc8);
const BG_RED: Color = Color::from_u32(0xfde3da);
const BG_YELLOW: Color = Color::from_u32(0xfaedcd);
const BG_GREEN: Color = Color::from_u32(0xf0f1d2);
const BG_BLUE: Color = Color::from_u32(0xe9f0e9);
const BG_PURPLE: Color = Color::from_u32(0xfae8e2);

const FG: Color = Color::from_u32(0x5c6a72);
const RED: Color = Color::from_u32(0xf85552);
const ORANGE: Color = Color::from_u32(0xf57d26);
const YELLOW: Color = Color::from_u32(0xdfa000);
const GREEN: Color = Color::from_u32(0x8da101);
const AQUA: Color = Color::from_u32(0x35a77c);
const BLUE: Color = Color::from_u32(0x3a94c5);
const PURPLE: Color = Color::from_u32(0xdf69ba);
const GREY0: Color = Color::from_u32(0xa6b0a0);
const GREY1: Color = Color::from_u32(0x939f91);
const GREY2: Color = Color::from_u32(0x829181);
const STATUSLINE1: Color = Color::from_u32(0x93b259);
const STATUSLINE2: Color = Color::from_u32(0x708089);
const STATUSLINE3: Color = Color::from_u32(0xe66868);

const DARK_BG0: Color = Color::from_u32(0x2d353b);
