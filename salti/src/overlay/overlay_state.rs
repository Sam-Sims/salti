use super::command_palette::CommandPaletteState;
use super::minimap::MinimapState;

#[derive(Debug)]
pub enum ActiveOverlay {
    Palette(CommandPaletteState),
    Minimap(MinimapState),
}

#[derive(Debug, Default)]
pub struct OverlayState {
    pub active_overlay: Option<ActiveOverlay>,
}

impl OverlayState {
    pub fn open_palette(&mut self, palette: CommandPaletteState) {
        self.active_overlay = Some(ActiveOverlay::Palette(palette));
    }

    pub fn toggle_minimap(&mut self) {
        self.active_overlay = match self.active_overlay.take() {
            Some(ActiveOverlay::Minimap(_)) => None,
            _ => Some(ActiveOverlay::Minimap(MinimapState::default())),
        };
    }

    pub fn close(&mut self) {
        self.active_overlay = None;
    }
}
