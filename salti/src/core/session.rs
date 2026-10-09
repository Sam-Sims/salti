use std::{fmt, ops::Range, range::RangeInclusive, str::FromStr};

use anyhow::{Result, bail, ensure};

use crate::core::{
    columns::{Cell, WindowColumns},
    layout::Layout,
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum DiffMode {
    #[default]
    Off,
    Reference,
    Consensus,
}

impl DiffMode {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Reference => "reference",
            Self::Consensus => "consensus",
        }
    }

    pub const fn all() -> [Self; 3] {
        [Self::Off, Self::Reference, Self::Consensus]
    }
}

impl fmt::Display for DiffMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl FromStr for DiffMode {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::all()
            .into_iter()
            .find(|mode| mode.name() == value)
            .ok_or_else(|| anyhow::format_err!("invalid diff mode: {value}"))
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ViewMode {
    Plain {
        alignment_type: libmsa::AlignmentType,
        filter: libmsa::ColumnFilter,
    },
    TranslationOverlay,
    ProteinView {
        filter: libmsa::ColumnFilter,
    },
}

impl ViewMode {
    pub fn filter(self) -> libmsa::ColumnFilter {
        match self {
            Self::Plain { filter, .. } | Self::ProteinView { filter } => filter,
            Self::TranslationOverlay => libmsa::ColumnFilter::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ViewState {
    pub frame: libmsa::ReadingFrame,
    pub row_regex_filter: Option<regex::Regex>,
    mode: ViewMode,
    pinned: Vec<usize>,
    reference: Option<usize>,
}

impl ViewState {
    pub fn mode(&self) -> ViewMode {
        self.mode
    }

    pub fn pinned(&self) -> &[usize] {
        &self.pinned
    }

    pub fn reference(&self) -> Option<usize> {
        self.reference
    }

    pub fn toggle_translation_overlay(&mut self) -> Result<()> {
        self.mode = match self.mode {
            ViewMode::Plain {
                alignment_type,
                filter,
            } => {
                ensure_translatable(alignment_type)?;
                ensure!(
                    !filter.is_active(),
                    "The translation overlay can't be used with column filters. Clear them first"
                );
                ViewMode::TranslationOverlay
            }
            ViewMode::TranslationOverlay => ViewMode::Plain {
                alignment_type: libmsa::AlignmentType::Dna,
                filter: libmsa::ColumnFilter::default(),
            },
            ViewMode::ProteinView { .. } => bail!(
                "The translation overlay isn't available in the protein view. Press T to leave it first"
            ),
        };
        Ok(())
    }

    pub fn toggle_protein_view(&mut self) -> Result<()> {
        self.mode = match self.mode {
            ViewMode::Plain {
                alignment_type,
                filter,
            } => {
                ensure_translatable(alignment_type)?;
                ViewMode::ProteinView { filter }
            }
            ViewMode::TranslationOverlay => ViewMode::ProteinView {
                filter: libmsa::ColumnFilter::default(),
            },
            ViewMode::ProteinView { filter } => ViewMode::Plain {
                alignment_type: libmsa::AlignmentType::Dna,
                filter,
            },
        };
        Ok(())
    }

    pub fn set_alignment_type(&mut self, alignment_type: libmsa::AlignmentType) -> Result<()> {
        match &mut self.mode {
            ViewMode::Plain {
                alignment_type: current,
                ..
            } => *current = alignment_type,
            ViewMode::TranslationOverlay => ensure!(
                alignment_type.supports_translation(),
                "The translation overlay needs a DNA alignment. Turn it off with t first"
            ),
            ViewMode::ProteinView { .. } => bail!(
                "The sequence type can't be changed in the protein view. Press T to leave it first"
            ),
        }
        Ok(())
    }

    pub fn set_gap_filter(&mut self, fraction: Option<f64>) -> Result<()> {
        match self.filter_mut() {
            Some(filter) => filter.max_gap_fraction = fraction,
            None => ensure_no_filter_in_overlay(fraction)?,
        }
        Ok(())
    }

    pub fn set_constant_filter(&mut self, fraction: Option<f64>) -> Result<()> {
        match self.filter_mut() {
            Some(filter) => filter.min_const_fraction = fraction,
            None => ensure_no_filter_in_overlay(fraction)?,
        }
        Ok(())
    }

    pub fn clear_column_filter(&mut self) {
        if let Some(filter) = self.filter_mut() {
            *filter = libmsa::ColumnFilter::default();
        }
    }

    pub fn pin(&mut self, row: usize) {
        self.pinned.push(row);
    }

    pub fn unpin(&mut self, row: usize) {
        self.pinned.retain(|&pinned| pinned != row);
    }

    pub fn set_reference(&mut self, row: Option<usize>) {
        self.pinned.retain(|&pinned| Some(pinned) != row);
        self.reference = row;
    }

    pub(super) fn grid<'a>(&self, base_alignment: &'a libmsa::Alignment) -> libmsa::Grid<'a> {
        match self.mode {
            ViewMode::Plain { alignment_type, .. } => base_alignment.grid(alignment_type),
            ViewMode::TranslationOverlay => base_alignment.grid(libmsa::AlignmentType::Dna),
            ViewMode::ProteinView { .. } => base_alignment.translated_grid(self.frame),
        }
    }

    fn filter_mut(&mut self) -> Option<&mut libmsa::ColumnFilter> {
        match &mut self.mode {
            ViewMode::Plain { filter, .. } | ViewMode::ProteinView { filter } => Some(filter),
            ViewMode::TranslationOverlay => None,
        }
    }
}

fn ensure_no_filter_in_overlay(fraction: Option<f64>) -> Result<()> {
    ensure!(
        fraction.is_none(),
        "Column filters can't be used with the translation overlay. Turn it off with t first"
    );
    Ok(())
}

fn ensure_translatable(alignment_type: libmsa::AlignmentType) -> Result<()> {
    ensure!(
        alignment_type.supports_translation(),
        "Translation needs a DNA alignment. Use set-sequence-type if the type was detected incorrectly"
    );
    Ok(())
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Position {
    pub row: usize,
    pub column: usize,
    pub name: usize,
}

#[derive(Debug)]
pub struct Session {
    base_alignment: libmsa::Alignment,
    source: String,
    state: ViewState,
    layout: Layout,
    pub diff_mode: DiffMode,
    pub consensus_method: libmsa::ConsensusMethod,
}

impl Session {
    pub fn new(base_alignment: libmsa::Alignment, source: String) -> Self {
        let state = ViewState {
            frame: libmsa::ReadingFrame::Frame1,
            row_regex_filter: None,
            mode: ViewMode::Plain {
                alignment_type: base_alignment.detected_type(),
                filter: libmsa::ColumnFilter::default(),
            },
            pinned: Vec::new(),
            reference: None,
        };
        let layout = Layout::build(&base_alignment, &state);

        Self {
            base_alignment,
            source,
            state,
            layout,
            diff_mode: DiffMode::default(),
            consensus_method: libmsa::ConsensusMethod::default(),
        }
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn update(
        &mut self,
        position: Position,
        change: impl FnOnce(&mut ViewState) -> Result<()>,
    ) -> Result<Position> {
        let columns = self.layout.columns();
        let nt = columns
            .get(position.column)
            .or(columns.last())
            .map(|&column| self.nt_start(column));
        let top = self.layout.main().get(position.row).copied();

        let mut next = self.state.clone();
        change(&mut next)?;
        self.layout = Layout::build(&self.base_alignment, &next);
        self.state = next;

        Ok(Position {
            column: nt.map_or(position.column, |nt| {
                self.layout
                    .column_position(self.column_range_at(nt..nt + 1).start)
            }),
            row: top.map_or(0, |row| self.layout.row_position(row)),
            ..position
        })
    }

    pub fn state(&self) -> &ViewState {
        &self.state
    }

    pub fn layout(&self) -> &Layout {
        &self.layout
    }

    pub fn base_alignment(&self) -> &libmsa::Alignment {
        &self.base_alignment
    }

    pub fn grid(&self) -> libmsa::Grid<'_> {
        self.state.grid(&self.base_alignment)
    }

    pub fn window_columns(&self, window: Range<usize>) -> WindowColumns<'_> {
        let (grid, columns, cells) = match self.state.mode {
            ViewMode::TranslationOverlay if !window.is_empty() => {
                let frame = self.state.frame;
                let last_column = self.base_alignment.width() - 1;
                let protein_window = self.protein_columns(window.clone());
                let cells = window
                    .map(|nt| {
                        let protein_column = frame.protein_col(nt)?;
                        let codon_start = frame.nt_range(protein_column).start;
                        Some(Cell {
                            index: protein_column - protein_window.start,
                            centre: nt == (codon_start + 1).min(last_column),
                        })
                    })
                    .collect();
                (
                    self.base_alignment.translated_grid(frame),
                    protein_window.collect(),
                    cells,
                )
            }
            _ => {
                let columns = self.layout.columns()[window].to_vec();
                let cells = (0..columns.len())
                    .map(|index| {
                        Some(Cell {
                            index,
                            centre: true,
                        })
                    })
                    .collect();
                (self.grid(), columns, cells)
            }
        };
        let summaries = grid.summaries(self.layout.rows(), &columns, self.consensus_method);
        WindowColumns {
            grid,
            columns,
            cells,
            summaries,
        }
    }

    fn nt_start(&self, column: usize) -> usize {
        match self.state.mode {
            ViewMode::ProteinView { .. } => self.state.frame.nt_range(column).start,
            ViewMode::Plain { .. } | ViewMode::TranslationOverlay => column,
        }
    }

    pub fn protein_columns(&self, nt: Range<usize>) -> Range<usize> {
        self.state
            .frame
            .protein_range(nt, self.base_alignment.width())
    }

    pub fn column_range_at(&self, nt: Range<usize>) -> Range<usize> {
        match self.state.mode {
            ViewMode::ProteinView { .. } => self.protein_columns(nt),
            ViewMode::Plain { .. } | ViewMode::TranslationOverlay => nt,
        }
    }

    pub fn selection_columns(&self, columns: RangeInclusive<usize>) -> RangeInclusive<usize> {
        if !matches!(self.state.mode, ViewMode::TranslationOverlay) {
            return columns;
        }
        let frame = self.state.frame;
        let last_column = self.base_alignment.width() - 1;
        let start = frame
            .protein_col(columns.start)
            .map_or(columns.start, |protein_column| {
                frame.nt_range(protein_column).start
            });
        let end = frame
            .protein_col(columns.last)
            .map_or(columns.last, |protein_column| {
                frame.nt_range(protein_column).end - 1
            });
        (start..=end.min(last_column)).into()
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;
    use crate::test_utils::session;

    type Change = fn(&mut ViewState) -> Result<()>;

    fn set_protein(state: &mut ViewState) -> Result<()> {
        state.set_alignment_type(libmsa::AlignmentType::Protein)
    }

    fn at_column(column: usize) -> Position {
        Position {
            column,
            ..Position::default()
        }
    }

    fn at_row(row: usize) -> Position {
        Position {
            row,
            ..Position::default()
        }
    }

    #[test]
    fn set_reference_unpins_row() {
        let mut state = session(&[b"A", b"A", b"A"]).state().clone();
        state.pin(2);
        state.pin(1);

        state.set_reference(Some(2));

        assert_eq!(state.pinned(), [1]);
        assert_eq!(state.reference(), Some(2));
    }

    #[test]
    fn unpin_keeps_other_pins() {
        let mut state = session(&[b"A", b"A", b"A"]).state().clone();
        state.pin(2);
        state.pin(0);
        state.pin(1);

        state.unpin(0);

        assert_eq!(state.pinned(), [2, 1]);
    }

    #[rstest]
    #[case::overlay_on_protein(set_protein, ViewState::toggle_translation_overlay)]
    #[case::overlay_with_column_filter(
        |state: &mut ViewState| state.set_gap_filter(Some(0.5)),
        ViewState::toggle_translation_overlay
    )]
    #[case::overlay_in_protein_view(
        ViewState::toggle_protein_view,
        ViewState::toggle_translation_overlay
    )]
    #[case::protein_view_on_protein(set_protein, ViewState::toggle_protein_view)]
    #[case::protein_type_in_overlay(ViewState::toggle_translation_overlay, set_protein)]
    #[case::type_in_protein_view(ViewState::toggle_protein_view, set_protein)]
    #[case::gap_filter_in_overlay(
        ViewState::toggle_translation_overlay,
        |state: &mut ViewState| state.set_gap_filter(Some(0.5))
    )]
    #[case::constant_filter_in_overlay(
        ViewState::toggle_translation_overlay,
        |state: &mut ViewState| state.set_constant_filter(Some(0.5))
    )]
    fn view_state_rejects(#[case] setup: Change, #[case] change: Change) {
        let mut state = session(&[b"ATG"]).state().clone();
        setup(&mut state).unwrap();

        assert!(change(&mut state).is_err());
    }

    #[rstest]
    #[case::shown_column(|state: &mut ViewState| state.set_gap_filter(Some(0.0)), 2, 1)]
    #[case::hidden_column_moves_to_next(|state: &mut ViewState| state.set_gap_filter(Some(0.0)), 1, 1)]
    #[case::past_end_keeps_last_column(|state: &mut ViewState| state.set_gap_filter(Some(0.0)), 9, 3)]
    #[case::protein_view(ViewState::toggle_protein_view, 4, 1)]
    fn update_keeps_left_column(
        #[case] change: Change,
        #[case] column: usize,
        #[case] expected: usize,
    ) {
        let mut session = session(&[b"A-A-AA", b"AAAAAA"]);

        let position = session.update(at_column(column), change).unwrap();

        assert_eq!(position.column, expected);
    }

    #[test]
    fn update_clearing_filter_keeps_left_column() {
        let mut session = session(&[b"A-A-A", b"AAAAA"]);
        session
            .update(Position::default(), |state| state.set_gap_filter(Some(0.0)))
            .unwrap();

        let position = session
            .update(at_column(1), |state| {
                state.clear_column_filter();
                Ok(())
            })
            .unwrap();

        assert_eq!(position.column, 2);
    }

    #[rstest]
    fn update_protein_view_round_trip_lands_on_codon_start(#[values(3, 4, 5)] column: usize) {
        let mut session = session(&[b"ATGATGATG"]);

        let protein = session
            .update(at_column(column), ViewState::toggle_protein_view)
            .unwrap();
        let nt = session
            .update(protein, ViewState::toggle_protein_view)
            .unwrap();

        assert_eq!(protein.column, 1);
        assert_eq!(nt.column, 3);
    }

    #[rstest]
    #[case::top_row_still_shown("s[023]", 2, 1)]
    #[case::top_row_hidden_moves_to_next("s[03]", 2, 1)]
    #[case::no_rows_left("nothing", 2, 0)]
    fn update_keeps_top_row(#[case] regex: &str, #[case] row: usize, #[case] expected: usize) {
        let mut session = session(&[b"A", b"A", b"A", b"A"]);

        let position = session
            .update(at_row(row), |state| {
                state.row_regex_filter = Some(regex::Regex::new(regex).unwrap());
                Ok(())
            })
            .unwrap();

        assert_eq!(position.row, expected);
    }

    #[test]
    fn update_keeps_name() {
        let mut session = session(&[b"A"]);
        let position = Position {
            name: 3,
            ..Position::default()
        };

        let position = session.update(position, |_| Ok(())).unwrap();

        assert_eq!(position.name, 3);
    }

    #[test]
    fn update_rejects_failed_change_and_keeps_state() {
        let mut session = session(&[b"A-A", b"AAA"]);
        session
            .update(Position::default(), |state| state.set_gap_filter(Some(0.0)))
            .unwrap();

        let result = session.update(Position::default(), ViewState::toggle_translation_overlay);

        assert!(result.is_err());
        assert_eq!(session.state().mode().filter().max_gap_fraction, Some(0.0));
        assert_eq!(session.layout().columns(), [0, 2]);
    }

    #[rstest]
    #[case::plain(|_: &mut ViewState| Ok(()), 3..6)]
    #[case::translation_overlay(ViewState::toggle_translation_overlay, 3..6)]
    #[case::protein_view(ViewState::toggle_protein_view, 1..2)]
    fn column_range_at_works(#[case] change: Change, #[case] expected: Range<usize>) {
        let mut session = session(&[b"ATGATGATG"]);
        session.update(Position::default(), change).unwrap();

        assert_eq!(session.column_range_at(3..6), expected);
    }

    #[rstest]
    #[case::widens_both_ends(libmsa::ReadingFrame::Frame1, 1..=4, 0..=5)]
    #[case::whole_codon(libmsa::ReadingFrame::Frame1, 3..=5, 3..=5)]
    #[case::before_offset(libmsa::ReadingFrame::Frame3, 0..=1, 0..=1)]
    #[case::from_before_offset(libmsa::ReadingFrame::Frame3, 1..=3, 1..=4)]
    #[case::clipped_to_width(libmsa::ReadingFrame::Frame1, 9..=9, 9..=9)]
    fn selection_columns_widens_to_codons_in_overlay(
        #[case] frame: libmsa::ReadingFrame,
        #[case] columns: std::ops::RangeInclusive<usize>,
        #[case] expected: std::ops::RangeInclusive<usize>,
    ) {
        let mut session = session(&[b"ATGATGATGA"]);
        session
            .update(Position::default(), |state| {
                state.frame = frame;
                state.toggle_translation_overlay()
            })
            .unwrap();

        assert_eq!(
            session.selection_columns(columns.into()),
            RangeInclusive::from(expected)
        );
    }

    #[test]
    fn selection_columns_unchanged_outside_overlay() {
        let session = session(&[b"ATGATGATGA"]);

        assert_eq!(
            session.selection_columns((1..=4).into()),
            RangeInclusive::from(1..=4)
        );
    }

    #[test]
    fn window_columns_shows_filtered_columns_in_plain() {
        let mut session = session(&[b"A-A-A", b"AAAAA"]);
        session
            .update(Position::default(), |state| state.set_gap_filter(Some(0.0)))
            .unwrap();

        let window = session.window_columns(1..3);

        assert_eq!(window.columns, [2, 4]);
        assert_eq!(
            window.cells,
            [
                Some(Cell {
                    index: 0,
                    centre: true
                }),
                Some(Cell {
                    index: 1,
                    centre: true
                }),
            ]
        );
    }

    #[rstest]
    #[case::from_start(0..8, 0..3, &[None, Some((0, false)), Some((0, true)), Some((0, false)), Some((1, false)), Some((1, true)), Some((1, false)), Some((2, true))])]
    #[case::from_middle(4..8, 1..3, &[Some((0, false)), Some((0, true)), Some((0, false)), Some((1, true))])]
    fn window_columns_works_in_translation_overlay(
        #[case] window: Range<usize>,
        #[case] expected_columns: Range<usize>,
        #[case] expected_cells: &[Option<(usize, bool)>],
    ) {
        let mut session = session(&[b"AATGATGA"]);
        session
            .update(Position::default(), |state| {
                state.frame = libmsa::ReadingFrame::Frame2;
                state.toggle_translation_overlay()
            })
            .unwrap();

        let window = session.window_columns(window);

        assert_eq!(window.columns, expected_columns.collect::<Vec<_>>());
        assert_eq!(
            window.cells,
            expected_cells
                .iter()
                .map(|cell| cell.map(|(index, centre)| Cell { index, centre }))
                .collect::<Vec<_>>()
        );
    }

    #[rstest]
    #[case::plain(|_: &mut ViewState| Ok(()))]
    #[case::translation_overlay(ViewState::toggle_translation_overlay)]
    fn window_columns_is_empty(#[case] change: Change) {
        let mut session = session(&[b"ATG"]);
        session.update(Position::default(), change).unwrap();

        let window = session.window_columns(1..1);

        assert!(
            window.columns.is_empty() && window.cells.is_empty() && window.summaries.is_empty()
        );
    }

    #[rstest]
    #[case::plain(|state: &mut ViewState| {
        state.pin(1);
        state.pin(2);
        state.row_regex_filter = Some(regex::Regex::new("s0").unwrap());
        Ok(())
    }, b'C')]
    #[case::translation_overlay(ViewState::toggle_translation_overlay, b'M')]
    fn window_columns_summarises_shown_rows(#[case] change: Change, #[case] expected: u8) {
        let mut session = session(&[b"ATG", b"CCC", b"CCC", b"ATG", b"ATG"]);
        session.update(Position::default(), change).unwrap();

        let window = session.window_columns(0..1);

        assert_eq!(window.summaries.len(), 1);
        assert_eq!(window.summaries[0].consensus, Some(expected));
    }
}
