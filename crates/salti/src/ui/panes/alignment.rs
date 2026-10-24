use ratatui::{
    buffer::Buffer,
    layout::{Margin, Rect},
    style::Styled,
    text::Line,
    widgets::{Paragraph, Widget},
};

use crate::{
    config::theme::Theme,
    gff::Gff,
    session::{DiffMode, Session, WindowColumns},
    ui::{
        self,
        layout::{self, Window},
        panes::{self, local_feature_track::LocalFeatureTrack, ruler::Ruler},
        selection::Selection,
    },
};

const SELECTION_ROW_ALPHA: f32 = 0.22;
const SELECTION_COLUMN_ALPHA: f32 = 0.28;

pub struct AlignmentPane<'frame> {
    pub session: &'frame Session,
    pub window: &'frame Window,
    pub columns: &'frame WindowColumns<'frame>,
    pub gff: Option<&'frame Gff>,
    pub local_feature_rows: u16,
    pub selection: Option<Selection>,
    pub theme: &'frame Theme,
}

impl Widget for AlignmentPane<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let (session, window, columns, theme) =
            (self.session, self.window, self.columns, self.theme);
        let inner_area = panes::render_pane(theme, None, area, buf);

        let [local_feature_area, ruler_area, sequence_rows_area] =
            layout::split_alignment_header(inner_area, self.local_feature_rows);

        if let Some(gff) = self.gff {
            LocalFeatureTrack {
                gff,
                session,
                window,
                theme,
            }
            .render(local_feature_area, buf);
        }
        Ruler {
            session,
            window,
            theme,
        }
        .render(ruler_area, buf);

        let layout = session.layout();
        panes::render_column_scrollbar(
            theme,
            layout.columns().len(),
            &window.columns,
            area.inner(Margin::new(1, 0)),
            buf,
        );

        let empty_message = if layout.columns().is_empty() {
            Some("No columns to display")
        } else if layout.rows().is_empty() {
            Some("No sequences to display")
        } else {
            None
        };
        if let Some(message) = empty_message {
            ui::render_centred_lines(
                vec![Line::from(message.set_style(theme.error))],
                theme.base,
                sequence_rows_area,
                buf,
            );
            return;
        }

        let bytes = |row| -> Vec<u8> { columns.grid.cells(row, &columns.columns).collect() };
        let diff = match session.diff_mode() {
            DiffMode::Off => None,
            DiffMode::Reference => session.state().reference().map(bytes),
            DiffMode::Consensus => Some(columns.consensus.clone()),
        };
        let lines: Vec<Line> = layout::screen_rows(layout, window)
            .map(|row| match row {
                Some(row) => panes::sequence_line(
                    columns,
                    &bytes(layout.rows()[row]),
                    diff.as_deref(),
                    theme,
                ),
                None => panes::separator_line(sequence_rows_area.width, theme),
            })
            .collect();
        Paragraph::new(lines)
            .style(theme.base)
            .render(sequence_rows_area, buf);

        if let Some(selection) = self.selection {
            selection.shade_rows(
                buf,
                sequence_rows_area,
                layout,
                window,
                theme.selection_rows,
                SELECTION_ROW_ALPHA,
            );
            selection.shade_columns(
                buf,
                sequence_rows_area,
                window,
                theme.selection_columns,
                SELECTION_COLUMN_ALPHA,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use std::ops::Range;

    use anyhow::Result;
    use regex::Regex;

    use super::*;
    use crate::{
        gff::{Feature, FeatureType, Strand},
        session::{Position, ViewState},
        test_utils::{buffer_text, full_window, session},
        ui::{layout::AppLayout, panes::local_feature_track},
    };

    const CAT: &[u8] = b"CATCATCATCATCATCAT";

    fn changed(sequences: &[&[u8]], change: impl FnOnce(&mut ViewState) -> Result<()>) -> Session {
        let mut session = session(sequences);
        session.update(Position::default(), change).unwrap();
        session
    }

    fn render_text(session: &Session, gff: Option<&Gff>, window: &Window, area: Rect) -> String {
        let local_feature_rows = gff.map_or(0, |gff| {
            u16::try_from(local_feature_track::row_count(
                gff,
                session,
                &window.columns,
            ))
            .unwrap()
        });
        let layout = AppLayout::new(area, 0, local_feature_rows);
        let columns = session.window_columns(window.columns.clone());
        let theme = Theme::default();
        let mut buf = Buffer::empty(area);

        AlignmentPane {
            session,
            window,
            columns: &columns,
            gff,
            local_feature_rows,
            selection: None,
            theme: &theme,
        }
        .render(layout.alignment_pane, &mut buf);

        buffer_text(&buf, layout.alignment_pane)
    }

    fn render_full(session: &Session) -> String {
        render_text(
            session,
            None,
            &full_window(session),
            Rect::new(0, 0, 100, 12),
        )
    }

    fn gene(name: &str, range: Range<usize>) -> Gff {
        Gff {
            features: vec![Feature {
                name: name.to_string(),
                kind: FeatureType::Gene,
                range,
                strand: Strand::Forward,
            }],
        }
    }

    #[test]
    fn basic() {
        let session = session(&[CAT, CAT, CAT]);

        insta::assert_snapshot!(render_full(&session));
    }

    #[test]
    fn feature_track_offscreen() {
        let session = session(&[CAT, CAT, CAT]);
        let gff = gene("Offscreen", 30..40);

        insta::assert_snapshot!(render_text(
            &session,
            Some(&gff),
            &full_window(&session),
            Rect::new(0, 0, 100, 12)
        ));
    }

    #[test]
    fn feature_track() {
        let session = session(&[CAT, CAT, CAT]);
        let gff = gene("Spike", 2..14);

        insta::assert_snapshot!(render_text(
            &session,
            Some(&gff),
            &full_window(&session),
            Rect::new(0, 0, 100, 12)
        ));
    }

    #[test]
    fn pinned_fragmented() {
        let session = changed(
            &[
                b"AAA---CATCATCATCAT",
                b"CCC---CATCATCATCAT",
                b"GGG---CATCATCATCAT",
                b"TTT---CATCATCATCAT",
            ],
            |state| {
                state.pin(1);
                state.pin(3);
                state.set_gap_filter(Some(0.5))
            },
        );

        insta::assert_snapshot!(render_full(&session));
    }

    #[test]
    fn translated() {
        let session = changed(&[CAT, CAT, CAT], ViewState::toggle_translation_overlay);

        insta::assert_snapshot!(render_full(&session));
    }

    #[test]
    fn protein_partial_codon() {
        let session = changed(
            &[b"CATCATCATCATCATCA", b"CATCATCATCATCATCA"],
            ViewState::toggle_protein_view,
        );

        insta::assert_snapshot!(render_full(&session));
    }

    #[test]
    fn diff_reference() {
        let mut session = changed(
            &[CAT, b"CATCATGATCATCATCAT", b"CATCATCATCATGATCAT"],
            |state| {
                state.set_reference(Some(0));
                Ok(())
            },
        );
        session.set_diff_mode(DiffMode::Reference).unwrap();

        insta::assert_snapshot!(render_full(&session));
    }

    #[test]
    fn translated_diff_reference() {
        let mut session = changed(
            &[CAT, b"CATCATGATCATCATCAT", b"CATCATCATCATGATCAT"],
            |state| {
                state.set_reference(Some(0));
                state.toggle_translation_overlay()
            },
        );
        session.set_diff_mode(DiffMode::Reference).unwrap();

        insta::assert_snapshot!(render_full(&session));
    }

    #[test]
    fn diff_consensus() {
        let mut session = session(&[CAT, b"CATCATGATCATCATCAT", b"CATCATCATCATGATCAT"]);
        session.set_diff_mode(DiffMode::Consensus).unwrap();

        insta::assert_snapshot!(render_full(&session));
    }

    #[test]
    fn scrolled() {
        let long: &[u8] = b"CATCATCATCATCATCATCATCATCATCATCATCAT";
        let session = session(&[long, long, long]);
        let window = Window {
            columns: 10..36,
            ..full_window(&session)
        };

        insta::assert_snapshot!(render_text(
            &session,
            None,
            &window,
            Rect::new(0, 0, 60, 12)
        ));
    }

    #[test]
    fn pinned_scrolled() {
        let session = changed(
            &[
                b"AAACATCATCATCATCAT",
                b"CCCCATCATCATCATCAT",
                b"GGGCATCATCATCATCAT",
                b"TTTCATCATCATCATCAT",
                b"ACGCATCATCATCATCAT",
                b"TGACATCATCATCATCAT",
            ],
            |state| {
                state.pin(1);
                state.pin(4);
                Ok(())
            },
        );
        let window = Window {
            rows: 2..3,
            ..full_window(&session)
        };

        insta::assert_snapshot!(render_text(
            &session,
            None,
            &window,
            Rect::new(0, 0, 100, 12)
        ));
    }

    #[test]
    fn no_rows() {
        let session = changed(&[CAT, CAT], |state| {
            state.row_regex_filter = Some(Regex::new("nothing").unwrap());
            Ok(())
        });

        insta::assert_snapshot!(render_full(&session));
    }

    #[test]
    fn no_columns() {
        let session = changed(&[CAT, CAT], |state| state.set_constant_filter(Some(0.0)));

        insta::assert_snapshot!(render_full(&session));
    }
}
