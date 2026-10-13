use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    buffer::{Buffer, CellWidth},
    layout::{Constraint, Rect},
    macros::vertical,
    text::{Line, Span},
    widgets::{Paragraph, Tabs, Widget},
};
use strum::VariantArray;

use crate::{
    command::Command,
    config::keybindings::{Binding, Category, KEY_BINDINGS},
    ui::popups::{COLUMN_GAP, Context, PopupContent, PopupFrame},
};

#[derive(Debug, Default)]
pub(crate) struct Keybinds {
    tab: Category,
    scroll: usize,
}

impl Keybinds {
    fn max_scroll(&self, inner: Rect) -> usize {
        let list_height = usize::from(inner.height.saturating_sub(1));
        bindings_in(self.tab).count().saturating_sub(list_height)
    }

    fn select_tab(&mut self, index: usize) {
        if let Some(&tab) = Category::VARIANTS.get(index) {
            *self = Self { tab, scroll: 0 };
        }
    }
}

impl PopupContent for Keybinds {
    fn frame(&self) -> PopupFrame<'_> {
        PopupFrame::Bordered { title: "Keybinds" }
    }

    fn area(&self, ctx: &Context) -> Rect {
        let description_width = KEY_BINDINGS
            .iter()
            .map(|binding| usize::from(binding.description.cell_width()))
            .max()
            .unwrap_or_default();
        let longest_tab = Category::VARIANTS
            .iter()
            .map(|&tab| bindings_in(tab).count())
            .max()
            .unwrap_or_default();
        let width = key_width() + COLUMN_GAP + description_width + 2;
        let height = longest_tab + 3;
        ctx.screen.frame.overlay_area.centered(
            Constraint::Length(u16::try_from(width).unwrap_or(u16::MAX)),
            Constraint::Length(u16::try_from(height).unwrap_or(u16::MAX)),
        )
    }

    fn handle_key(&mut self, key: KeyEvent, inner: Rect, _: &Context) -> Vec<Command> {
        let index = self.tab as usize;
        match key.code {
            KeyCode::Left if index > 0 => self.select_tab(index - 1),
            KeyCode::Right => self.select_tab(index + 1),
            KeyCode::Up => self.scroll = self.scroll.saturating_sub(1),
            KeyCode::Down => self.scroll = (self.scroll + 1).min(self.max_scroll(inner)),
            _ => {}
        }
        Vec::new()
    }

    fn render(&self, inner: Rect, buf: &mut Buffer, ctx: &Context) {
        let theme = ctx.theme;
        let [tabs_area, list_area] = inner.layout(&vertical![==1, *=1]);
        Tabs::new(Category::VARIANTS.iter().map(ToString::to_string))
            .select(self.tab as usize)
            .style(theme.text_muted)
            .highlight_style(theme.highlight)
            .render(tabs_area, buf);

        let column_width = key_width() + COLUMN_GAP;
        let scroll = self.scroll.min(self.max_scroll(inner));
        let lines: Vec<Line> = bindings_in(self.tab)
            .skip(scroll)
            .map(|binding| {
                Line::from(vec![
                    Span::styled(format!("{:column_width$}", binding.key()), theme.accent),
                    Span::styled(binding.description, theme.text),
                ])
            })
            .collect();
        Paragraph::new(lines).render(list_area, buf);
    }
}

fn key_width() -> usize {
    KEY_BINDINGS
        .iter()
        .map(|binding| usize::from(binding.key().as_str().cell_width()))
        .max()
        .unwrap_or_default()
}

fn bindings_in(tab: Category) -> impl Iterator<Item = &'static Binding> {
    KEY_BINDINGS
        .iter()
        .filter(move |binding| binding.category == tab)
}
