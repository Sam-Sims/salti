use crossterm::event::{KeyModifiers, MouseEvent, MouseEventKind};

use crate::{
    cli::StartupState,
    core::session::{Position, Session},
    ui::ui_state::UiState,
};

pub(crate) fn session(sequences: &[&[u8]]) -> Session {
    Session::new(
        libmsa::Alignment::new(
            sequences
                .iter()
                .enumerate()
                .map(|(i, residues)| libmsa::Sequence {
                    id: format!("s{i}"),
                    residues: residues.to_vec(),
                })
                .collect(),
        )
        .unwrap(),
    )
}

pub(crate) fn pinned_session(row_count: usize, pinned: &[usize]) -> Session {
    let mut session = session(&vec![[b'A'; 20].as_slice(); row_count]);
    session
        .update(Position::default(), |state| {
            for &row in pinned {
                state.pin(row);
            }
            Ok(())
        })
        .unwrap();
    session
}

pub(crate) fn ui_state() -> UiState {
    UiState::new(StartupState::default())
}

pub(crate) fn mouse_event(kind: MouseEventKind, column: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind,
        column,
        row,
        modifiers: KeyModifiers::empty(),
    }
}
