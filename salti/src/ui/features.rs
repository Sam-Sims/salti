use std::{num::NonZeroUsize, ops::Range};

use ratatui::style::Style;

use crate::{
    core::{
        gff::{Feature, Gff},
        model::AlignmentModel,
    },
    ui::ui_state::ThemeState,
};

const NUCLEOTIDE_POSITIONS_PER_COL: NonZeroUsize = NonZeroUsize::new(1).unwrap();
const PROTEIN_POSITIONS_PER_COL: NonZeroUsize = NonZeroUsize::new(3).unwrap();

#[derive(Debug, Clone)]
pub(crate) struct DisplayFeature<'a> {
    pub(crate) feature: &'a Feature,
    pub(crate) relative_col_range: Range<usize>,
    pub(crate) colour_idx: usize,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct FeatureStyle {
    pub(crate) background: Style,
    pub(crate) text: Style,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct FeatureMap {
    absolute_total_columns: usize,
    offset: usize,
    positions_to_col: NonZeroUsize,
}

impl FeatureMap {
    pub(crate) fn for_alignment(alignment: &AlignmentModel) -> Self {
        let absolute_total_columns = alignment.base().column_count();

        if alignment.is_reloaded_as_protein() {
            Self::protein(
                absolute_total_columns,
                alignment.translation_frame().offset(),
            )
        } else {
            Self::nucleotide(absolute_total_columns)
        }
    }

    pub(crate) fn protein(absolute_total_columns: usize, frame_offset: usize) -> Self {
        Self {
            absolute_total_columns,
            offset: frame_offset,
            positions_to_col: PROTEIN_POSITIONS_PER_COL,
        }
    }

    fn nucleotide(absolute_total_columns: usize) -> Self {
        Self {
            absolute_total_columns,
            offset: 0,
            positions_to_col: NUCLEOTIDE_POSITIONS_PER_COL,
        }
    }

    pub(crate) fn map_feature(
        &self,
        view: &libmsa::Alignment,
        feature: &Feature,
    ) -> Option<Range<usize>> {
        let absolute_range = self.map_feature_absolute_range(feature)?;
        view.relative_column_range_intersecting(absolute_range)
    }

    fn map_feature_absolute_range(&self, feature: &Feature) -> Option<Range<usize>> {
        let positions_to_col = self.positions_to_col.get();
        let start = feature.range.start.saturating_sub(self.offset) / positions_to_col;
        let end = feature
            .range
            .end
            .saturating_sub(self.offset)
            .div_ceil(positions_to_col);
        let clipped_range =
            start.min(self.absolute_total_columns)..end.min(self.absolute_total_columns);
        (!clipped_range.is_empty()).then_some(clipped_range)
    }
}

pub(crate) fn display_features<'a>(
    gff: &'a Gff,
    alignment: &AlignmentModel,
) -> Vec<DisplayFeature<'a>> {
    let mapping = FeatureMap::for_alignment(alignment);
    gff.features
        .iter()
        .filter_map(|feature| {
            mapping
                .map_feature(alignment.view(), feature)
                .map(|range| (feature, range))
        })
        .enumerate()
        .map(
            |(colour_idx, (feature, relative_col_range))| DisplayFeature {
                feature,
                relative_col_range,
                colour_idx,
            },
        )
        .collect()
}

pub(crate) fn feature_style(theme: &ThemeState, colour_idx: usize) -> FeatureStyle {
    let dna = theme.theme.sequence.dna;
    let colour = match colour_idx % 4 {
        0 => dna.a,
        1 => dna.t,
        2 => dna.c,
        _ => dna.g,
    };
    let background = theme.styles.base_block.bg(colour);
    let text = background.fg(theme.theme.sequence.foreground);
    FeatureStyle { background, text }
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

    fn raw(sequence: &[u8]) -> libmsa::RawSequence {
        libmsa::RawSequence {
            id: "seq".to_owned(),
            sequence: sequence.to_vec(),
        }
    }

    fn model_with_sequence(sequence: &[u8]) -> AlignmentModel {
        let alignment = libmsa::Alignment::new(vec![raw(sequence)]).unwrap();
        AlignmentModel::new(alignment).unwrap()
    }

    fn model_with_len(len: usize) -> AlignmentModel {
        let alignment = libmsa::Alignment::new(vec![libmsa::RawSequence {
            id: "seq".to_owned(),
            sequence: vec![b'A'; len],
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
