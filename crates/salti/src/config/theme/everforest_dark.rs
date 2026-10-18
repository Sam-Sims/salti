// Everforest dark (Medium). Colours from sainnhe/everforest
// Licence: licenses/everforest.LICENSE
#![allow(dead_code, clippy::unreadable_literal)]

use ratatui::style::{Color, Modifier, Style};

use crate::config::{residues::ResidueStyles, theme::Theme};

pub(super) const THEME: Theme = Theme {
    base: Style::new().fg(FG).bg(BG0),
    text: Style::new().fg(FG),
    text_muted: Style::new().fg(GREY1),
    text_dim: Style::new().fg(GREY0),
    accent: Style::new().fg(AQUA).add_modifier(Modifier::BOLD),
    accent_alt: Style::new().fg(BLUE),
    success: Style::new().fg(GREEN).add_modifier(Modifier::BOLD),
    warning: Style::new().fg(YELLOW).add_modifier(Modifier::BOLD),
    error: Style::new().fg(RED).add_modifier(Modifier::BOLD),
    diff_match: Style::new().fg(GREY0),
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
    selection_columns: BG2,
    selection_ids: AQUA,
    residues: ResidueStyles {
        a: Style::new().fg(BG0).bg(GREEN),
        t: Style::new().fg(BG0).bg(RED),
        c: Style::new().fg(BG0).bg(BLUE),
        g: Style::new().fg(BG0).bg(YELLOW),
        ambiguity: Style::new().fg(BG0).bg(PURPLE),
        hydrophobic: Style::new().fg(BG0).bg(BLUE),
        cysteine: Style::new().fg(PURPLE).bg(BG_VISUAL),
        positive: Style::new().fg(BG0).bg(RED),
        negative: Style::new().fg(BG0).bg(PURPLE),
        polar: Style::new().fg(BG0).bg(GREEN),
        glycine: Style::new().fg(BG0).bg(ORANGE),
        proline: Style::new().fg(BG0).bg(YELLOW),
        aromatic: Style::new().fg(BG0).bg(AQUA),
        gap: Style::new().fg(BG0).bg(GREY0),
        unknown: Style::new().fg(BG0).bg(GREY1),
    },
};

const BG_DIM: Color = Color::from_u32(0x232a2e);
const BG0: Color = Color::from_u32(0x2d353b);
const BG1: Color = Color::from_u32(0x343f44);
const BG2: Color = Color::from_u32(0x3d484d);
const BG3: Color = Color::from_u32(0x475258);
const BG4: Color = Color::from_u32(0x4f585e);
const BG5: Color = Color::from_u32(0x56635f);
const BG_VISUAL: Color = Color::from_u32(0x543a48);
const BG_RED: Color = Color::from_u32(0x514045);
const BG_YELLOW: Color = Color::from_u32(0x4d4c43);
const BG_GREEN: Color = Color::from_u32(0x425047);
const BG_BLUE: Color = Color::from_u32(0x3a515d);
const BG_PURPLE: Color = Color::from_u32(0x4a444e);

const FG: Color = Color::from_u32(0xd3c6aa);
const RED: Color = Color::from_u32(0xe67e80);
const ORANGE: Color = Color::from_u32(0xe69875);
const YELLOW: Color = Color::from_u32(0xdbbc7f);
const GREEN: Color = Color::from_u32(0xa7c080);
const AQUA: Color = Color::from_u32(0x83c092);
const BLUE: Color = Color::from_u32(0x7fbbb3);
const PURPLE: Color = Color::from_u32(0xd699b6);
const GREY0: Color = Color::from_u32(0x7a8478);
const GREY1: Color = Color::from_u32(0x859289);
const GREY2: Color = Color::from_u32(0x9da9a0);
const STATUSLINE1: Color = Color::from_u32(0xa7c080);
const STATUSLINE2: Color = Color::from_u32(0xd3c6aa);
const STATUSLINE3: Color = Color::from_u32(0xe67e80);
