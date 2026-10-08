use std::{fmt, ops::Range, range::RangeInclusive, str::FromStr};

use anyhow::Result;

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

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ViewMode {
    Plain,
    TranslationOverlay,
    ProteinView,
}

#[derive(Debug, Clone)]
pub struct ViewState {
    pub alignment_type: libmsa::AlignmentType,
    pub mode: ViewMode,
    pub frame: libmsa::ReadingFrame,
    pub filter: libmsa::ColumnFilter,
    pub reference: Option<usize>,
    pub pinned: Vec<usize>,
    pub row_regex_filter: Option<regex::Regex>,
}

impl ViewState {
    pub(super) fn grid<'a>(&self, base_alignment: &'a libmsa::Alignment) -> libmsa::Grid<'a> {
        match self.mode {
            ViewMode::ProteinView => base_alignment.translated_grid(self.frame),
            ViewMode::Plain | ViewMode::TranslationOverlay => {
                base_alignment.grid(self.alignment_type)
            }
        }
    }
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
    state: ViewState,
    layout: Layout,
    pub diff_mode: DiffMode,
    pub consensus_method: libmsa::ConsensusMethod,
}

impl Session {
    pub fn new(base_alignment: libmsa::Alignment) -> Self {
        let state = ViewState {
            alignment_type: base_alignment.detected_type(),
            mode: ViewMode::Plain,
            frame: libmsa::ReadingFrame::Frame1,
            filter: libmsa::ColumnFilter::default(),
            reference: None,
            pinned: Vec::new(),
            row_regex_filter: None,
        };

        let layout =
            Layout::build(&base_alignment, &state).expect("default state should always build");

        Self {
            base_alignment,
            state,
            layout,
            diff_mode: DiffMode::default(),
            consensus_method: libmsa::ConsensusMethod::default(),
        }
    }

    pub fn update(
        &mut self,
        position: Position,
        change: impl FnOnce(&mut ViewState),
    ) -> Result<Position> {
        let columns = self.layout.columns();
        let nt = self.nt_start(columns[position.column.min(columns.len() - 1)]);
        let top = self.layout.main().get(position.row).copied();

        let mut next = self.state.clone();
        change(&mut next);
        self.layout = Layout::build(&self.base_alignment, &next)?;
        self.state = next;

        Ok(Position {
            column: self
                .layout
                .column_position(self.column_range_at(nt..nt + 1).start),
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

    pub fn nt_start(&self, column: usize) -> usize {
        match self.state.mode {
            ViewMode::ProteinView => self.state.frame.nt_range(column).start,
            ViewMode::Plain | ViewMode::TranslationOverlay => column,
        }
    }

    pub fn protein_columns(&self, nt: Range<usize>) -> Range<usize> {
        self.state
            .frame
            .protein_range(nt, self.base_alignment.width())
    }

    pub fn column_range_at(&self, nt: Range<usize>) -> Range<usize> {
        match self.state.mode {
            ViewMode::ProteinView => self.protein_columns(nt),
            ViewMode::Plain | ViewMode::TranslationOverlay => nt,
        }
    }

    pub fn codon_columns(&self, columns: RangeInclusive<usize>) -> RangeInclusive<usize> {
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
