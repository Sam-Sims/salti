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
    use anyhow::Result;
    use rstest::rstest;

    use super::*;
    use crate::{
        core::{
            gff::FeatureType,
            session::{Position, ViewState},
        },
        test_utils::session,
    };

    type Change = fn(&mut ViewState) -> Result<()>;

    fn gff(ranges: &[Range<usize>]) -> Gff {
        Gff {
            features: ranges
                .iter()
                .map(|range| Feature {
                    name: format!("gene{}", range.start),
                    kind: FeatureType::Gene,
                    range: range.clone(),
                    strand: Strand::Forward,
                })
                .collect(),
        }
    }

    fn gap_filter(state: &mut ViewState) -> Result<()> {
        state.set_gap_filter(Some(0.0))
    }

    fn protein_view_frame3(state: &mut ViewState) -> Result<()> {
        state.frame = libmsa::ReadingFrame::Frame3;
        state.toggle_protein_view()
    }

    fn shown_columns(sequence: &[u8], change: Change, feature: Range<usize>) -> Vec<Range<usize>> {
        let mut session = session(&[sequence]);
        session.update(Position::default(), change).unwrap();
        let gff = gff(&[feature]);

        display_features(&gff, &session)
            .into_iter()
            .map(|display| display.columns)
            .collect()
    }

    #[rstest]
    #[case::plain(b"ACGTACGT", |_: &mut ViewState| Ok(()), 2..5, 2..5)]
    #[case::partly_hidden_shrinks(b"-A-CC--G", gap_filter, 2..7, 1..3)]
    #[case::clipped_past_end(b"ACGTACGT", |_: &mut ViewState| Ok(()), 6..11, 6..8)]
    #[case::protein_view(b"ATGAAATTTCC", ViewState::toggle_protein_view, 3..9, 1..3)]
    #[case::protein_view_after_offset(b"AAATGAAATTT", protein_view_frame3, 0..5, 0..1)]
    fn display_features_works(
        #[case] sequence: &[u8],
        #[case] change: Change,
        #[case] feature: Range<usize>,
        #[case] expected: Range<usize>,
    ) {
        assert_eq!(shown_columns(sequence, change, feature), [expected]);
    }

    #[rstest]
    #[case::hidden_by_filter(b"-A-CC--G", gap_filter, 5..7)]
    #[case::past_end(b"ACGTACGT", |_: &mut ViewState| Ok(()), 10..12)]
    #[case::before_frame_offset(b"AAATGAAATTT", protein_view_frame3, 0..2)]
    fn display_features_hides(
        #[case] sequence: &[u8],
        #[case] change: Change,
        #[case] feature: Range<usize>,
    ) {
        assert_eq!(
            shown_columns(sequence, change, feature),
            Vec::<Range<usize>>::new()
        );
    }

    #[test]
    fn display_features_colours_only_shown_features() {
        let mut session = session(&[b"-A-CC--G"]);
        session.update(Position::default(), gap_filter).unwrap();
        let gff = gff(&[5..7, 1..2, 3..5]);

        let colours: Vec<(usize, usize)> = display_features(&gff, &session)
            .into_iter()
            .map(|display| (display.feature.range.start, display.colour_idx))
            .collect();

        assert_eq!(colours, [(1, 0), (3, 1)]);
    }
}
