mod fuzzy;
pub(crate) mod keybinds;
pub(crate) mod minimap;
pub(crate) mod palette;
mod text_input;

use std::fmt::Display;

use anyhow::{Context as _, Result};
use crossterm::event::{KeyCode, KeyEvent, MouseEvent};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    symbols,
    text::{Line, Span},
    widgets::{Block, Clear, Widget},
};
use strum::VariantArray;

use crate::{
    command::Command,
    config::theme::{Theme, ThemeChoice},
    gff::Gff,
    session::Session,
    ui::{
        layout::ScreenLayout,
        popups::{
            keybinds::Keybinds,
            minimap::{Minimap, MinimapView},
            palette::PaletteState,
        },
    },
};

const COLUMN_GAP: usize = 2;

#[derive(Debug)]
pub(crate) enum Popup {
    Palette(PaletteState),
    Minimap(Minimap),
    Keybinds(Keybinds),
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
        let theme = ctx.theme;
        let block = block(content.frame(ctx))
            .border_style(theme.border)
            .style(theme.popup);
        let inner = block.inner(area);
        Clear.render(area, buf);
        block.render(area, buf);
        content.render(inner, buf, ctx);
    }

    pub(super) fn theme_preview(&self) -> Option<ThemeChoice> {
        match self {
            Self::Palette(palette) => palette.theme_preview(),
            Self::Minimap(_) | Self::Keybinds(_) => None,
        }
    }

    fn inner(&self, ctx: &Context) -> Rect {
        let content = self.content();
        block(content.frame(ctx)).inner(content.area(ctx))
    }

    fn content(&self) -> &dyn PopupContent {
        match self {
            Self::Palette(palette) => palette,
            Self::Minimap(minimap) => minimap,
            Self::Keybinds(keybinds) => keybinds,
        }
    }

    fn content_mut(&mut self) -> &mut dyn PopupContent {
        match self {
            Self::Palette(palette) => palette,
            Self::Minimap(minimap) => minimap,
            Self::Keybinds(keybinds) => keybinds,
        }
    }
}

trait PopupContent {
    fn frame(&self, ctx: &Context) -> PopupFrame;
    fn area(&self, ctx: &Context) -> Rect;
    fn handle_key(&mut self, key: KeyEvent, inner: Rect, ctx: &Context) -> Vec<Command>;
    fn handle_mouse(&mut self, _mouse: MouseEvent, _inner: Rect, _ctx: &Context) -> Vec<Command> {
        Vec::new()
    }
    fn render(&self, inner: Rect, buf: &mut Buffer, ctx: &Context);
}

enum PopupFrame {
    Bordered { title: Line<'static> },
    Bare,
}

pub(crate) struct Context<'a> {
    pub(crate) session: Option<&'a Session>,
    pub(crate) gff: Option<&'a Gff>,
    pub(crate) theme: &'a Theme,
    pub(crate) screen: &'a ScreenLayout,
    pub(crate) minimap_view: MinimapView,
}

impl Context<'_> {
    fn session(&self) -> Result<&Session> {
        self.session
            .context("No alignment is loaded. Open one with load-alignment")
    }
}

fn block(frame: PopupFrame) -> Block<'static> {
    match frame {
        PopupFrame::Bordered { title } => Block::bordered().title(title),
        PopupFrame::Bare => Block::new(),
    }
}

fn tab_title<T: VariantArray + Display + Copy + PartialEq>(
    selected: T,
    theme: &Theme,
) -> Line<'static> {
    let mut spans = Vec::new();
    for (index, tab) in T::VARIANTS.iter().enumerate() {
        if index > 0 {
            spans.push(Span::styled(symbols::line::VERTICAL, theme.text_muted));
        }
        let style = if *tab == selected {
            theme.highlight
        } else {
            theme.text_muted
        };
        spans.extend([" ".into(), Span::styled(tab.to_string(), style), " ".into()]);
    }
    Line::from(spans)
}

fn next_tab<T: VariantArray + Copy + PartialEq>(tab: T) -> T {
    step_tab(tab, 1)
}

fn previous_tab<T: VariantArray + Copy + PartialEq>(tab: T) -> T {
    step_tab(tab, T::VARIANTS.len() - 1)
}

fn step_tab<T: VariantArray + Copy + PartialEq>(tab: T, step: usize) -> T {
    let tabs = T::VARIANTS;
    let index = tabs
        .iter()
        .position(|&variant| variant == tab)
        .expect("a tab is one of its variants");
    tabs[(index + step) % tabs.len()]
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::forward(MinimapView::Composition, MinimapView::Entropy)]
    #[case::wraps(MinimapView::Missing, MinimapView::Composition)]
    fn next_tab_works(#[case] tab: MinimapView, #[case] expected: MinimapView) {
        assert_eq!(next_tab(tab), expected);
    }

    #[rstest]
    #[case::backward(MinimapView::Entropy, MinimapView::Composition)]
    #[case::wraps(MinimapView::Composition, MinimapView::Missing)]
    fn previous_tab_works(#[case] tab: MinimapView, #[case] expected: MinimapView) {
        assert_eq!(previous_tab(tab), expected);
    }
}
