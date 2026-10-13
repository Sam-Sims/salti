use std::ops::Range;

use crossterm::event::{KeyModifiers, MouseEvent, MouseEventKind};
use ratatui::{buffer::Buffer, layout::Rect};

use crate::{
    config::theme::Theme,
    core::{
        gff::{Feature, FeatureType, Gff, Strand},
        session::{Position, Session},
    },
    ui::{
        layout::{AppLayout, FrameLayout, ScreenLayout, Window},
        popups::Context,
        state::UiState,
    },
};

pub(crate) fn session(sequences: &[&[u8]]) -> Session {
    let ids: Vec<String> = (0..sequences.len()).map(|i| format!("s{i}")).collect();
    session_from(&ids, sequences)
}

pub(crate) fn session_with_numbered_ids(sequences: &[&[u8]]) -> Session {
    let ids: Vec<String> = (1..=sequences.len()).map(|i| format!("seq{i}")).collect();
    session_from(&ids, sequences)
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
        "test.fasta".to_string(),
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

pub(crate) fn screen(area: Rect, gff_height: u16, window: Window) -> ScreenLayout {
    let frame = FrameLayout::new(area);
    ScreenLayout {
        area,
        frame,
        app: AppLayout::new(frame.content_area, gff_height, 0),
        window,
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

pub(crate) fn genes(ranges: &[Range<usize>]) -> Gff {
    Gff {
        features: ranges
            .iter()
            .enumerate()
            .map(|(i, range)| Feature {
                name: format!("gene{i}"),
                kind: FeatureType::Gene,
                range: range.clone(),
                strand: Strand::Forward,
            })
            .collect(),
    }
}

pub(crate) fn with_context<T>(
    session: Option<&Session>,
    gff: Option<&Gff>,
    check: impl FnOnce(&Context) -> T,
) -> T {
    let theme = Theme::default();
    let screen = ScreenLayout::new(Rect::default(), session, gff, &mut Position::default());
    check(&Context {
        session,
        gff,
        theme: &theme,
        screen: &screen,
    })
}

pub(crate) fn ui_state() -> UiState {
    UiState::default()
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

fn session_from(ids: &[String], sequences: &[&[u8]]) -> Session {
    let sequences: Vec<(&str, &[u8])> = ids
        .iter()
        .map(String::as_str)
        .zip(sequences.iter().copied())
        .collect();
    session_with_ids(&sequences)
}
