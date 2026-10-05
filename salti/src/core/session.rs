use std::{fmt, ops::Range, range::RangeInclusive, str::FromStr};

use anyhow::Result;

use crate::core::{columns::WindowColumns, layout::Layout};

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
    Default,
    QuickTranslate,
    FullTranslate,
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
            ViewMode::FullTranslate => base_alignment.translated_grid(self.frame),
            ViewMode::Default | ViewMode::QuickTranslate => {
                base_alignment.grid(self.alignment_type)
            }
        }
    }
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
        todo!()
    }

    pub fn update(&mut self, change: impl FnOnce(&mut ViewState)) -> Result<()> {
        let mut next = self.state.clone();
        change(&mut next);
        self.layout = Layout::build(&self.base_alignment, &next)?;
        self.state = next;
        Ok(())
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
        todo!()
    }

    pub fn nt_start(&self, column: usize) -> usize {
        //  need a way to get first nt of a grid which will anchor viewport
        todo!()
    }

    pub fn protein_columns(&self, nt: Range<usize>) -> Range<usize> {
        // take a nt range and return the protein coordinates
        todo!()
    }

    pub fn column_range_at(&self, nt: Range<usize>) -> Range<usize> {
        todo!()
    }

    pub fn codon_columns(&self, columns: RangeInclusive<usize>) -> RangeInclusive<usize> {
        // convert columns to the 3 wide codons
        todo!()
    }
}
