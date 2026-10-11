use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::command::Command;

struct Binding {
    code: KeyCode,
    modifiers: KeyModifiers,
    action: Command,
    #[allow(dead_code)]
    help: &'static str,
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
        action: Command::OpenPalette,
        help: "Open command palette",
    },
    Binding {
        code: KeyCode::Char('t'),
        modifiers: KeyModifiers::NONE,
        action: Command::ToggleTranslationOverlay,
        help: "Toggle the translation overlay",
    },
    Binding {
        code: KeyCode::Char('T'),
        modifiers: KeyModifiers::NONE,
        action: Command::ToggleProteinView,
        help: "Toggle the protein view",
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
        action: Command::OpenMinimap,
        help: "Open minimap",
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
        action: Command::JumpToIndex(0),
        help: "Jump to start of alignment",
    },
    Binding {
        code: KeyCode::End,
        modifiers: KeyModifiers::NONE,
        action: Command::JumpToEnd,
        help: "Jump to end of alignment",
    },
];

pub(crate) fn lookup(key: KeyEvent) -> Option<Command> {
    let mut modifiers = key.modifiers;
    if let KeyCode::Char(_) = key.code {
        modifiers.remove(KeyModifiers::SHIFT);
    }
    KEY_BINDINGS
        .iter()
        .find(|binding| binding.code == key.code && binding.modifiers == modifiers)
        .map(|binding| binding.action.clone())
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::letter(KeyCode::Char('T'), KeyModifiers::NONE, Command::ToggleProteinView)]
    #[case::shifted_letter(KeyCode::Char('T'), KeyModifiers::SHIFT, Command::ToggleProteinView)]
    #[case::shifted_punctuation(KeyCode::Char(':'), KeyModifiers::SHIFT, Command::OpenPalette)]
    #[case::arrow(KeyCode::Up, KeyModifiers::NONE, Command::ScrollUp { amount: 1 })]
    #[case::shifted_arrow(KeyCode::Up, KeyModifiers::SHIFT, Command::ScrollUp { amount: 10 })]
    fn lookup_works(
        #[case] code: KeyCode,
        #[case] modifiers: KeyModifiers,
        #[case] expected: Command,
    ) {
        assert_eq!(lookup(KeyEvent::new(code, modifiers)), Some(expected));
    }

    #[rstest]
    #[case::unbound(KeyCode::Char('z'), KeyModifiers::NONE)]
    #[case::control(KeyCode::Char('t'), KeyModifiers::CONTROL)]
    fn lookup_rejects(#[case] code: KeyCode, #[case] modifiers: KeyModifiers) {
        assert_eq!(lookup(KeyEvent::new(code, modifiers)), None);
    }
}
