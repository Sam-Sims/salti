use crate::app::AppMode;
use crossterm::event::{KeyCode, KeyModifiers};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyAction {
    Quit,
    ScrollDown,
    SkipDown,
    ScrollUp,
    SkipUp,
    ScrollLeft,
    ScrollRight,
    SkipLeft,
    SkipRight,
    JumpToStart,
    JumpToEnd,
    ToggleHelp,
    ToggleJump,
    CycleColorScheme,
    // Widget actions
    CloseWidget,
    // Jump widget actions
    RunJump,
    JumpInputChar(char),
    JumpInputBackspace,
    JumpToggleMode,
    JumpMoveUp,
    JumpMoveDown,
}

#[derive(Debug, Clone)]
pub struct KeyBinding {
    pub key: KeyCode,
    pub modifiers: KeyModifiers,
    pub description: String,
    pub alt_key: Option<KeyCode>,
    pub category: KeyBindingCategory,
    pub action: KeyAction,
    pub mode: AppMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyBindingCategory {
    Navigation,
    Application,
    Widget,
}

impl fmt::Display for KeyBindingCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Navigation => write!(f, "Navigation"),
            Self::Application => write!(f, "Application"),
            Self::Widget => write!(f, "Widget"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct KeyBindings {
    pub bindings: Vec<KeyBinding>,
}

impl KeyBindings {
    pub fn loookup_binding_context(
        &self,
        key: KeyCode,
        modifiers: KeyModifiers,
        mode: AppMode,
    ) -> Option<&KeyBinding> {
        self.bindings.iter().find(|binding| {
            (binding.key == key || binding.alt_key == Some(key))
                && binding.modifiers == modifiers
                && binding.mode == mode
        })
    }
}

impl Default for KeyBindings {
    #[allow(clippy::too_many_lines)]
    fn default() -> Self {
        let bindings = vec![
            KeyBinding {
                key: KeyCode::Char('q'),
                modifiers: KeyModifiers::NONE,
                alt_key: None,
                description: "Quit application".into(),
                category: KeyBindingCategory::Application,
                action: KeyAction::Quit,
                mode: AppMode::Alignment,
            },
            KeyBinding {
                key: KeyCode::Down,
                modifiers: KeyModifiers::NONE,
                alt_key: None,
                description: "Scroll down".into(),
                category: KeyBindingCategory::Navigation,
                action: KeyAction::ScrollDown,
                mode: AppMode::Alignment,
            },
            KeyBinding {
                key: KeyCode::Up,
                modifiers: KeyModifiers::NONE,
                alt_key: None,
                description: "Scroll up".into(),
                category: KeyBindingCategory::Navigation,
                action: KeyAction::ScrollUp,
                mode: AppMode::Alignment,
            },
            KeyBinding {
                key: KeyCode::Left,
                modifiers: KeyModifiers::NONE,
                alt_key: None,
                description: "Scroll left".into(),
                category: KeyBindingCategory::Navigation,
                action: KeyAction::ScrollLeft,
                mode: AppMode::Alignment,
            },
            KeyBinding {
                key: KeyCode::Right,
                modifiers: KeyModifiers::NONE,
                alt_key: None,
                description: "Scroll right".into(),
                category: KeyBindingCategory::Navigation,
                action: KeyAction::ScrollRight,
                mode: AppMode::Alignment,
            },
            KeyBinding {
                key: KeyCode::Down,
                modifiers: KeyModifiers::SHIFT,
                alt_key: None,
                description: "Fast scroll down".into(),
                category: KeyBindingCategory::Navigation,
                action: KeyAction::SkipDown,
                mode: AppMode::Alignment,
            },
            KeyBinding {
                key: KeyCode::Up,
                modifiers: KeyModifiers::SHIFT,
                alt_key: None,
                description: "Fast scroll up".into(),
                category: KeyBindingCategory::Navigation,
                action: KeyAction::SkipUp,
                mode: AppMode::Alignment,
            },
            KeyBinding {
                key: KeyCode::Left,
                modifiers: KeyModifiers::SHIFT,
                alt_key: None,
                description: "Fast scroll left".into(),
                category: KeyBindingCategory::Navigation,
                action: KeyAction::SkipLeft,
                mode: AppMode::Alignment,
            },
            KeyBinding {
                key: KeyCode::Right,
                modifiers: KeyModifiers::SHIFT,
                alt_key: None,
                description: "Fast scroll right".into(),
                category: KeyBindingCategory::Navigation,
                action: KeyAction::SkipRight,
                mode: AppMode::Alignment,
            },
            KeyBinding {
                key: KeyCode::Home,
                modifiers: KeyModifiers::NONE,
                alt_key: None,
                description: "Jump to start of alignment".into(),
                category: KeyBindingCategory::Navigation,
                action: KeyAction::JumpToStart,
                mode: AppMode::Alignment,
            },
            KeyBinding {
                key: KeyCode::End,
                modifiers: KeyModifiers::NONE,
                alt_key: None,
                description: "Jump to end of alignment".into(),
                category: KeyBindingCategory::Navigation,
                action: KeyAction::JumpToEnd,
                mode: AppMode::Alignment,
            },
            KeyBinding {
                key: KeyCode::Char('?'),
                modifiers: KeyModifiers::NONE,
                alt_key: None,
                description: "Toggle this help menu".into(),
                category: KeyBindingCategory::Application,
                action: KeyAction::ToggleHelp,
                mode: AppMode::Alignment,
            },
            KeyBinding {
                key: KeyCode::Char('j'),
                modifiers: KeyModifiers::NONE,
                alt_key: None,
                description: "Jump to a position".into(),
                category: KeyBindingCategory::Application,
                action: KeyAction::ToggleJump,
                mode: AppMode::Alignment,
            },
            KeyBinding {
                key: KeyCode::Char('c'),
                modifiers: KeyModifiers::NONE,
                alt_key: None,
                description: "Cycle color scheme".into(),
                category: KeyBindingCategory::Application,
                action: KeyAction::CycleColorScheme,
                mode: AppMode::Alignment,
            },
            // Help widget bindings
            KeyBinding {
                key: KeyCode::Esc,
                modifiers: KeyModifiers::NONE,
                alt_key: None,
                description: "Close help".into(),
                category: KeyBindingCategory::Widget,
                action: KeyAction::CloseWidget,
                mode: AppMode::Help,
            },
            // Jump widget bindings
            KeyBinding {
                key: KeyCode::Esc,
                modifiers: KeyModifiers::NONE,
                alt_key: None,
                description: "Cancel jump".into(),
                category: KeyBindingCategory::Widget,
                action: KeyAction::CloseWidget,
                mode: AppMode::Jump,
            },
            KeyBinding {
                key: KeyCode::Enter,
                modifiers: KeyModifiers::NONE,
                alt_key: None,
                description: "Execute jump".into(),
                category: KeyBindingCategory::Widget,
                action: KeyAction::RunJump,
                mode: AppMode::Jump,
            },
            KeyBinding {
                key: KeyCode::Backspace,
                modifiers: KeyModifiers::NONE,
                alt_key: None,
                description: "Delete character".into(),
                category: KeyBindingCategory::Widget,
                action: KeyAction::JumpInputBackspace,
                mode: AppMode::Jump,
            },
            KeyBinding {
                key: KeyCode::Tab,
                modifiers: KeyModifiers::NONE,
                alt_key: None,
                description: "Switch input mode".into(),
                category: KeyBindingCategory::Widget,
                action: KeyAction::JumpToggleMode,
                mode: AppMode::Jump,
            },
            KeyBinding {
                key: KeyCode::Up,
                modifiers: KeyModifiers::NONE,
                alt_key: None,
                description: "Move selection up".into(),
                category: KeyBindingCategory::Widget,
                action: KeyAction::JumpMoveUp,
                mode: AppMode::Jump,
            },
            KeyBinding {
                key: KeyCode::Down,
                modifiers: KeyModifiers::NONE,
                alt_key: None,
                description: "Move selection down".into(),
                category: KeyBindingCategory::Widget,
                action: KeyAction::JumpMoveDown,
                mode: AppMode::Jump,
            },
        ];

        Self { bindings }
    }
}

pub fn format_key_for_display(key: KeyCode, modifiers: KeyModifiers) -> String {
    let key_str = match key {
        KeyCode::Esc => "Esc".to_string(),
        KeyCode::Char(' ') => "Space".to_string(),
        KeyCode::Char(c) => c.to_string(),
        KeyCode::Up => "↑".to_string(),
        KeyCode::Down => "↓".to_string(),
        KeyCode::Left => "←".to_string(),
        KeyCode::Right => "→".to_string(),
        KeyCode::Home => "Home".to_string(),
        KeyCode::End => "End".to_string(),
        _ => format!("{key:?}"),
    };

    if modifiers.contains(KeyModifiers::SHIFT) {
        format!("Shift + {key_str}")
    } else {
        key_str
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_bindings() {
        let keybindings = KeyBindings::default();
        let binding = keybindings.loookup_binding_context(
            KeyCode::Char('q'),
            KeyModifiers::NONE,
            AppMode::Alignment,
        );
        assert!(binding.is_some());
        assert_eq!(binding.unwrap().action, KeyAction::Quit);
        let binding = keybindings.loookup_binding_context(
            KeyCode::Char('q'),
            KeyModifiers::NONE,
            AppMode::Help,
        );
        assert!(binding.is_none());
        let binding = keybindings.loookup_binding_context(
            KeyCode::Char('z'),
            KeyModifiers::NONE,
            AppMode::Alignment,
        );
        assert!(binding.is_none());
    }

    #[test]
    fn test_alt_key() {
        let mut keybindings = KeyBindings::default();
        keybindings.bindings.push(KeyBinding {
            key: KeyCode::Char('a'),
            modifiers: KeyModifiers::NONE,
            alt_key: Some(KeyCode::Char('b')),
            description: "Test binding".into(),
            category: KeyBindingCategory::Application,
            action: KeyAction::Quit,
            mode: AppMode::Alignment,
        });
        let binding = keybindings.loookup_binding_context(
            KeyCode::Char('a'),
            KeyModifiers::NONE,
            AppMode::Alignment,
        );
        assert!(binding.is_some());
        let binding = keybindings.loookup_binding_context(
            KeyCode::Char('b'),
            KeyModifiers::NONE,
            AppMode::Alignment,
        );
        assert!(binding.is_some());
    }

    #[test]
    fn test_modifier_key() {
        let keybindings = KeyBindings::default();
        let binding = keybindings.loookup_binding_context(
            KeyCode::Down,
            KeyModifiers::SHIFT,
            AppMode::Alignment,
        );
        assert!(binding.is_some());
        assert_eq!(binding.unwrap().action, KeyAction::SkipDown);
        let binding = keybindings.loookup_binding_context(
            KeyCode::Down,
            KeyModifiers::NONE,
            AppMode::Alignment,
        );
        assert!(binding.is_some());
        assert_eq!(binding.unwrap().action, KeyAction::ScrollDown);
    }
}
