use std::ops::Range;

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    widgets::{Clear, Widget},
};

use crate::{
    core::{
        gff::{Feature, Gff, Strand},
        session::Session,
    },
    ui::ui_state::ThemeState,
};

const MIN_LABEL_WIDTH: usize = 2;

#[derive(Debug, Clone)]
pub(crate) struct DisplayFeature<'a> {
    pub(crate) feature: &'a Feature,
    pub(crate) columns: Range<usize>,
    pub(crate) colour_idx: usize,
}

#[derive(Debug, Clone)]
pub(crate) struct PlacedFeature<'a> {
    pub(crate) feature: &'a Feature,
    pub(crate) span: Range<usize>,
    pub(crate) row: usize,
    pub(crate) colour_idx: usize,
}

pub(crate) fn display_features<'a>(gff: &'a Gff, session: &Session) -> Vec<DisplayFeature<'a>> {
    let layout = session.layout();
    gff.features
        .iter()
        .filter_map(|feature| {
            let cols = session.column_range_at(feature.range.clone());
            let shown = layout.column_position(cols.start)..layout.column_position(cols.end);
            (!shown.is_empty()).then_some((feature, shown))
        })
        .enumerate()
        .map(|(colour_idx, (feature, columns))| DisplayFeature {
            feature,
            columns,
            colour_idx,
        })
        .collect()
}

pub(crate) fn render_features(
    placed_features: &[PlacedFeature<'_>],
    area: Rect,
    theme: &ThemeState,
    buf: &mut Buffer,
) {
    Clear.render(area, buf);
    buf.set_style(area, theme.styles.base_block);
    let width = usize::from(area.width);
    let dna = theme.theme.sequence.dna;

    for placed in placed_features {
        let span = placed.span.start.min(width)..placed.span.end.min(width);
        if usize::from(area.height) <= placed.row || span.is_empty() {
            continue;
        }
        let colour = match placed.colour_idx % 4 {
            0 => dna.a,
            1 => dna.t,
            2 => dna.c,
            _ => dna.g,
        };
        let background = theme.styles.base_block.bg(colour);
        let text = background.fg(theme.theme.sequence.foreground);
        let y = area.y + placed.row as u16;
        let x = |offset: usize| area.x + offset as u16;
        buf.set_style(
            Rect::new(x(span.start), y, span.len() as u16, 1),
            background,
        );

        let (label, arrow) = match placed.feature.strand {
            Strand::Forward => (span.start..span.end - 1, Some((span.end - 1, "→"))),
            Strand::Reverse => (span.start + 1..span.end, Some((span.start, "←"))),
            Strand::Unknown => (span.clone(), None),
        };
        if let Some((at, arrow)) = arrow {
            buf.set_string(x(at), y, arrow, text);
        }
        if MIN_LABEL_WIDTH <= label.len() {
            let name: String = placed.feature.name.chars().take(label.len()).collect();
            let offset = (label.len() - name.chars().count()) / 2;
            buf.set_string(x(label.start + offset), y, name, text);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::gff::Strand;

    fn feature(start: usize, end: usize) -> Feature {
        Feature {
            name: "gene".to_owned(),
            kind: crate::core::gff::FeatureType::Gene,
            range: start..end + 1,
            strand: Strand::Forward,
        }
    }

    fn raw(sequence: &[u8]) -> libmsa::Sequence {
        libmsa::Sequence {
            id: "seq".to_owned(),
            residues: sequence.to_vec(),
        }
    }

    fn model_with_sequence(sequence: &[u8]) -> AlignmentModel {
        let alignment = libmsa::Alignment::new(vec![raw(sequence)]).unwrap();
        AlignmentModel::new(alignment).unwrap()
    }

    fn model_with_len(len: usize) -> AlignmentModel {
        let alignment = libmsa::Alignment::new(vec![libmsa::Sequence {
            id: "seq".to_owned(),
            residues: vec![b'A'; len],
        }])
        .unwrap();
        AlignmentModel::new(alignment).unwrap()
    }

    #[test]
    fn filtered_nt_collapses_hidden_cols() {
        let mut model = model_with_sequence(b"-A-CC--G");
        model.set_gap_filter(Some(0.0)).unwrap();
        let mapping = FeatureMap::for_alignment(&model);

        assert!(model.view().absolute_column_ids().eq([1, 3, 4, 7]));
        assert_eq!(
            mapping.map_feature(model.view(), &feature(2, 6)),
            Some(1..3)
        );
    }

    #[test]
    fn filtered_nt_hides_feature() {
        let mut model = model_with_sequence(b"-A-CC--G");
        model.set_gap_filter(Some(0.0)).unwrap();
        let mapping = FeatureMap::for_alignment(&model);

        assert_eq!(mapping.map_feature(model.view(), &feature(5, 6)), None);
    }

    #[test]
    fn filtered_protein_projects_cols() {
        let mut model = model_with_sequence(b"M-M-M");
        model.set_gap_filter(Some(0.0)).unwrap();
        let mapping = FeatureMap::protein(5, 0);

        assert!(model.view().absolute_column_ids().eq([0, 2, 4]));
        assert_eq!(
            mapping.map_feature(model.view(), &feature(3, 8)),
            Some(1..2)
        );
    }

    #[test]
    fn protein_clips_before_frame() {
        let model = model_with_len(5);
        let mapping = FeatureMap::protein(5, 2);

        assert_eq!(
            mapping.map_feature(model.view(), &feature(0, 4)),
            Some(0..1)
        );
    }

    #[test]
    fn mapping_clips_past_alignment_end() {
        let model = model_with_len(8);
        let mapping = FeatureMap::for_alignment(&model);

        assert_eq!(
            mapping.map_feature(model.view(), &feature(6, 10)),
            Some(6..8)
        );
    }

    #[test]
    fn mapping_hides_after_alignment_end() {
        let model = model_with_len(8);
        let mapping = FeatureMap::for_alignment(&model);

        assert_eq!(mapping.map_feature(model.view(), &feature(10, 12)), None);
    }
}
