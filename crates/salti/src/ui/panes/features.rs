use std::ops::Range;

use libmsa::AlignmentType;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    widgets::{Clear, Widget},
};

use crate::{
    config::theme::Theme,
    gff::{Feature, Gff, Strand},
    session::Session,
};

const MIN_LABEL_WIDTH: usize = 2;

#[derive(Debug, Clone)]
pub struct DisplayFeature<'gff> {
    pub feature: &'gff Feature,
    /// the visible columns it covers
    pub columns: Range<usize>,
    /// counts only shown features, so neighbours on screen get different colours
    pub colour_idx: usize,
}

#[derive(Debug, Clone)]
pub struct PlacedFeature<'gff> {
    pub feature: &'gff Feature,
    /// the screen cells it covers, from the left of the track
    pub span: Range<usize>,
    pub row: usize,
    pub colour_idx: usize,
}

/// the features that cover at least one visible column, coloured in order
pub fn display<'gff>(gff: &'gff Gff, session: &Session) -> Vec<DisplayFeature<'gff>> {
    let layout = session.layout();
    gff.features
        .iter()
        .filter_map(|feature| {
            let columns = session.column_range_at(feature.range.clone());
            let shown = layout.column_position(columns.start)..layout.column_position(columns.end);
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

pub fn render(placed_features: &[PlacedFeature<'_>], area: Rect, theme: &Theme, buf: &mut Buffer) {
    Clear.render(area, buf);
    buf.set_style(area, theme.base);
    let width = usize::from(area.width);

    for placed in placed_features {
        let span = placed.span.start.min(width)..placed.span.end.min(width);
        if usize::from(area.height) <= placed.row || span.is_empty() {
            continue;
        }
        // features take the four nucleotide colours in turn until proper palette support
        let style = theme
            .residues
            .style(AlignmentType::Dna, b"ATCG"[placed.colour_idx % 4]);
        let y = area.y + placed.row as u16;
        let x = |offset: usize| area.x + offset as u16;
        buf.set_style(Rect::new(x(span.start), y, span.len() as u16, 1), style);

        let (label, arrow) = match placed.feature.strand {
            Strand::Forward => (span.start..span.end - 1, Some((span.end - 1, "→"))),
            Strand::Reverse => (span.start + 1..span.end, Some((span.start, "←"))),
            Strand::Unknown => (span.clone(), None),
        };
        if let Some((at, arrow)) = arrow {
            buf.set_string(x(at), y, arrow, style);
        }
        if MIN_LABEL_WIDTH <= label.len() {
            let name: String = placed.feature.name.chars().take(label.len()).collect();
            let offset = (label.len() - name.chars().count()) / 2;
            buf.set_string(x(label.start + offset), y, name, style);
        }
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use rstest::rstest;

    use super::*;
    use crate::{
        session::{Position, ViewState},
        test_utils::{genes, session},
    };

    type Change = fn(&mut ViewState) -> Result<()>;

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
        let gff = genes(&[feature]);

        display(&gff, &session)
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
    fn display_works(
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
    fn display_hides(
        #[case] sequence: &[u8],
        #[case] change: Change,
        #[case] feature: Range<usize>,
    ) {
        let expected: [Range<usize>; 0] = [];

        assert_eq!(shown_columns(sequence, change, feature), expected);
    }

    #[test]
    fn display_colours_only_shown_features() {
        let mut session = session(&[b"-A-CC--G"]);
        session.update(Position::default(), gap_filter).unwrap();
        let gff = genes(&[5..7, 1..2, 3..5]);

        let colours: Vec<(usize, usize)> = display(&gff, &session)
            .into_iter()
            .map(|display| (display.feature.range.start, display.colour_idx))
            .collect();

        assert_eq!(colours, [(1, 0), (3, 1)]);
    }
}
