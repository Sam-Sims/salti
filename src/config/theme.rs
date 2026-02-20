use ratatui::style::{Color, Style};

#[derive(Debug, Clone, Copy)]
pub struct Theme {
    pub base_bg: Color,
    pub surface_bg: Color,
    pub panel_bg: Color,
    pub panel_bg_dim: Color,
    pub overlay_bg: Color,
    pub border: Color,
    pub border_active: Color,
    pub text: Color,
    pub text_muted: Color,
    pub text_dim: Color,
    pub accent: Color,
    pub accent_alt: Color,
    pub success: Color,
    pub warning: Color,
    pub error: Color,
    pub selection_bg: Color,
    pub selection_fg: Color,
    pub sequence: SequenceTheme,
}

#[derive(Debug, Clone, Copy)]
pub struct ThemeStyles {
    pub base_block: Style,
    pub panel_block: Style,
    pub panel_block_dim: Style,
    pub border: Style,
    pub border_active: Style,
    pub text: Style,
    pub text_muted: Style,
    pub text_dim: Style,
    pub accent: Style,
    pub accent_alt: Style,
    pub success: Style,
    pub warning: Style,
    pub error: Style,
    pub selection: Style,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeId {
    EverforestDark,
    SolarizedLight,
    TokyoNight,
    TerminalDefault,
}

impl ThemeId {
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            ThemeId::EverforestDark => "everforest-dark",
            ThemeId::SolarizedLight => "solarized-light",
            ThemeId::TokyoNight => "tokyo-night",
            ThemeId::TerminalDefault => "terminal-default",
        }
    }

    #[must_use]
    pub const fn all() -> [ThemeId; 4] {
        [
            ThemeId::EverforestDark,
            ThemeId::SolarizedLight,
            ThemeId::TokyoNight,
            ThemeId::TerminalDefault,
        ]
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SequenceTheme {
    pub foreground: Color,
    pub dna: DnaPalette,
    pub amino_acid: AminoAcidPalette,
    pub diff_match: Color,
}

#[derive(Debug, Clone, Copy)]
pub struct DnaPalette {
    pub a: Color,
    pub t: Color,
    pub c: Color,
    pub g: Color,
    pub n: Color,
    pub ambiguity: Color,
    pub gap: Color,
}

#[derive(Debug, Clone, Copy)]
pub struct AminoAcidPalette {
    pub hydrophobic: Color,
    pub positive: Color,
    pub negative: Color,
    pub polar: Color,
    pub glycine: Color,
    pub proline: Color,
    pub aromatic: Color,
    pub special: Color,
}

impl SequenceTheme {
    pub fn nucleotide_style(&self, byte: u8) -> Style {
        match self.dna_colour(byte) {
            Some(colour) => Style::new().bg(colour).fg(self.foreground),
            None => Style::new(),
        }
    }

    pub fn amino_acid_style(&self, byte: u8) -> Style {
        match self.amino_acid_colour(byte) {
            Some(colour) => Style::new().bg(colour).fg(self.foreground),
            None => Style::new(),
        }
    }
    #[must_use]
    pub fn dna_colour(&self, byte: u8) -> Option<Color> {
        match byte {
            b'A' | b'a' => Some(self.dna.a),
            b'T' | b't' => Some(self.dna.t),
            b'C' | b'c' => Some(self.dna.c),
            b'G' | b'g' => Some(self.dna.g),
            b'N' | b'n' => Some(self.dna.n),
            b'R' | b'r' | b'Y' | b'y' | b'M' | b'm' | b'K' | b'k' | b'S' | b's' | b'W' | b'w'
            | b'H' | b'h' | b'B' | b'b' | b'V' | b'v' | b'D' | b'd' => Some(self.dna.ambiguity),
            b'-' => Some(self.dna.gap),
            _ => None,
        }
    }

    // colours from clustal default
    // http://www.jalview.org/help/html/colourSchemes/clustal.html
    #[must_use]
    pub fn amino_acid_colour(&self, byte: u8) -> Option<Color> {
        match byte {
            b'A' | b'a' | b'V' | b'v' | b'L' | b'l' | b'I' | b'i' | b'M' | b'm' | b'F' | b'f'
            | b'W' | b'w' | b'C' | b'c' => Some(self.amino_acid.hydrophobic),
            b'Y' | b'y' | b'H' | b'h' => Some(self.amino_acid.aromatic),
            b'S' | b's' | b'T' | b't' | b'N' | b'n' | b'Q' | b'q' => Some(self.amino_acid.polar),
            b'K' | b'k' | b'R' | b'r' => Some(self.amino_acid.positive),
            b'D' | b'd' | b'E' | b'e' => Some(self.amino_acid.negative),
            b'G' | b'g' => Some(self.amino_acid.glycine),
            b'P' | b'p' => Some(self.amino_acid.proline),
            b'-' | b'X' | b'x' => Some(self.amino_acid.special),
            _ => None,
        }
    }
}

pub const EVERFOREST_DARK: Theme = Theme {
    base_bg: Color::from_u32(0x2d353b),
    surface_bg: Color::from_u32(0x343f44),
    panel_bg: Color::from_u32(0x3d484d),
    panel_bg_dim: Color::from_u32(0x323b3f),
    overlay_bg: Color::from_u32(0x3d484d),
    border: Color::from_u32(0x7a8478),
    border_active: Color::from_u32(0x859289),
    text: Color::from_u32(0xd3c6aa),
    text_muted: Color::from_u32(0x859289),
    text_dim: Color::from_u32(0x7a8478),
    accent: Color::from_u32(0x83c092),
    accent_alt: Color::from_u32(0x7fbbb3),
    success: Color::from_u32(0xa7c080),
    warning: Color::from_u32(0xdbbc7f),
    error: Color::from_u32(0xe67e80),
    selection_bg: Color::from_u32(0x7fbbb3),
    selection_fg: Color::from_u32(0x2d353b),
    sequence: SequenceTheme {
        foreground: Color::from_u32(0x2d353b),
        dna: DnaPalette {
            a: Color::from_u32(0xa7c080),
            t: Color::from_u32(0xe67e80),
            c: Color::from_u32(0x7fbbb3),
            g: Color::from_u32(0xdbbc7f),
            n: Color::from_u32(0x859289),
            ambiguity: Color::from_u32(0xd699b6),
            gap: Color::from_u32(0x7a8478),
        },
        amino_acid: AminoAcidPalette {
            hydrophobic: Color::from_u32(0x7fbbb3),
            positive: Color::from_u32(0xe67e80),
            negative: Color::from_u32(0xd699b6),
            polar: Color::from_u32(0xa7c080),
            glycine: Color::from_u32(0xe69875),
            proline: Color::from_u32(0xdbbc7f),
            aromatic: Color::from_u32(0x83c092),
            special: Color::from_u32(0x9da9a0),
        },
        diff_match: Color::from_u32(0x7a8478),
    },
};

pub const SOLARIZED_LIGHT: Theme = Theme {
    base_bg: Color::from_u32(0xfdf6e3),
    surface_bg: Color::from_u32(0xeee8d5),
    panel_bg: Color::from_u32(0xeee8d5),
    panel_bg_dim: Color::from_u32(0xdddbcc),
    overlay_bg: Color::from_u32(0xeee8d5),
    border: Color::from_u32(0x93a1a1),
    border_active: Color::from_u32(0x268bd2),
    text: Color::from_u32(0x586e75),
    text_muted: Color::from_u32(0x93a1a1),
    text_dim: Color::from_u32(0x657b83),
    accent: Color::from_u32(0x268bd2),
    accent_alt: Color::from_u32(0x6c71c4),
    success: Color::from_u32(0x859900),
    warning: Color::from_u32(0xcb4b16),
    error: Color::from_u32(0xdc322f),
    selection_bg: Color::from_u32(0xc5c8bd),
    selection_fg: Color::from_u32(0x586e75),
    sequence: SequenceTheme {
        foreground: Color::from_u32(0x002b36),
        dna: DnaPalette {
            a: Color::from_u32(0x859900),
            t: Color::from_u32(0xdc322f),
            c: Color::from_u32(0x2aa198),
            g: Color::from_u32(0xb58900),
            n: Color::from_u32(0x93a1a1),
            ambiguity: Color::from_u32(0x6c71c4),
            gap: Color::from_u32(0x839496),
        },
        amino_acid: AminoAcidPalette {
            hydrophobic: Color::from_u32(0x268bd2),
            positive: Color::from_u32(0xdc322f),
            negative: Color::from_u32(0xd33682),
            polar: Color::from_u32(0x859900),
            glycine: Color::from_u32(0xcb4b16),
            proline: Color::from_u32(0xb58900),
            aromatic: Color::from_u32(0x6c71c4),
            special: Color::from_u32(0x93a1a1),
        },
        diff_match: Color::from_u32(0x93a1a1),
    },
};

pub const TOKYO_NIGHT: Theme = Theme {
    base_bg: Color::from_u32(0x1a1b26),
    surface_bg: Color::from_u32(0x292e42),
    panel_bg: Color::from_u32(0x16161e),
    panel_bg_dim: Color::from_u32(0x343a55),
    overlay_bg: Color::from_u32(0x16161e),
    border: Color::from_u32(0x15161e),
    border_active: Color::from_u32(0x27a1b9),
    text: Color::from_u32(0xc0caf5),
    text_muted: Color::from_u32(0x565f89),
    text_dim: Color::from_u32(0x3b4261),
    accent: Color::from_u32(0x7dcfff),
    accent_alt: Color::from_u32(0x7aa2f7),
    success: Color::from_u32(0x9ece6a),
    warning: Color::from_u32(0xe0af68),
    error: Color::from_u32(0xdb4b4b),
    selection_bg: Color::from_u32(0x283457),
    selection_fg: Color::from_u32(0xc0caf5),
    sequence: SequenceTheme {
        foreground: Color::from_u32(0x1a1b26),
        dna: DnaPalette {
            a: Color::from_u32(0x9ece6a),
            t: Color::from_u32(0xf7768e),
            c: Color::from_u32(0x7dcfff),
            g: Color::from_u32(0xe0af68),
            n: Color::from_u32(0x737aa2),
            ambiguity: Color::from_u32(0xbb9af7),
            gap: Color::from_u32(0x3b4261),
        },
        amino_acid: AminoAcidPalette {
            hydrophobic: Color::from_u32(0x7aa2f7),
            positive: Color::from_u32(0xf7768e),
            negative: Color::from_u32(0xbb9af7),
            polar: Color::from_u32(0x73daca),
            glycine: Color::from_u32(0xff9e64),
            proline: Color::from_u32(0xe0af68),
            aromatic: Color::from_u32(0x2ac3de),
            special: Color::from_u32(0x565f89),
        },
        diff_match: Color::from_u32(0x6183bb),
    },
};

pub const TERMINAL_DEFAULT: Theme = Theme {
    base_bg: Color::Reset,
    surface_bg: Color::Black,
    panel_bg: Color::Black,
    panel_bg_dim: Color::DarkGray,
    overlay_bg: Color::Black,
    border: Color::DarkGray,
    border_active: Color::Gray,
    text: Color::Reset,
    text_muted: Color::Gray,
    text_dim: Color::DarkGray,
    accent: Color::Cyan,
    accent_alt: Color::Blue,
    success: Color::Green,
    warning: Color::Yellow,
    error: Color::Red,
    selection_bg: Color::Blue,
    selection_fg: Color::Black,
    sequence: SequenceTheme {
        foreground: Color::Black,
        dna: DnaPalette {
            a: Color::Green,
            t: Color::Red,
            c: Color::Blue,
            g: Color::Yellow,
            n: Color::Gray,
            ambiguity: Color::Magenta,
            gap: Color::DarkGray,
        },
        amino_acid: AminoAcidPalette {
            hydrophobic: Color::Blue,
            positive: Color::Red,
            negative: Color::Magenta,
            polar: Color::Green,
            glycine: Color::Cyan,
            proline: Color::Yellow,
            aromatic: Color::LightGreen,
            special: Color::Gray,
        },
        diff_match: Color::DarkGray,
    },
};

#[must_use]
pub fn theme_from_id(theme_id: ThemeId) -> Theme {
    match theme_id {
        ThemeId::EverforestDark => EVERFOREST_DARK,
        ThemeId::SolarizedLight => SOLARIZED_LIGHT,
        ThemeId::TokyoNight => TOKYO_NIGHT,
        ThemeId::TerminalDefault => TERMINAL_DEFAULT,
    }
}

#[must_use]
pub fn build_theme_styles(theme: Theme) -> ThemeStyles {
    ThemeStyles {
        base_block: Style::new().bg(theme.base_bg).fg(theme.text),
        panel_block: Style::new().bg(theme.panel_bg).fg(theme.text),
        panel_block_dim: Style::new().bg(theme.panel_bg_dim).fg(theme.text),
        border: Style::new().fg(theme.border),
        border_active: Style::new().fg(theme.border_active),
        text: Style::new().fg(theme.text),
        text_muted: Style::new().fg(theme.text_muted),
        text_dim: Style::new().fg(theme.text_dim),
        accent: Style::new().fg(theme.accent).bold(),
        accent_alt: Style::new().fg(theme.accent_alt),
        success: Style::new().fg(theme.success).bold(),
        warning: Style::new().fg(theme.warning).bold(),
        error: Style::new().fg(theme.error).bold(),
        selection: Style::new()
            .bg(theme.selection_bg)
            .fg(theme.selection_fg)
            .bold(),
    }
}
