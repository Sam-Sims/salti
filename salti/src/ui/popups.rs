pub(crate) mod minimap;
pub(crate) mod palette;

use crossterm::event::{KeyCode, KeyEvent, MouseEvent};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    widgets::{Block, Clear, Widget},
};

use crate::{
    command::Command,
    core::session::Session,
    ui::{layout::ScreenLayout, popups::minimap::Minimap, state::ThemeState},
};

#[derive(Debug)]
pub(crate) enum Popup {
    Minimap(Minimap),
}

impl Popup {
    pub(crate) fn handle_key(&mut self, key: KeyEvent, ctx: &Context) -> Vec<Command> {
        if key.code == KeyCode::Esc {
            return vec![Command::ClosePopup];
        }
        let inner = self.inner(ctx);
        self.content_mut().handle_key(key, inner, ctx)
    }

    pub(crate) fn handle_mouse(&mut self, mouse: MouseEvent, ctx: &Context) -> Vec<Command> {
        let inner = self.inner(ctx);
        if !inner.contains((mouse.column, mouse.row).into()) {
            return Vec::new();
        }
        self.content_mut().handle_mouse(mouse, inner, ctx)
    }

    pub(crate) fn render(&self, buf: &mut Buffer, ctx: &Context) {
        let content = self.content();
        let area = content.area(ctx);
        let styles = &ctx.theme.styles;
        let block = block(content.frame())
            .border_style(styles.border)
            .style(styles.panel_block);
        let inner = block.inner(area);
        Clear.render(area, buf);
        block.render(area, buf);
        content.render(inner, buf, ctx);
    }

    fn inner(&self, ctx: &Context) -> Rect {
        let content = self.content();
        block(content.frame()).inner(content.area(ctx))
    }

    fn content(&self) -> &dyn PopupContent {
        match self {
            Self::Minimap(minimap) => minimap,
        }
    }

    fn content_mut(&mut self) -> &mut dyn PopupContent {
        match self {
            Self::Minimap(minimap) => minimap,
        }
    }
}

trait PopupContent {
    fn frame(&self) -> PopupFrame<'_>;
    fn area(&self, ctx: &Context) -> Rect;
    fn handle_key(&mut self, key: KeyEvent, inner: Rect, ctx: &Context) -> Vec<Command>;
    fn handle_mouse(&mut self, _mouse: MouseEvent, _inner: Rect, _ctx: &Context) -> Vec<Command> {
        Vec::new()
    }
    fn render(&self, inner: Rect, buf: &mut Buffer, ctx: &Context);
}

#[derive(Clone, Copy)]
enum PopupFrame<'a> {
    Bordered { title: &'a str },
}

pub(crate) struct Context<'a> {
    pub(crate) session: Option<&'a Session>,
    pub(crate) theme: &'a ThemeState,
    pub(crate) screen: &'a ScreenLayout,
}

fn block(frame: PopupFrame<'_>) -> Block<'_> {
    match frame {
        PopupFrame::Bordered { title } => Block::bordered().title(title),
    }
}
