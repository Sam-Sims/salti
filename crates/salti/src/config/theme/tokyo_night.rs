// Tokyo Night. Colours from folke/tokyonight.nvim
// Licence: licenses/tokyonight.LICENSE
#![allow(dead_code, clippy::unreadable_literal)]

use ratatui::style::{Color, Modifier, Style};

use crate::config::{residues::ResidueStyles, theme::Theme};

pub(super) const THEME: Theme = Theme {
    base: Style::new().fg(FG).bg(BG),
    text: Style::new().fg(FG),
    text_muted: Style::new().fg(DARK5),
    text_dim: Style::new().fg(COMMENT),
    accent: Style::new().fg(CYAN).add_modifier(Modifier::BOLD),
    accent_alt: Style::new().fg(BLUE),
    success: Style::new().fg(GREEN).add_modifier(Modifier::BOLD),
    warning: Style::new().fg(YELLOW).add_modifier(Modifier::BOLD),
    error: Style::new().fg(RED1).add_modifier(Modifier::BOLD),
    diff_match: Style::new().fg(GIT_CHANGE),
    border: Style::new().fg(FG_GUTTER),
    scrollbar: Style::new().fg(BLUE),
    bar: Style::new().fg(FG).bg(BG_DARK),
    popup: Style::new().fg(FG).bg(BG_DARK),
    popup_inset: Style::new().fg(FG).bg(FG_SELECTED),
    highlight: Style::new()
        .fg(FG)
        .bg(BG_VISUAL)
        .add_modifier(Modifier::BOLD),
    selection_rows: BG_HIGHLIGHT,
    selection_columns: BG_DARK,
    selection_ids: CYAN,
    residues: ResidueStyles {
        a: Style::new().fg(BG).bg(GREEN),
        t: Style::new().fg(BG).bg(RED),
        c: Style::new().fg(BG).bg(CYAN),
        g: Style::new().fg(BG).bg(YELLOW),
        ambiguity: Style::new().fg(BG).bg(MAGENTA),
        hydrophobic: Style::new().fg(BG).bg(BLUE),
        cysteine: Style::new().fg(BG).bg(MAGENTA2),
        positive: Style::new().fg(BG).bg(RED),
        negative: Style::new().fg(BG).bg(MAGENTA),
        polar: Style::new().fg(BG).bg(GREEN1),
        glycine: Style::new().fg(BG).bg(ORANGE),
        proline: Style::new().fg(BG).bg(YELLOW),
        aromatic: Style::new().fg(BG).bg(BLUE1),
        gap: Style::new().fg(BG).bg(FG_GUTTER),
        unknown: Style::new().fg(BG).bg(DARK5),
    },
};

const BG: Color = Color::from_u32(0x1a1b26);
const BG_DARK: Color = Color::from_u32(0x16161e);
const BG_DARK1: Color = Color::from_u32(0x0c0e14);
const BG_HIGHLIGHT: Color = Color::from_u32(0x292e42);
const BLUE: Color = Color::from_u32(0x7aa2f7);
const BLUE0: Color = Color::from_u32(0x3d59a1);
const BLUE1: Color = Color::from_u32(0x2ac3de);
const BLUE2: Color = Color::from_u32(0x0db9d7);
const BLUE5: Color = Color::from_u32(0x89ddff);
const BLUE6: Color = Color::from_u32(0xb4f9f8);
const BLUE7: Color = Color::from_u32(0x394b70);
const COMMENT: Color = Color::from_u32(0x565f89);
const CYAN: Color = Color::from_u32(0x7dcfff);
const DARK3: Color = Color::from_u32(0x545c7e);
const DARK5: Color = Color::from_u32(0x737aa2);
const FG: Color = Color::from_u32(0xc0caf5);
const FG_DARK: Color = Color::from_u32(0xa9b1d6);
const FG_GUTTER: Color = Color::from_u32(0x3b4261);
const GREEN: Color = Color::from_u32(0x9ece6a);
const GREEN1: Color = Color::from_u32(0x73daca);
const GREEN2: Color = Color::from_u32(0x41a6b5);
const MAGENTA: Color = Color::from_u32(0xbb9af7);
const MAGENTA2: Color = Color::from_u32(0xff007c);
const ORANGE: Color = Color::from_u32(0xff9e64);
const PURPLE: Color = Color::from_u32(0x9d7cd8);
const RED: Color = Color::from_u32(0xf7768e);
const RED1: Color = Color::from_u32(0xdb4b4b);
const TEAL: Color = Color::from_u32(0x1abc9c);
const TERMINAL_BLACK: Color = Color::from_u32(0x414868);
const YELLOW: Color = Color::from_u32(0xe0af68);
const GIT_ADD: Color = Color::from_u32(0x449dab);
const GIT_CHANGE: Color = Color::from_u32(0x6183bb);
const GIT_DELETE: Color = Color::from_u32(0x914c54);

const BLACK: Color = Color::from_u32(0x15161e);
const BORDER_HIGHLIGHT: Color = Color::from_u32(0x27a1b9);
const BG_VISUAL: Color = Color::from_u32(0x283457);
const DIFF_ADD: Color = Color::from_u32(0x243e4a);
const DIFF_DELETE: Color = Color::from_u32(0x4a272f);
const DIFF_CHANGE: Color = Color::from_u32(0x1f2231);

const FG_SELECTED: Color = Color::from_u32(0x343a55);
