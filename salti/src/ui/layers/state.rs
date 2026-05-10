use crate::ui::layers::minimap::MinimapState;
use crate::ui::layers::palette::CommandPaletteState;

#[derive(Debug)]
pub enum ActiveLayer {
    Palette(Box<CommandPaletteState>),
    Minimap(MinimapState),
}

#[derive(Debug, Default)]
pub struct LayerState {
    pub active: Option<ActiveLayer>,
}

impl LayerState {
    pub fn open_palette(&mut self, palette: CommandPaletteState) {
        self.active = Some(ActiveLayer::Palette(Box::new(palette)));
    }

    pub fn toggle_minimap(&mut self) {
        self.active = match self.active.take() {
            Some(ActiveLayer::Minimap(_)) => None,
            _ => Some(ActiveLayer::Minimap(MinimapState::default())),
        };
    }

    pub fn close_active(&mut self) {
        self.active = None;
    }
}
