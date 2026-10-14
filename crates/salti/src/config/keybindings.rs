use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::command::Command;

pub(crate) struct Binding {
    // unsure if to add keybinds config file - if so name will be useful/needed
    #[allow(dead_code)]
    name: &'static str,
    code: KeyCode,
    modifiers: KeyModifiers,
    command: Command,
    pub(crate) category: Category,
    pub(crate) description: &'static str,
}

impl Binding {
    pub(crate) fn key(&self) -> String {
        if self.modifiers.is_empty() {
            self.code.to_string()
        } else {
            format!("{}+{}", self.modifiers, self.code)
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, strum::Display, strum::VariantArray)]
pub(crate) enum Category {
    #[default]
    General,
    Navigation,
    Translation,
}

pub(crate) const KEY_BINDINGS: &[Binding] = &[
    Binding {
        name: "quit",
        code: KeyCode::Char('q'),
        modifiers: KeyModifiers::NONE,
        command: Command::Quit,
        category: Category::General,
        description: "Quit application",
    },
    Binding {
        name: "open-palette",
        code: KeyCode::Char(':'),
        modifiers: KeyModifiers::NONE,
        command: Command::OpenPalette,
        category: Category::General,
        description: "Open command palette",
    },
    Binding {
        name: "keybinds",
        code: KeyCode::Char('?'),
        modifiers: KeyModifiers::NONE,
        command: Command::OpenKeybinds,
        category: Category::General,
        description: "Show key bindings",
    },
    Binding {
        name: "open-minimap",
        code: KeyCode::Char('m'),
        modifiers: KeyModifiers::NONE,
        command: Command::OpenMinimap,
        category: Category::General,
        description: "Open minimap",
    },
    Binding {
        name: "cycle-diff-mode",
        code: KeyCode::Char('d'),
        modifiers: KeyModifiers::NONE,
        command: Command::CycleDiffMode,
        category: Category::General,
        description: "Cycle diff mode",
    },
    Binding {
        name: "scroll-up",
        code: KeyCode::Up,
        modifiers: KeyModifiers::NONE,
        command: Command::ScrollUp { amount: 1 },
        category: Category::Navigation,
        description: "Scroll up",
    },
    Binding {
        name: "scroll-down",
        code: KeyCode::Down,
        modifiers: KeyModifiers::NONE,
        command: Command::ScrollDown { amount: 1 },
        category: Category::Navigation,
        description: "Scroll down",
    },
    Binding {
        name: "scroll-left",
        code: KeyCode::Left,
        modifiers: KeyModifiers::NONE,
        command: Command::ScrollLeft { amount: 1 },
        category: Category::Navigation,
        description: "Scroll left",
    },
    Binding {
        name: "scroll-right",
        code: KeyCode::Right,
        modifiers: KeyModifiers::NONE,
        command: Command::ScrollRight { amount: 1 },
        category: Category::Navigation,
        description: "Scroll right",
    },
    Binding {
        name: "scroll-up-fast",
        code: KeyCode::Up,
        modifiers: KeyModifiers::SHIFT,
        command: Command::ScrollUp { amount: 10 },
        category: Category::Navigation,
        description: "Fast scroll up",
    },
    Binding {
        name: "scroll-down-fast",
        code: KeyCode::Down,
        modifiers: KeyModifiers::SHIFT,
        command: Command::ScrollDown { amount: 10 },
        category: Category::Navigation,
        description: "Fast scroll down",
    },
    Binding {
        name: "scroll-left-fast",
        code: KeyCode::Left,
        modifiers: KeyModifiers::SHIFT,
        command: Command::ScrollLeft { amount: 10 },
        category: Category::Navigation,
        description: "Fast scroll left",
    },
    Binding {
        name: "scroll-right-fast",
        code: KeyCode::Right,
        modifiers: KeyModifiers::SHIFT,
        command: Command::ScrollRight { amount: 10 },
        category: Category::Navigation,
        description: "Fast scroll right",
    },
    Binding {
        name: "scroll-names-left",
        code: KeyCode::Left,
        modifiers: KeyModifiers::ALT,
        command: Command::ScrollNamesLeft { amount: 1 },
        category: Category::Navigation,
        description: "Scroll names left",
    },
    Binding {
        name: "scroll-names-right",
        code: KeyCode::Right,
        modifiers: KeyModifiers::ALT,
        command: Command::ScrollNamesRight { amount: 1 },
        category: Category::Navigation,
        description: "Scroll names right",
    },
    Binding {
        name: "jump-start",
        code: KeyCode::Home,
        modifiers: KeyModifiers::NONE,
        command: Command::JumpToIndex(0),
        category: Category::Navigation,
        description: "Jump to start of alignment",
    },
    Binding {
        name: "jump-end",
        code: KeyCode::End,
        modifiers: KeyModifiers::NONE,
        command: Command::JumpToEnd,
        category: Category::Navigation,
        description: "Jump to end of alignment",
    },
    Binding {
        name: "toggle-translation-overlay",
        code: KeyCode::Char('t'),
        modifiers: KeyModifiers::NONE,
        command: Command::ToggleTranslationOverlay,
        category: Category::Translation,
        description: "Toggle the translation overlay",
    },
    Binding {
        name: "toggle-protein-view",
        code: KeyCode::Char('T'),
        modifiers: KeyModifiers::NONE,
        command: Command::ToggleProteinView,
        category: Category::Translation,
        description: "Toggle the protein view",
    },
    Binding {
        name: "set-translation-frame-1",
        code: KeyCode::Char('1'),
        modifiers: KeyModifiers::ALT,
        command: Command::SetTranslationFrame(libmsa::ReadingFrame::Frame1),
        category: Category::Translation,
        description: "Set translation frame +1",
    },
    Binding {
        name: "set-translation-frame-2",
        code: KeyCode::Char('2'),
        modifiers: KeyModifiers::ALT,
        command: Command::SetTranslationFrame(libmsa::ReadingFrame::Frame2),
        category: Category::Translation,
        description: "Set translation frame +2",
    },
    Binding {
        name: "set-translation-frame-3",
        code: KeyCode::Char('3'),
        modifiers: KeyModifiers::ALT,
        command: Command::SetTranslationFrame(libmsa::ReadingFrame::Frame3),
        category: Category::Translation,
        description: "Set translation frame +3",
    },
    Binding {
        name: "set-translation-frame-reverse-1",
        code: KeyCode::Char('4'),
        modifiers: KeyModifiers::ALT,
        command: Command::SetTranslationFrame(libmsa::ReadingFrame::Frame1Reverse),
        category: Category::Translation,
        description: "Set translation frame -1",
    },
    Binding {
        name: "set-translation-frame-reverse-2",
        code: KeyCode::Char('5'),
        modifiers: KeyModifiers::ALT,
        command: Command::SetTranslationFrame(libmsa::ReadingFrame::Frame2Reverse),
        category: Category::Translation,
        description: "Set translation frame -2",
    },
    Binding {
        name: "set-translation-frame-reverse-3",
        code: KeyCode::Char('6'),
        modifiers: KeyModifiers::ALT,
        command: Command::SetTranslationFrame(libmsa::ReadingFrame::Frame3Reverse),
        category: Category::Translation,
        description: "Set translation frame -3",
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
        .map(|binding| binding.command.clone())
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::letter(KeyCode::Char('T'), KeyModifiers::NONE, Command::ToggleProteinView)]
    #[case::shifted_letter(KeyCode::Char('T'), KeyModifiers::SHIFT, Command::ToggleProteinView)]
    #[case::shifted_punctuation(KeyCode::Char(':'), KeyModifiers::SHIFT, Command::OpenPalette)]
    #[case::shifted_question_mark(KeyCode::Char('?'), KeyModifiers::SHIFT, Command::OpenKeybinds)]
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

    #[test]
    fn key_bindings_are_unique() {
        let mut names = HashSet::new();
        let mut keys = HashSet::new();
        for binding in KEY_BINDINGS {
            assert!(
                names.insert(binding.name),
                "duplicate name {}",
                binding.name
            );
            assert!(
                keys.insert((binding.code, binding.modifiers)),
                "duplicate key for {}",
                binding.name
            );
            assert!(
                !(matches!(binding.code, KeyCode::Char(_))
                    && binding.modifiers.contains(KeyModifiers::SHIFT)),
                "{} binds shift on a character",
                binding.name
            );
        }
    }

    #[rstest]
    #[case::arrow(KeyCode::Up, KeyModifiers::NONE, "Up")]
    #[case::shifted_arrow(KeyCode::Up, KeyModifiers::SHIFT, "Shift+Up")]
    #[case::letter(KeyCode::Char('T'), KeyModifiers::NONE, "T")]
    #[case::punctuation(KeyCode::Char('?'), KeyModifiers::NONE, "?")]
    fn key_works(#[case] code: KeyCode, #[case] modifiers: KeyModifiers, #[case] expected: &str) {
        let binding = Binding {
            name: "test",
            code,
            modifiers,
            command: Command::Quit,
            category: Category::General,
            description: "",
        };
        assert_eq!(binding.key(), expected);
    }
}
