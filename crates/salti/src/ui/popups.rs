mod fuzzy;
pub mod keybinds;
pub mod minimap;
pub mod palette;
mod text_input;

use std::fmt::Display;

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, MouseEvent};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
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
            palette::Palette,
        },
    },
};

pub const COLUMN_GAP: usize = 2;

#[derive(Debug)]
pub enum Popup {
    Palette(Palette),
    Minimap(Minimap),
    Keybinds(Keybinds),
}

impl Popup {
    pub fn handle_key(&mut self, key: KeyEvent, ctx: &Context) -> Vec<Command> {
        if key.code == KeyCode::Esc {
            return vec![Command::ClosePopup];
        }
        let inner = self.inner(ctx);
        self.content_mut().handle_key(key, inner, ctx)
    }

    pub fn handle_mouse(&mut self, mouse: MouseEvent, ctx: &Context) -> Vec<Command> {
        let inner = self.inner(ctx);
        if !inner.contains((mouse.column, mouse.row).into()) {
            return Vec::new();
        }
        self.content_mut().handle_mouse(mouse, inner, ctx)
    }

    pub fn render(&self, buf: &mut Buffer, ctx: &Context) {
        let content = self.content();
        let area = content.area(ctx);
        let frame = content.frame();
        let mut block = block(frame)
            .border_style(ctx.theme.border)
            .style(ctx.theme.popup);
        let inner = block.inner(area);
        if frame == PopupFrame::Bordered {
            block = block.title(content.title(inner.width, ctx));
        }
        Clear.render(area, buf);
        block.render(area, buf);
        content.render(inner, buf, ctx);
    }

    pub fn theme_preview(&self) -> Option<ThemeChoice> {
        match self {
            Self::Palette(palette) => palette.theme_preview(),
            Self::Minimap(_) | Self::Keybinds(_) => None,
        }
    }

    fn inner(&self, ctx: &Context) -> Rect {
        let content = self.content();
        block(content.frame()).inner(content.area(ctx))
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

pub struct Context<'frame> {
    pub session: Option<&'frame Session>,
    pub gff: Option<&'frame Gff>,
    pub theme: &'frame Theme,
    pub screen: &'frame ScreenLayout,
    pub minimap_view: MinimapView,
}

impl Context<'_> {
    fn session(&self) -> Result<&Session> {
        use anyhow::Context as _;

        self.session
            .context("No alignment is loaded. Open one with load-alignment")
    }
}

pub trait PopupContent {
    fn frame(&self) -> PopupFrame;
    fn title(&self, _width: u16, _ctx: &Context) -> Line<'static> {
        Line::default()
    }
    fn area(&self, ctx: &Context) -> Rect;
    fn handle_key(&mut self, key: KeyEvent, inner: Rect, ctx: &Context) -> Vec<Command>;
    fn handle_mouse(&mut self, _mouse: MouseEvent, _inner: Rect, _ctx: &Context) -> Vec<Command> {
        Vec::new()
    }
    fn render(&self, inner: Rect, buf: &mut Buffer, ctx: &Context);
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PopupFrame {
    Bordered,
    Bare,
}

pub fn tab_title<T: VariantArray + Display + Copy + PartialEq>(
    selected: T,
    width: u16,
    theme: &Theme,
) -> Line<'static> {
    let label = |tab: T, style: Style| -> [Span<'static>; 3] {
        [" ".into(), Span::styled(tab.to_string(), style), " ".into()]
    };
    let mut spans = Vec::new();
    for (index, &tab) in T::VARIANTS.iter().enumerate() {
        if index > 0 {
            spans.push(Span::styled(symbols::line::VERTICAL, theme.text_muted));
        }
        let style = if tab == selected {
            theme.highlight
        } else {
            theme.text_muted
        };
        spans.extend(label(tab, style));
    }
    let tabs = Line::from(spans);
    if tabs.width() <= usize::from(width) {
        tabs
    } else {
        Line::from_iter(label(selected, theme.highlight))
    }
}

pub fn next_tab<T: VariantArray + Copy + PartialEq>(tab: T) -> T {
    step_tab(tab, 1)
}

pub fn previous_tab<T: VariantArray + Copy + PartialEq>(tab: T) -> T {
    step_tab(tab, T::VARIANTS.len() - 1)
}

fn block(frame: PopupFrame) -> Block<'static> {
    match frame {
        PopupFrame::Bordered => Block::bordered(),
        PopupFrame::Bare => Block::new(),
    }
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

    #[rstest]
    #[case::fits(47, " Composition │ Entropy │ Differences │ Missing ")]
    #[case::too_narrow(46, " Missing ")]
    fn tab_title_works(#[case] width: u16, #[case] expected: &str) {
        assert_eq!(
            tab_title(MinimapView::Missing, width, &Theme::default()).to_string(),
            expected
        );
    }
}
