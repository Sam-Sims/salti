use crossterm::event::{KeyModifiers, MouseEvent, MouseEventKind};
use ratatui::{buffer::Buffer, layout::Rect};

use crate::{
    cli::StartupState,
    core::session::{Position, Session},
    ui::{layout::Window, ui_state::UiState},
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

pub(crate) fn session_with_ids(sequences: &[(&str, &[u8])]) -> Session {
    Session::new(
        libmsa::Alignment::new(
            sequences
                .iter()
                .map(|&(id, residues)| libmsa::Sequence {
                    id: id.to_string(),
                    residues: residues.to_vec(),
                })
                .collect(),
        )
        .unwrap(),
    )
}

pub(crate) fn full_window(session: &Session) -> Window {
    let layout = session.layout();
    Window {
        pinned: 0..layout.pinned(),
        rows: 0..layout.main().len(),
        columns: 0..layout.columns().len(),
        names: 0..session.base_alignment().max_id_len(),
    }
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

pub(crate) fn buffer_text(buffer: &Buffer, area: Rect) -> String {
    let lines: Vec<String> = area
        .rows()
        .map(|row| {
            row.positions()
                .map(|position| match buffer[position].symbol() {
                    "" => " ",
                    symbol => symbol,
                })
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .collect();
    lines.join("\n").trim_end().to_string()
}
