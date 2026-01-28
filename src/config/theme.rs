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
}

impl ThemeId {
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            ThemeId::EverforestDark => "everforest-dark",
        }
    }

    #[must_use]
    pub const fn all() -> [ThemeId; 1] {
        [ThemeId::EverforestDark]
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
    fn dna_colour(&self, byte: u8) -> Option<Color> {
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
    fn amino_acid_colour(&self, byte: u8) -> Option<Color> {
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
    base_bg: rgb(0x2d, 0x35, 0x3b),
    surface_bg: rgb(0x34, 0x3f, 0x44),
    panel_bg: rgb(0x3d, 0x48, 0x4d),
    panel_bg_dim: rgb(0x32, 0x3b, 0x3f),
    overlay_bg: rgb(0x3d, 0x48, 0x4d),
    border: rgb(0x7a, 0x84, 0x78),
    border_active: rgb(0x85, 0x92, 0x89),
    text: rgb(0xd3, 0xc6, 0xaa),
    text_muted: rgb(0x85, 0x92, 0x89),
    text_dim: rgb(0x7a, 0x84, 0x78),
    accent: rgb(0x83, 0xc0, 0x92),
    accent_alt: rgb(0x7f, 0xbb, 0xb3),
    success: rgb(0xa7, 0xc0, 0x80),
    warning: rgb(0xdb, 0xbc, 0x7f),
    error: rgb(0xe6, 0x7e, 0x80),
    selection_bg: rgb(0x7f, 0xbb, 0xb3),
    selection_fg: rgb(0x2d, 0x35, 0x3b),
    sequence: SequenceTheme {
        foreground: rgb(0x2d, 0x35, 0x3b),
        dna: DnaPalette {
            a: rgb(0xa7, 0xc0, 0x80),
            t: rgb(0xe6, 0x7e, 0x80),
            c: rgb(0x7f, 0xbb, 0xb3),
            g: rgb(0xdb, 0xbc, 0x7f),
            n: rgb(0x85, 0x92, 0x89),
            ambiguity: rgb(0xd6, 0x99, 0xb6),
            gap: rgb(0x7a, 0x84, 0x78),
        },
        amino_acid: AminoAcidPalette {
            hydrophobic: rgb(0x7f, 0xbb, 0xb3),
            positive: rgb(0xe6, 0x7e, 0x80),
            negative: rgb(0xd6, 0x99, 0xb6),
            polar: rgb(0xa7, 0xc0, 0x80),
            glycine: rgb(0xe6, 0x98, 0x75),
            proline: rgb(0xdb, 0xbc, 0x7f),
            aromatic: rgb(0x83, 0xc0, 0x92),
            special: rgb(0x9d, 0xa9, 0xa0),
        },
        diff_match: rgb(0x7a, 0x84, 0x78),
    },
};

#[must_use]
pub fn theme_from_id(theme_id: ThemeId) -> Theme {
    match theme_id {
        ThemeId::EverforestDark => EVERFOREST_DARK,
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

const fn rgb(red: u8, green: u8, blue: u8) -> Color {
    Color::Rgb(red, green, blue)
}
