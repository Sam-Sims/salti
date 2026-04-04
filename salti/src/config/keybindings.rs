use crossterm::event::{KeyCode, KeyModifiers};

use crate::command::Command;

pub struct Binding {
    pub code: KeyCode,
    pub modifiers: KeyModifiers,
    pub action: Command,
    #[allow(dead_code)]
    pub help: &'static str,
}

const KEY_BINDINGS: &[Binding] = &[
    Binding {
        code: KeyCode::Char('q'),
        modifiers: KeyModifiers::NONE,
        action: Command::Quit,
        help: "Quit application",
    },
    Binding {
        code: KeyCode::Char(':'),
        modifiers: KeyModifiers::NONE,
        action: Command::OpenCommandPalette,
        help: "Open command palette",
    },
    Binding {
        code: KeyCode::Char('t'),
        modifiers: KeyModifiers::NONE,
        action: Command::ToggleTranslationView,
        help: "Toggle NT to AA translation view",
    },
    Binding {
        code: KeyCode::Char('T'),
        modifiers: KeyModifiers::SHIFT,
        action: Command::ReloadAsProtein { frame: None },
        help: "Reload alignment as protein",
    },
    Binding {
        code: KeyCode::Char('1'),
        modifiers: KeyModifiers::ALT,
        action: Command::SetTranslationFrame(libmsa::ReadingFrame::Frame1),
        help: "Set translation frame 1",
    },
    Binding {
        code: KeyCode::Char('2'),
        modifiers: KeyModifiers::ALT,
        action: Command::SetTranslationFrame(libmsa::ReadingFrame::Frame2),
        help: "Set translation frame 2",
    },
    Binding {
        code: KeyCode::Char('3'),
        modifiers: KeyModifiers::ALT,
        action: Command::SetTranslationFrame(libmsa::ReadingFrame::Frame3),
        help: "Set translation frame 3",
    },
    Binding {
        code: KeyCode::Char('m'),
        modifiers: KeyModifiers::NONE,
        action: Command::ToggleMinimap,
        help: "Toggle minimap overlay",
    },
    Binding {
        code: KeyCode::Down,
        modifiers: KeyModifiers::NONE,
        action: Command::ScrollDown { amount: 1 },
        help: "Scroll down",
    },
    Binding {
        code: KeyCode::Down,
        modifiers: KeyModifiers::SHIFT,
        action: Command::ScrollDown { amount: 10 },
        help: "Fast scroll down",
    },
    Binding {
        code: KeyCode::Up,
        modifiers: KeyModifiers::NONE,
        action: Command::ScrollUp { amount: 1 },
        help: "Scroll up",
    },
    Binding {
        code: KeyCode::Up,
        modifiers: KeyModifiers::SHIFT,
        action: Command::ScrollUp { amount: 10 },
        help: "Fast scroll up",
    },
    Binding {
        code: KeyCode::Left,
        modifiers: KeyModifiers::NONE,
        action: Command::ScrollLeft { amount: 1 },
        help: "Scroll left",
    },
    Binding {
        code: KeyCode::Left,
        modifiers: KeyModifiers::SHIFT,
        action: Command::ScrollLeft { amount: 10 },
        help: "Fast scroll left",
    },
    Binding {
        code: KeyCode::Right,
        modifiers: KeyModifiers::NONE,
        action: Command::ScrollRight { amount: 1 },
        help: "Scroll right",
    },
    Binding {
        code: KeyCode::Right,
        modifiers: KeyModifiers::SHIFT,
        action: Command::ScrollRight { amount: 10 },
        help: "Fast scroll right",
    },
    Binding {
        code: KeyCode::Left,
        modifiers: KeyModifiers::ALT,
        action: Command::ScrollNamesLeft { amount: 1 },
        help: "Scroll names left",
    },
    Binding {
        code: KeyCode::Right,
        modifiers: KeyModifiers::ALT,
        action: Command::ScrollNamesRight { amount: 1 },
        help: "Scroll names right",
    },
    Binding {
        code: KeyCode::Home,
        modifiers: KeyModifiers::NONE,
        action: Command::JumpToStart,
        help: "Jump to start of alignment",
    },
    Binding {
        code: KeyCode::End,
        modifiers: KeyModifiers::NONE,
        action: Command::JumpToEnd,
        help: "Jump to end of alignment",
    },
];

pub fn lookup(code: KeyCode, modifiers: KeyModifiers) -> Option<Command> {
    KEY_BINDINGS
        .iter()
        .find(|binding| binding.code == code && binding.modifiers == modifiers)
        .map(|binding| binding.action.clone())
}
