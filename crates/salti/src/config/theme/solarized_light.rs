// Solarized light. Colours from Solarized
// Licence: licenses/solarized.LICENSE
#![allow(dead_code, clippy::unreadable_literal)]

use ratatui::style::{Color, Modifier, Style};

use crate::config::{residues::ResidueStyles, theme::Theme};

pub(super) const THEME: Theme = Theme {
    base: Style::new().fg(BASE00).bg(BASE3),
    text: Style::new().fg(BASE00),
    text_muted: Style::new().fg(BASE0),
    text_dim: Style::new().fg(BASE1),
    accent: Style::new().fg(BLUE).add_modifier(Modifier::BOLD),
    accent_alt: Style::new().fg(VIOLET),
    success: Style::new().fg(GREEN).add_modifier(Modifier::BOLD),
    warning: Style::new().fg(ORANGE).add_modifier(Modifier::BOLD),
    error: Style::new().fg(RED).add_modifier(Modifier::BOLD),
    diff_match: Style::new().fg(BASE1),
    border: Style::new().fg(BASE1),
    scrollbar: Style::new().fg(VIOLET),
    bar: Style::new().fg(BASE00).bg(BASE2),
    popup: Style::new().fg(BASE00).bg(BASE2),
    popup_inset: Style::new().fg(BASE00).bg(BASE175),
    highlight: Style::new()
        .fg(BASE01)
        .bg(BASE15)
        .add_modifier(Modifier::BOLD),
    selection_rows: BASE2,
    selection_columns: BASE2,
    selection_ids: BLUE,
    minimap_window: Style::new().fg(BASE15),
    minimap_empty: BASE175,
    residues: ResidueStyles {
        a: Style::new().fg(BASE03).bg(GREEN),
        t: Style::new().fg(BASE03).bg(RED),
        c: Style::new().fg(BASE03).bg(CYAN),
        g: Style::new().fg(BASE03).bg(YELLOW),
        ambiguity: Style::new().fg(BASE03).bg(VIOLET),
        hydrophobic: Style::new().fg(BASE03).bg(BLUE),
        cysteine: Style::new().fg(BASE03).bg(MAGENTA),
        positive: Style::new().fg(BASE03).bg(RED),
        negative: Style::new().fg(BASE03).bg(VIOLET),
        polar: Style::new().fg(BASE03).bg(GREEN),
        glycine: Style::new().fg(BASE03).bg(ORANGE),
        proline: Style::new().fg(BASE03).bg(YELLOW),
        aromatic: Style::new().fg(BASE03).bg(CYAN),
        gap: Style::new().fg(BASE03).bg(BASE0),
        unknown: Style::new().fg(BASE03).bg(BASE1),
    },
};

const BASE03: Color = Color::from_u32(0x002b36);
const BASE02: Color = Color::from_u32(0x073642);
const BASE01: Color = Color::from_u32(0x586e75);
const BASE00: Color = Color::from_u32(0x657b83);
const BASE0: Color = Color::from_u32(0x839496);
const BASE1: Color = Color::from_u32(0x93a1a1);
const BASE2: Color = Color::from_u32(0xeee8d5);
const BASE3: Color = Color::from_u32(0xfdf6e3);

const YELLOW: Color = Color::from_u32(0xb58900);
const ORANGE: Color = Color::from_u32(0xcb4b16);
const RED: Color = Color::from_u32(0xdc322f);
const MAGENTA: Color = Color::from_u32(0xd33682);
const VIOLET: Color = Color::from_u32(0x6c71c4);
const BLUE: Color = Color::from_u32(0x268bd2);
const CYAN: Color = Color::from_u32(0x2aa198);
const GREEN: Color = Color::from_u32(0x859900);

const BASE15: Color = Color::from_u32(0xc5c8bd);
const BASE175: Color = Color::from_u32(0xdddbcc);
const BASE25: Color = Color::from_u32(0xf5eedb);
