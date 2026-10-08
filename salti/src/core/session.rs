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
    state: ViewState,
    layout: Layout,
    pub diff_mode: DiffMode,
    pub consensus_method: libmsa::ConsensusMethod,
}

impl Session {
    pub fn new(base_alignment: libmsa::Alignment) -> Self {
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
            state,
            layout,
            diff_mode: DiffMode::default(),
            consensus_method: libmsa::ConsensusMethod::default(),
        }
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
