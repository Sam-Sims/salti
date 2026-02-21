use crossterm::event::{KeyCode, KeyModifiers};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyAction {
    Quit,
    OpenCommandPalette,
    ToggleMinimap,
    ScrollDown,
    SkipDown,
    ScrollUp,
    SkipUp,
    ScrollLeft,
    ScrollRight,
    SkipLeft,
    SkipRight,
    ScrollNamesLeft,
    ScrollNamesRight,
    JumpToStart,
    JumpToEnd,
    ToggleTranslationView,
}

pub struct Binding {
    pub code: KeyCode,
    pub modifiers: KeyModifiers,
    pub action: KeyAction,
    #[allow(dead_code)]
    pub help: &'static str,
}

const KEY_BINDINGS: &[Binding] = &[
    Binding {
        code: KeyCode::Char('q'),
        modifiers: KeyModifiers::NONE,
        action: KeyAction::Quit,
        help: "Quit application",
    },
    Binding {
        code: KeyCode::Char(':'),
        modifiers: KeyModifiers::NONE,
        action: KeyAction::OpenCommandPalette,
        help: "Open command palette",
    },
    Binding {
        code: KeyCode::Char('t'),
        modifiers: KeyModifiers::NONE,
        action: KeyAction::ToggleTranslationView,
        help: "Toggle NT to AA translation view",
    },
    Binding {
        code: KeyCode::Char('m'),
        modifiers: KeyModifiers::NONE,
        action: KeyAction::ToggleMinimap,
        help: "Toggle minimap overlay",
    },
    Binding {
        code: KeyCode::Down,
        modifiers: KeyModifiers::NONE,
        action: KeyAction::ScrollDown,
        help: "Scroll down",
    },
    Binding {
        code: KeyCode::Down,
        modifiers: KeyModifiers::SHIFT,
        action: KeyAction::SkipDown,
        help: "Fast scroll down",
    },
    Binding {
        code: KeyCode::Up,
        modifiers: KeyModifiers::NONE,
        action: KeyAction::ScrollUp,
        help: "Scroll up",
    },
    Binding {
        code: KeyCode::Up,
        modifiers: KeyModifiers::SHIFT,
        action: KeyAction::SkipUp,
        help: "Fast scroll up",
    },
    Binding {
        code: KeyCode::Left,
        modifiers: KeyModifiers::NONE,
        action: KeyAction::ScrollLeft,
        help: "Scroll left",
    },
    Binding {
        code: KeyCode::Left,
        modifiers: KeyModifiers::SHIFT,
        action: KeyAction::SkipLeft,
        help: "Fast scroll left",
    },
    Binding {
        code: KeyCode::Right,
        modifiers: KeyModifiers::NONE,
        action: KeyAction::ScrollRight,
        help: "Scroll right",
    },
    Binding {
        code: KeyCode::Right,
        modifiers: KeyModifiers::SHIFT,
        action: KeyAction::SkipRight,
        help: "Fast scroll right",
    },
    Binding {
        code: KeyCode::Left,
        modifiers: KeyModifiers::ALT,
        action: KeyAction::ScrollNamesLeft,
        help: "Scroll names left",
    },
    Binding {
        code: KeyCode::Right,
        modifiers: KeyModifiers::ALT,
        action: KeyAction::ScrollNamesRight,
        help: "Scroll names right",
    },
    Binding {
        code: KeyCode::Home,
        modifiers: KeyModifiers::NONE,
        action: KeyAction::JumpToStart,
        help: "Jump to start of alignment",
    },
    Binding {
        code: KeyCode::End,
        modifiers: KeyModifiers::NONE,
        action: KeyAction::JumpToEnd,
        help: "Jump to end of alignment",
    },
];

pub fn lookup(code: KeyCode, modifiers: KeyModifiers) -> Option<KeyAction> {
    KEY_BINDINGS
        .iter()
        .find(|binding| binding.code == code && binding.modifiers == modifiers)
        .map(|binding| binding.action)
}
