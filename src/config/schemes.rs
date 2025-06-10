use ratatui::prelude::{Color, Span, Style};

pub trait ColorScheme {
    fn get_nucleotide_style(&self, byte: u8) -> Style;
}

#[derive(Debug, Clone, Default)]
pub struct StandardScheme;

impl ColorScheme for StandardScheme {
    fn get_nucleotide_style(&self, byte: u8) -> Style {
        match byte {
            b'A' | b'a' => Style::default().fg(Color::Green),
            b'T' | b't' => Style::default().fg(Color::Red),
            b'C' | b'c' => Style::default().fg(Color::Blue),
            b'G' | b'g' => Style::default().fg(Color::Yellow),
            b'N' | b'n' => Style::default().fg(Color::DarkGray),
            // IUPAC ambiguity codes
            b'R' | b'r' | b'Y' | b'y' | b'M' | b'm' | b'K' | b'k' | b'S' | b's' | b'W' | b'w'
            | b'H' | b'h' | b'B' | b'b' | b'V' | b'v' | b'D' | b'd' => {
                Style::default().fg(Color::LightMagenta)
            }
            b'-' => Style::default().fg(Color::Gray),
            _ => Style::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct BackgroundScheme;

impl ColorScheme for BackgroundScheme {
    fn get_nucleotide_style(&self, byte: u8) -> Style {
        match byte {
            b'A' | b'a' => Style::default().bg(Color::Green).fg(Color::Black),
            b'T' | b't' => Style::default().bg(Color::Red).fg(Color::Black),
            b'C' | b'c' => Style::default().bg(Color::Blue).fg(Color::Black),
            b'G' | b'g' => Style::default().bg(Color::Yellow).fg(Color::Black),
            b'N' | b'n' => Style::default().bg(Color::DarkGray).fg(Color::Black),
            // IUPAC ambiguity codes
            b'R' | b'r' | b'Y' | b'y' | b'M' | b'm' | b'K' | b'k' | b'S' | b's' | b'W' | b'w'
            | b'H' | b'h' | b'B' | b'b' | b'V' | b'v' | b'D' | b'd' => {
                Style::default().bg(Color::LightMagenta).fg(Color::Black)
            }
            b'-' => Style::default().bg(Color::Gray).fg(Color::Black),
            _ => Style::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorSchemeType {
    Standard,
    Background,
}

impl Default for ColorSchemeType {
    fn default() -> Self {
        Self::Standard
    }
}

impl ColorSchemeType {
    pub fn all() -> [ColorSchemeType; 2] {
        [ColorSchemeType::Standard, ColorSchemeType::Background]
    }

    pub fn create_scheme(self) -> Box<dyn ColorScheme> {
        match self {
            ColorSchemeType::Standard => Box::new(StandardScheme),
            ColorSchemeType::Background => Box::new(BackgroundScheme),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ColorSchemeFormatter {
    active_scheme: ColorSchemeType,
    cached_styles: [Style; 256],
}

impl ColorSchemeFormatter {
    pub fn new(scheme: ColorSchemeType) -> Self {
        let mut manager = Self {
            active_scheme: scheme,
            cached_styles: [Style::default(); 256],
        };
        manager.rebuild_cache();
        manager
    }

    fn rebuild_cache(&mut self) {
        let scheme = self.active_scheme.create_scheme();
        for i in 0..256 {
            self.cached_styles[i] = scheme.get_nucleotide_style(i as u8);
        }
    }

    pub fn active_scheme(&self) -> ColorSchemeType {
        self.active_scheme
    }

    pub fn set_scheme(&mut self, scheme: ColorSchemeType) {
        self.active_scheme = scheme;
        self.rebuild_cache();
    }

    #[inline]
    pub fn get_nucleotide_style(&self, byte: u8) -> Style {
        self.cached_styles[byte as usize]
    }
    pub fn available_schemes() -> [ColorSchemeType; 2] {
        ColorSchemeType::all()
    }

    pub fn cycle_scheme(&mut self) {
        let schemes = Self::available_schemes();
        let current_index = schemes
            .iter()
            .position(|&s| s == self.active_scheme)
            .unwrap_or(0);
        let next_index = (current_index + 1) % schemes.len();
        self.active_scheme = schemes[next_index];
        self.rebuild_cache();
    }

    #[inline]
    pub fn format_sequence_bytes(&self, sequence: &[u8]) -> Vec<Span<'static>> {
        sequence
            .iter()
            .map(|&b| {
                let ch = Self::byte_to_nucleotide_str(b);
                Span::styled(ch, self.get_nucleotide_style(b))
            })
            .collect()
    }

    #[inline]
    fn byte_to_nucleotide_str(b: u8) -> &'static str {
        match b {
            // Standard nucleotides
            b'A' => "A",
            b'a' => "a",
            b'T' => "T",
            b't' => "t",
            b'C' => "C",
            b'c' => "c",
            b'G' => "G",
            b'g' => "g",
            b'N' => "N",
            b'n' => "n",
            // IUPAC ambiguity codes
            b'R' => "R",
            b'r' => "r",
            b'Y' => "Y",
            b'y' => "y",
            b'M' => "M",
            b'm' => "m",
            b'K' => "K",
            b'k' => "k",
            b'S' => "S",
            b's' => "s",
            b'W' => "W",
            b'w' => "w",
            b'H' => "H",
            b'h' => "h",
            b'B' => "B",
            b'b' => "b",
            b'V' => "V",
            b'v' => "v",
            b'D' => "D",
            b'd' => "d",
            // Gap
            b'-' => "-",
            _ => "?",
        }
    }
}

impl Default for ColorSchemeFormatter {
    fn default() -> Self {
        Self::new(ColorSchemeType::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::prelude::Color;

    #[test]
    fn test_format_with_background_scheme() {
        let sequence = b"ATCG-NX";
        let manager = ColorSchemeFormatter::new(ColorSchemeType::Background);
        let spans = manager.format_sequence_bytes(sequence);
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
    }

    #[test]
    fn test_format_with_standard_scheme() {
        let sequence = b"ATCG-NX";
        let manager = ColorSchemeFormatter::new(ColorSchemeType::Standard);
        let spans = manager.format_sequence_bytes(sequence);
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
    }
}
