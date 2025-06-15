use crate::parser::SequenceType;
use ratatui::prelude::{Color, Span, Style, Stylize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColorScheme {
    #[default]
    Dna,
    DnaBackground,
    AminoAcid,
    AminoAcidBackground,
}

impl ColorScheme {
    pub fn name(self) -> &'static str {
        match self {
            ColorScheme::Dna => "DNA",
            ColorScheme::DnaBackground => "DNA - Background",
            ColorScheme::AminoAcid => "Amino Acid",
            ColorScheme::AminoAcidBackground => "Amino Acid - Background",
        }
    }

    pub fn all() -> [ColorScheme; 4] {
        [
            ColorScheme::Dna,
            ColorScheme::DnaBackground,
            ColorScheme::AminoAcid,
            ColorScheme::AminoAcidBackground,
        ]
    }

    pub fn get_default_scheme(sequence_type: SequenceType) -> Self {
        match sequence_type {
            SequenceType::Dna => ColorScheme::Dna,
            SequenceType::AminoAcid => ColorScheme::AminoAcid,
        }
    }

    pub fn cycle(current: Self) -> Self {
        let schemes = Self::all();
        let current_index = schemes.iter().position(|&s| s == current).unwrap_or(0);
        let next_index = (current_index + 1) % schemes.len();
        schemes[next_index]
    }
}


fn format_nucleotide(byte: u8) -> Option<Color> {
    match byte {
        b'A' | b'a' => Some(Color::Green),
        b'T' | b't' => Some(Color::Red),
        b'C' | b'c' => Some(Color::Blue),
        b'G' | b'g' => Some(Color::Yellow),
        b'N' | b'n' => Some(Color::DarkGray),
        b'R' | b'r' | b'Y' | b'y' | b'M' | b'm' | b'K' | b'k' | b'S' | b's' | b'W' | b'w'
        | b'H' | b'h' | b'B' | b'b' | b'V' | b'v' | b'D' | b'd' => Some(Color::LightMagenta),
        b'-' => Some(Color::Gray),
        _ => None,
    }
}

// colours from clustal default
// http://www.jalview.org/help/html/colourSchemes/clustal.html
const HYDROPHOBIC: ratatui::prelude::Color = Color::LightBlue;
const POSITIVE_CHARGE: ratatui::prelude::Color = Color::LightRed;
const NEGATIVE_CHARGE: ratatui::prelude::Color = Color::LightMagenta;
const POLAR: ratatui::prelude::Color = Color::LightGreen;
// const CYSTEINE: ratatui::prelude::Color = Color::Rgb(255, 182, 193);
const GLYCINES: ratatui::prelude::Color = Color::Rgb(255, 170, 72);
const PROLINE: ratatui::prelude::Color = Color::LightYellow;
const AROMATIC: ratatui::prelude::Color = Color::LightCyan;
const SPECIAL: ratatui::prelude::Color = Color::White;

fn format_amino_acid(byte: u8) -> Option<Color> {
    match byte {
        b'A' | b'a' | b'V' | b'v' | b'L' | b'l' | b'I' | b'i' | b'M' | b'm' | b'F' | b'f' | b'W' | b'w' | b'C' | b'c' => Some(HYDROPHOBIC),
        b'Y' | b'y' | b'H' | b'h' => Some(AROMATIC),
        b'S' | b's' | b'T' | b't' | b'N' | b'n' | b'Q' | b'q' => Some(POLAR),
        b'K' | b'k' | b'R' | b'r' => Some(POSITIVE_CHARGE),
        b'D' | b'd' | b'E' | b'e' => Some(NEGATIVE_CHARGE),
        b'G' | b'g' => Some(GLYCINES),
        b'P' | b'p' => Some(PROLINE),
        b'-' | b'X' | b'x' => Some(SPECIAL),
        _ => None,
    }
}

// https://kevinlynagh.com/notes/match-vs-lookup/
static BYTE_TO_CHAR: [&str; 256] = [
    "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?",
    "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", " ", "!", "\"", "#", "$", "%",
    "&", "'", "(", ")", "*", "+", ",", "-", ".", "/", "0", "1", "2", "3", "4", "5", "6", "7", "8",
    "9", ":", ";", "<", "=", ">", "?", "@", "A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K",
    "L", "M", "N", "O", "P", "Q", "R", "S", "T", "U", "V", "W", "X", "Y", "Z", "[", "\\", "]", "^",
    "_", "`", "a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "l", "m", "n", "o", "p", "q",
    "r", "s", "t", "u", "v", "w", "x", "y", "z", "{", "|", "}", "~", "?", "?", "?", "?", "?", "?",
    "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?",
    "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?",
    "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?",
    "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?",
    "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?",
    "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?", "?",
    "?", "?", "?", "?", "?", "?", "?", "?", "?",
];

#[inline]
pub fn get_nucleotide_style(byte: u8, scheme: ColorScheme) -> Style {
    let color = match scheme {
        ColorScheme::Dna | ColorScheme::DnaBackground => format_nucleotide(byte),
        ColorScheme::AminoAcid | ColorScheme::AminoAcidBackground => format_amino_acid(byte),
    };

    match color {
        Some(c) => {
            if matches!(
                scheme,
                ColorScheme::DnaBackground | ColorScheme::AminoAcidBackground
            ) {
                Style::default().bg(c).fg(Color::Black)
            } else {
                Style::default().fg(c).bold()
            }
        }
        None => Style::default(),
    }
}

#[inline]
pub fn format_sequence_bytes(sequence: &[u8], scheme: ColorScheme) -> Vec<Span<'static>> {
    sequence
        .iter()
        .map(|&b| {
            let ch = BYTE_TO_CHAR[b as usize];
            Span::styled(ch, get_nucleotide_style(b, scheme))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::prelude::Color;

    #[test]
    fn test_format_with_background_scheme() {
        let sequence = b"ATCG-NX";
        let spans = format_sequence_bytes(sequence, ColorScheme::DnaBackground);
        assert_eq!(spans[0].style.bg, Some(Color::Green));
        assert_eq!(spans[0].style.fg, Some(Color::Black));
        assert_eq!(spans[1].style.bg, Some(Color::Red));
        assert_eq!(spans[1].style.fg, Some(Color::Black));
        assert_eq!(spans[2].style.bg, Some(Color::Blue));
        assert_eq!(spans[2].style.fg, Some(Color::Black));
        assert_eq!(spans[3].style.bg, Some(Color::Yellow));
        assert_eq!(spans[3].style.fg, Some(Color::Black));
        assert_eq!(spans[4].style.bg, Some(Color::Gray));
        assert_eq!(spans[4].style.fg, Some(Color::Black));
        assert_eq!(spans[5].style.bg, Some(Color::DarkGray));
        assert_eq!(spans[5].style.fg, Some(Color::Black));
        assert_eq!(spans[6].style.bg, None);
    }

    #[test]
    fn test_format_with_foreground_scheme() {
        let sequence = b"ATCG-NX";
        let spans = format_sequence_bytes(sequence, ColorScheme::Dna);
        assert_eq!(spans[0].style.fg, Some(Color::Green));
        assert_eq!(spans[0].style.bg, None);
        assert_eq!(spans[1].style.fg, Some(Color::Red));
        assert_eq!(spans[1].style.bg, None);
        assert_eq!(spans[2].style.fg, Some(Color::Blue));
        assert_eq!(spans[2].style.bg, None);
        assert_eq!(spans[3].style.fg, Some(Color::Yellow));
        assert_eq!(spans[3].style.bg, None);
        assert_eq!(spans[4].style.fg, Some(Color::Gray));
        assert_eq!(spans[4].style.bg, None);
        assert_eq!(spans[5].style.fg, Some(Color::DarkGray));
        assert_eq!(spans[5].style.bg, None);
        assert_eq!(spans[6].style.fg, None);
    }

    #[test]
    fn test_scheme_cycling() {
        let mut scheme = ColorScheme::Dna;
        assert_eq!(scheme, ColorScheme::Dna);

        scheme = ColorScheme::cycle(scheme);
        assert_eq!(scheme, ColorScheme::DnaBackground);

        scheme = ColorScheme::cycle(scheme);
        assert_eq!(scheme, ColorScheme::AminoAcid);

        scheme = ColorScheme::cycle(scheme);
        assert_eq!(scheme, ColorScheme::AminoAcidBackground);
    }
}
