use libmsa::{
    AlignmentType,
    residue::{self, Class},
};
use ratatui::style::{Color, Style};

// Generated from iwanthue
// TODO: explore palette crate and maybe build.rs for compile time nicer generated palettes?
#[allow(clippy::unreadable_literal)]
const FULL_ASCII_COLOURS: [Color; 94] = [
    Color::from_u32(0x914d5a),
    Color::from_u32(0x52c95a),
    Color::from_u32(0x924ac9),
    Color::from_u32(0x8ebe39),
    Color::from_u32(0x7475f2),
    Color::from_u32(0x469e2d),
    Color::from_u32(0xbf209e),
    Color::from_u32(0x3bce80),
    Color::from_u32(0xeb50bd),
    Color::from_u32(0x3fcc97),
    Color::from_u32(0xee3f91),
    Color::from_u32(0x3ea866),
    Color::from_u32(0xb85bd2),
    Color::from_u32(0xb2b831),
    Color::from_u32(0x3659cb),
    Color::from_u32(0xcfb746),
    Color::from_u32(0x7452c2),
    Color::from_u32(0x85c570),
    Color::from_u32(0xd978eb),
    Color::from_u32(0x3d8136),
    Color::from_u32(0xa13c9f),
    Color::from_u32(0x829c3a),
    Color::from_u32(0x5886f1),
    Color::from_u32(0xe5a43e),
    Color::from_u32(0x554baa),
    Color::from_u32(0xa8952a),
    Color::from_u32(0x5762be),
    Color::from_u32(0xe47f2e),
    Color::from_u32(0x356db5),
    Color::from_u32(0xdf5628),
    Color::from_u32(0x44d2dd),
    Color::from_u32(0xd12a47),
    Color::from_u32(0x5bd5bc),
    Color::from_u32(0xd42273),
    Color::from_u32(0x70cb92),
    Color::from_u32(0xc93892),
    Color::from_u32(0x577e26),
    Color::from_u32(0xb083e6),
    Color::from_u32(0x757913),
    Color::from_u32(0x8474d3),
    Color::from_u32(0xb97e27),
    Color::from_u32(0x5d94e1),
    Color::from_u32(0xb23821),
    Color::from_u32(0x26b09d),
    Color::from_u32(0xec476f),
    Color::from_u32(0x33a27e),
    Color::from_u32(0xc42e64),
    Color::from_u32(0x3b8556),
    Color::from_u32(0xe273cb),
    Color::from_u32(0x516615),
    Color::from_u32(0x834ca7),
    Color::from_u32(0x92b167),
    Color::from_u32(0xa62771),
    Color::from_u32(0x74b68d),
    Color::from_u32(0xe7699f),
    Color::from_u32(0x2f6633),
    Color::from_u32(0xab4c8d),
    Color::from_u32(0x476025),
    Color::from_u32(0x6b4fa0),
    Color::from_u32(0x937b2c),
    Color::from_u32(0x505099),
    Color::from_u32(0xbdb26f),
    Color::from_u32(0x7e4990),
    Color::from_u32(0x828136),
    Color::from_u32(0xae75bf),
    Color::from_u32(0x5b5b10),
    Color::from_u32(0xb0a5e8),
    Color::from_u32(0x815a16),
    Color::from_u32(0x55aede),
    Color::from_u32(0xe95e57),
    Color::from_u32(0x36b0b8),
    Color::from_u32(0xaf3f58),
    Color::from_u32(0x328b7c),
    Color::from_u32(0xe398d4),
    Color::from_u32(0x226a4d),
    Color::from_u32(0xee8f6f),
    Color::from_u32(0x475993),
    Color::from_u32(0xa55b23),
    Color::from_u32(0x4382af),
    Color::from_u32(0xca6a58),
    Color::from_u32(0x6f64a6),
    Color::from_u32(0x705d18),
    Color::from_u32(0x8381c0),
    Color::from_u32(0x667f42),
    Color::from_u32(0xc17bab),
    Color::from_u32(0x62612c),
    Color::from_u32(0x925a88),
    Color::from_u32(0x89834d),
    Color::from_u32(0x9d476e),
    Color::from_u32(0xd59d6a),
    Color::from_u32(0x7f4569),
    Color::from_u32(0x956d3c),
    Color::from_u32(0xe18291),
    Color::from_u32(0x965d39),
];

#[derive(Debug)]
pub struct ResidueStyles {
    pub(super) a: Style,
    pub(super) t: Style,
    pub(super) c: Style,
    pub(super) g: Style,
    pub(super) ambiguity: Style,
    pub(super) hydrophobic: Style,
    pub(super) cysteine: Style,
    pub(super) positive: Style,
    pub(super) negative: Style,
    pub(super) polar: Style,
    pub(super) glycine: Style,
    pub(super) proline: Style,
    pub(super) aromatic: Style,
    pub(super) gap: Style,
    pub(super) unknown: Style,
}

impl ResidueStyles {
    pub fn style(&self, alignment_type: AlignmentType, byte: u8) -> Style {
        match (alignment_type, residue::classify(byte, alignment_type)) {
            (AlignmentType::Generic, _) => generic(byte),
            (_, Class::Gap) => self.gap,
            (_, Class::Unknown) => self.unknown,
            (_, Class::Ambiguous) => self.ambiguity,
            (AlignmentType::Dna, Class::Residue) => self.base(byte),
            (AlignmentType::Protein, Class::Residue) => self.amino_acid(byte),
        }
    }

    fn base(&self, byte: u8) -> Style {
        residue::nucleotide_index(byte)
            .map_or_else(Style::new, |index| [self.a, self.t, self.c, self.g][index])
    }

    // groups from clustal
    // http://www.jalview.org/help/html/colourSchemes/clustal.html
    fn amino_acid(&self, byte: u8) -> Style {
        match byte.to_ascii_uppercase() {
            b'A' | b'V' | b'L' | b'I' | b'M' | b'F' | b'W' => self.hydrophobic,
            b'C' => self.cysteine,
            b'K' | b'R' => self.positive,
            b'D' | b'E' => self.negative,
            b'S' | b'T' | b'N' | b'Q' => self.polar,
            b'G' => self.glycine,
            b'P' => self.proline,
            b'H' | b'Y' => self.aromatic,
            _ => Style::new(),
        }
    }
}

fn generic(byte: u8) -> Style {
    byte.checked_sub(b'!')
        .and_then(|index| FULL_ASCII_COLOURS.get(usize::from(index)))
        .map_or_else(Style::new, |&colour| {
            Style::new().bg(colour).fg(Color::Black)
        })
}
