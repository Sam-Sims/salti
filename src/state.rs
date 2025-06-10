use crate::config::schemes::ColorSchemeFormatter;
use crate::parser::Alignment;
use std::path::PathBuf;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum LoadingState {
    #[default]
    Loading,
    Loaded,
}

impl std::fmt::Display for LoadingState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoadingState::Loading => write!(f, "Status: Loading alignments..."),
            LoadingState::Loaded => write!(f, "Status: Loaded"),
        }
    }
}

#[derive(Debug)]
pub struct State {
    pub alignments: Vec<Alignment>,
    pub file_path: Option<PathBuf>,
    pub sequence_length: usize,
    pub loading_state: LoadingState,
    pub consensus: Option<Vec<u8>>,
    pub color_scheme_manager: ColorSchemeFormatter,
    consensus_receiver: tokio::sync::watch::Receiver<Vec<u8>>,
}

impl State {
    pub fn new(consensus_receiver: tokio::sync::watch::Receiver<Vec<u8>>) -> Self {
        Self {
            alignments: Vec::new(),
            file_path: None,
            sequence_length: 0,
            loading_state: LoadingState::default(),
            consensus: None,
            color_scheme_manager: ColorSchemeFormatter::default(),
            consensus_receiver,
        }
    }
    pub fn load_alignments(&mut self, alignments: Vec<Alignment>) {
        self.loading_state = LoadingState::Loaded;
        self.sequence_length = alignments.first().map_or(0, |a| a.sequence.len());
        self.alignments = alignments;
    }
    pub fn cycle_color_scheme(&mut self) {
        self.color_scheme_manager.cycle_scheme();
    }

    pub fn check_consensus_updates(&mut self) {
        if self.consensus_receiver.has_changed().unwrap_or(false) {
            let new_consensus = self.consensus_receiver.borrow_and_update().clone();
            if !new_consensus.is_empty() {
                self.consensus = Some(new_consensus);
            }
        }
    }
}
