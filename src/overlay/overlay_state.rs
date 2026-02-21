use super::{CommandPaletteState, MinimapState, Notification};

#[derive(Debug, Default)]
pub struct OverlayState {
    pub palette: Option<CommandPaletteState>,
    pub minimap: Option<MinimapState>,
    pub notification: Option<Notification>,
}
