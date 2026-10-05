use crate::core::model::DiffMode;

#[derive(Debug)]
pub enum ViewMode {
    Default,
    QuickTranslate,
    FullTranslate,
}

#[derive(Debug)]
pub struct ViewState {
    alignment_type: libmsa::AlignmentType,
    mode: ViewMode,
    frame: libmsa::ReadingFrame,
    filter: libmsa::ColumnFilter,
    reference: Option<usize>,
    pinned: Vec<usize>,
    row_regex_filter: Option<regex::Regex>,
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
struct Session {
    base_alignment: libmsa::Alignment,
    state: ViewState,
    pub diff_mode: DiffMode,
    pub consensus_method: libmsa::ConsensusMethod,
}

impl Session {
    pub fn new(base_alignment: libmsa::Alignment) -> Self {
        todo!()
    }

    pub fn state(&self) -> &ViewState {
        &self.state
    }

    pub fn base_alignment(&self) -> &libmsa::Alignment {
        &self.base_alignment
    }

    pub fn grid(&self) -> libmsa::Grid<'_> {
        self.state.grid(&self.base_alignment)
    }
}
