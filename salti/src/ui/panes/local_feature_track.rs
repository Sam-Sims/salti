use std::ops::Range;

use ratatui::{buffer::Buffer, layout::Rect, widgets::Widget};

use crate::{
    core::{gff::Gff, session::Session},
    ui::{
        features::{DisplayFeature, PlacedFeature, display_features, render_features},
        layout::Window,
        ui_state::ThemeState,
    },
};

const MAX_LOCAL_FEATURE_ROWS: usize = 5;

pub(crate) struct LocalFeatureTrack<'a> {
    pub(crate) gff: &'a Gff,
    pub(crate) session: &'a Session,
    pub(crate) window: &'a Window,
    pub(crate) theme: &'a ThemeState,
}

impl Widget for LocalFeatureTrack<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() {
            return;
        }

        let display_features = display_features(self.gff, self.session);
        let placed_features = place_visible_features(&display_features, &self.window.columns);
        render_features(&placed_features, area, self.theme, buf);
    }
}

pub(crate) fn local_feature_row_count(
    gff: &Gff,
    session: &Session,
    columns: &Range<usize>,
) -> usize {
    place_visible_features(&display_features(gff, session), columns)
        .iter()
        .map(|placed| placed.row + 1)
        .max()
        .unwrap_or(1)
}

fn place_visible_features<'a>(
    display_features: &[DisplayFeature<'a>],
    col_range: &Range<usize>,
) -> Vec<PlacedFeature<'a>> {
    let mut res: Vec<PlacedFeature<'a>> = Vec::new();
    let mut row_ends = [0; MAX_LOCAL_FEATURE_ROWS];

    for display_feature in display_features {
        let Some(visible_range) = intersect(&display_feature.columns, col_range) else {
            continue;
        };
        let span = visible_range.start - col_range.start..visible_range.end - col_range.start;
        let Some(row) = row_ends.iter().position(|&row_end| row_end <= span.start) else {
            continue;
        };
        row_ends[row] = span.end;
        res.push(PlacedFeature {
            feature: display_feature.feature,
            span,
            row,
            colour_idx: display_feature.colour_idx,
        });
    }

    res
}

fn intersect(left: &Range<usize>, right: &Range<usize>) -> Option<Range<usize>> {
    let start = left.start.max(right.start);
    let end = left.end.min(right.end);
    (start < end).then_some(start..end)
}

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use crate::core::gff::{Feature, FeatureType, Strand};

//     fn feature_with_name(name: &str, start: usize, end: usize) -> Feature {
//         Feature {
//             name: name.to_string(),
//             kind: FeatureType::Gene,
//             range: start..end,
//             strand: Strand::Forward,
//         }
//     }

//     fn raw(sequence: &[u8]) -> libmsa::Sequence {
//         libmsa::Sequence {
//             id: "seq".to_string(),
//             residues: sequence.to_vec(),
//         }
//     }

//     fn model_with_len(len: usize) -> AlignmentModel {
//         let alignment = libmsa::Alignment::new(vec![raw(&vec![b'A'; len])]).unwrap();
//         AlignmentModel::new(alignment).unwrap()
//     }

//     #[test]
//     fn local_feature_row_count_is_one_when_no_feature_is_visible() {
//         let gff = Gff {
//             features: vec![feature_with_name("left", 0, 5)],
//         };
//         let model = model_with_len(20);

//         assert_eq!(local_feature_row_count(&gff, &model, &(10..15)), 1);
//     }

//     #[test]
//     fn local_features_stack_overlapping_visible_spans() {
//         let gff = Gff {
//             features: vec![
//                 feature_with_name("a", 0, 10),
//                 feature_with_name("b", 2, 8),
//                 feature_with_name("c", 10, 12),
//             ],
//         };
//         let model = model_with_len(20);

//         assert_eq!(local_feature_row_count(&gff, &model, &(0..12)), 2);
//     }

//     #[test]
//     fn local_feature_rows_are_capped_at_five() {
//         let gff = Gff {
//             features: (0..8)
//                 .map(|idx| feature_with_name(&format!("f{idx}"), 0, 10))
//                 .collect(),
//         };
//         let model = model_with_len(20);

//         assert_eq!(local_feature_row_count(&gff, &model, &(0..12)), 5);
//     }
// }
