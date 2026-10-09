use crossterm::event::KeyEvent;

use crate::{
    command::Command,
    config::keybindings,
    input::route::{KeyRoute, route_key},
    ui::{layers::state::ActiveLayer, ui_state::UiState},
};

pub(crate) fn handle_key_event(ui: &mut UiState, key: KeyEvent) -> Vec<Command> {
    match route_key(ui) {
        KeyRoute::Palette => match ui.layers.active.as_mut() {
            Some(ActiveLayer::Palette(palette)) => palette.handle_key_event(key),
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
    use crate::{test_utils::ui_state, ui::layers::palette::CommandPaletteState};

    #[test]
    fn handle_key_event_uses_keybindings() {
        let mut ui = ui_state();

        let commands = handle_key_event(&mut ui, KeyEvent::from(KeyCode::Char('q')));

        assert_eq!(commands, vec![Command::Quit]);
    }

    #[test]
    fn handle_key_event_sends_keys_to_open_palette() {
        let mut ui = ui_state();
        ui.layers.open_palette(CommandPaletteState::empty());

        let commands = handle_key_event(&mut ui, KeyEvent::from(KeyCode::Esc));

        assert_eq!(commands, vec![Command::CloseOverlay]);
    }
}
