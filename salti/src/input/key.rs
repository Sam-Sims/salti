use crossterm::event::KeyEvent;

use crate::command::Command;
use crate::config::keybindings;
use crate::input::route::{KeyRoute, route_key};
use crate::overlay::overlay_state::ActiveOverlay;
use crate::ui::ui_state::UiState;

pub(crate) fn handle_key_event(ui: &mut UiState, key: KeyEvent) -> Vec<Command> {
    match route_key(ui) {
        KeyRoute::Palette => match ui.overlay.active_overlay.as_mut() {
            Some(ActiveOverlay::Palette(palette)) => palette.handle_key_event(key),
            _ => Vec::new(),
        },
        KeyRoute::Global => match keybindings::lookup(key.code, key.modifiers) {
            Some(command) => vec![command],
            None => Vec::new(),
        },
    }
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyCode, KeyEvent};

    use super::*;
    use crate::cli::StartupState;
    use crate::overlay::command_palette::CommandPaletteState;

    fn ui_state() -> UiState {
        UiState::new(StartupState {
            file_path: None,
            initial_position: 0,
        })
    }

    #[test]
    fn global_keys_return_binding_command() {
        let mut ui = ui_state();

        let commands = handle_key_event(&mut ui, KeyEvent::from(KeyCode::Char('q')));

        assert_eq!(commands, vec![Command::Quit]);
    }

    #[test]
    fn palette_keys_are_routed_to_palette_state() {
        let mut ui = ui_state();
        ui.overlay.open_palette(CommandPaletteState::empty());

        let commands = handle_key_event(&mut ui, KeyEvent::from(KeyCode::Esc));

        assert_eq!(commands, vec![Command::CloseOverlay]);
    }
}
