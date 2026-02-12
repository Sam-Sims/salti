use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Styled;
use ratatui::text::Line;
use ratatui::widgets::{Block, Clear, Paragraph, Widget};

use crate::core::search::SearchableList;

use super::command_spec::PaletteCommand;
use super::input::{CommandPaletteState, PaletteState};
use super::utils::{pad_label, wrap_text};

/// maximum number of rows shown in the command/preview grid at once.
const COMMAND_GRID_MAX_VISIBLE_ROWS: usize = 6;
/// maximum number of columns shown in the command/preview grid.
const COMMAND_GRID_MAX_VISIBLE_COLS: usize = 4;
/// number of spaces inserted between grid columns in preview/command lists.
const COMMAND_GRID_COLUMN_GAP_SPACES: usize = 2;
/// spacing text rendered between grid columns.
const COMMAND_GRID_COLUMN_GAP_TEXT: &str = "  ";

impl CommandPaletteState {
    fn help_overlay_lines(
        spec: PaletteCommand,
        width: usize,
        theme: &crate::config::theme::ThemeStyles,
    ) -> Vec<Line<'static>> {
        let help_lines = wrap_text(spec.help_text(), width)
            .into_iter()
            .map(|line| Line::from(line.set_style(theme.text)));

        let alias_lines = (!spec.aliases().is_empty())
            .then(|| format!("Aliases: {}", spec.aliases().join(", ")))
            .into_iter()
            .flat_map(|line| wrap_text(line.as_str(), width).into_iter())
            .map(|line| Line::from(line.set_style(theme.text_muted)));

        help_lines.chain(alias_lines).collect()
    }

    fn render_command_help_overlay(
        f: &mut Frame,
        area: Rect,
        theme: &crate::config::theme::ThemeStyles,
        spec: PaletteCommand,
    ) {
        let lines = Self::help_overlay_lines(spec, area.width.saturating_sub(2) as usize, theme);
        let height = (lines.len().saturating_add(2) as u16).min(area.height);
        if height < 2 {
            return;
        }

        let help_area = Rect::new(area.x, area.y, area.width, height);
        let block = Block::bordered()
            .border_style(theme.border)
            .style(theme.panel_block_dim);
        let inner_area = block.inner(help_area);

        f.render_widget(block, help_area);
        f.render_widget(
            Paragraph::new(lines).style(theme.panel_block_dim),
            inner_area,
        );
    }

    fn help_overlay_required_rows(
        spec: PaletteCommand,
        width: usize,
        theme: &crate::config::theme::ThemeStyles,
    ) -> usize {
        Self::help_overlay_lines(
            spec,
            width.saturating_div(3).max(3).saturating_sub(2),
            theme,
        )
        .len()
        .saturating_add(2)
    }

    fn help_overlay_spec(&self) -> Option<PaletteCommand> {
        self.current_command_context()
            .or_else(|| self.command_exact_match())
    }

    fn preview_list_grid_lines<'a>(
        list: &'a SearchableList,
        selected_display_index: Option<usize>,
        width: usize,
        max_rows: usize,
        theme: &crate::config::theme::ThemeStyles,
    ) -> Vec<Line<'a>> {
        let items_len = list.visible_len();
        if items_len == 0 {
            return vec![Line::from("No matches".set_style(theme.text_dim))];
        }

        let cols = width.clamp(1, COMMAND_GRID_MAX_VISIBLE_COLS);
        let rows_per_page = items_len
            .div_ceil(cols)
            .min(COMMAND_GRID_MAX_VISIBLE_ROWS)
            .min(max_rows);

        let selection = selected_display_index.unwrap_or_default();
        let page_size = rows_per_page.saturating_mul(cols);
        let page_start = (selection / page_size).saturating_mul(page_size);
        let page_end = page_start.saturating_add(page_size);
        let visible_end = items_len.min(page_end);
        let selection_in_page = selection.saturating_sub(page_start);

        let col_width = width
            .saturating_sub(COMMAND_GRID_COLUMN_GAP_SPACES.saturating_mul(cols.saturating_sub(1)))
            .saturating_div(cols)
            .max(1);
        let mut lines = Vec::with_capacity(rows_per_page);
        for row in 0..rows_per_page {
            let mut spans = Vec::with_capacity(cols.saturating_mul(3));
            for col in 0..cols {
                let in_page_index = col * rows_per_page + row;
                let index = page_start.saturating_add(in_page_index);
                if index >= visible_end {
                    continue;
                }
                let Some(label) = list.visible_item_at(index) else {
                    continue;
                };
                let (text, padding) = pad_label(label, col_width);
                let is_selected =
                    selected_display_index.is_some() && selection_in_page == in_page_index;
                let style = if is_selected {
                    theme.selection
                } else {
                    theme.text
                };
                let padding_style = if is_selected { theme.text_muted } else { style };
                spans.push(text.set_style(style));
                spans.push(padding.set_style(padding_style));
                if col + 1 < cols {
                    spans.push(COMMAND_GRID_COLUMN_GAP_TEXT.set_style(theme.text_muted));
                }
            }
            lines.push(Line::from(spans));
        }
        lines
    }

    fn preview_lines(
        &self,
        spec: PaletteCommand,
        width: usize,
        max_rows: usize,
        theme: &crate::config::theme::ThemeStyles,
    ) -> Vec<Line<'_>> {
        if max_rows == 0 || spec.typable().is_none() || !self.has_active_completions() {
            return Vec::new();
        }

        Self::preview_list_grid_lines(
            &self.completion_list,
            self.completion_list.selected_display_index(),
            width,
            max_rows,
            theme,
        )
    }

    fn preview_required_rows(&self, spec: PaletteCommand, width: usize) -> usize {
        if spec.typable().is_none() || !self.has_active_completions() {
            return 0;
        }

        self.completion_list
            .visible_len()
            .max(1)
            .div_ceil(width.clamp(1, COMMAND_GRID_MAX_VISIBLE_COLS))
            .min(COMMAND_GRID_MAX_VISIBLE_ROWS)
    }

    fn render_help_and_preview(
        &self,
        f: &mut Frame,
        area: Rect,
        theme: &crate::config::theme::ThemeStyles,
        spec: PaletteCommand,
    ) {
        let lines = self.preview_lines(spec, area.width as usize, area.height as usize, theme);
        f.render_widget(Paragraph::new(lines).style(theme.panel_block), area);
    }

    fn render_command_grid(
        &self,
        f: &mut Frame,
        area: Rect,
        theme: &crate::config::theme::ThemeStyles,
    ) {
        let items_len = self.command_list.visible_len();
        let cols = (area.width as usize).clamp(1, COMMAND_GRID_MAX_VISIBLE_COLS);
        let rows = items_len
            .div_ceil(cols)
            .clamp(1, COMMAND_GRID_MAX_VISIBLE_ROWS)
            .min(area.height as usize);
        if rows == 0 {
            return;
        }

        let col_width = (area.width as usize).saturating_div(cols).max(1);
        let visible_end = items_len.min(rows.saturating_mul(cols));
        let selected_index = self.command_list.selected_display_index();

        let mut lines = Vec::with_capacity(rows);
        for row in 0..rows {
            let mut spans = Vec::with_capacity(cols.saturating_mul(2));
            for col in 0..cols {
                let index = col * rows + row;
                if index >= visible_end {
                    continue;
                }
                let Some(label) = self.command_list.visible_item_at(index) else {
                    continue;
                };
                let (text, padding) = pad_label(label, col_width);
                let is_selected = selected_index == Some(index);
                let style = if is_selected {
                    theme.selection
                } else {
                    theme.text
                };
                let padding_style = if is_selected { theme.text_muted } else { style };
                spans.push(text.set_style(style));
                spans.push(padding.set_style(padding_style));
            }
            lines.push(Line::from(spans));
        }

        f.render_widget(Paragraph::new(lines).style(theme.panel_block), area);
    }

    fn render_input(&self, f: &mut Frame, area: Rect, theme: &crate::config::theme::ThemeStyles) {
        let input = match self.phase {
            PaletteState::Command => format!(":{}", self.command_input),
            PaletteState::Argument { .. } => {
                format!(":{} {}", self.command_input, self.argument_input)
            }
        };

        let line = Line::from(format!("{input}█").set_style(theme.warning));

        f.render_widget(Paragraph::new(line).style(theme.base_block), area);
    }

    fn render_palette_region(
        &self,
        f: &mut Frame,
        area: Rect,
        theme: &crate::config::theme::ThemeStyles,
    ) {
        f.render_widget(Block::new().style(theme.panel_block), area);

        let area_width = area.width as usize;
        let area_height = area.height as usize;
        let help_spec = self.help_overlay_spec();
        let help_height = help_spec.map_or(0, |spec| {
            Self::help_overlay_required_rows(spec, area_width, theme).min(area_height) as u16
        });

        let (help_area, content_area) = if help_height == 0 {
            (None, area)
        } else {
            let help_width = area.width.saturating_div(3).max(3).min(area.width);
            let help_area = Rect::new(area.x, area.y, help_width, help_height);
            let gap_width = area.width.saturating_sub(help_width);
            if gap_width > 0 {
                let gap_area = Rect::new(
                    area.x.saturating_add(help_width),
                    area.y,
                    gap_width,
                    help_height,
                );
                f.render_widget(Block::new().style(theme.base_block), gap_area);
            }
            let content_area = Rect::new(
                area.x,
                area.y.saturating_add(help_height),
                area.width,
                area.height.saturating_sub(help_height),
            );
            (Some(help_area), content_area)
        };

        if let Some(spec) = self.current_command_context() {
            self.render_help_and_preview(f, content_area, theme, spec);
        } else {
            self.render_command_grid(f, content_area, theme);
        }

        if let (Some(help_area), Some(spec)) = (help_area, help_spec) {
            Self::render_command_help_overlay(f, help_area, theme, spec);
        }
    }

    fn palette_height(&self, content_area: Rect, theme: &crate::config::theme::ThemeStyles) -> u16 {
        let width = content_area.width as usize;
        let max_height = content_area.height as usize;

        let rows = if let Some(spec) = self.current_command_context() {
            self.preview_required_rows(spec, width)
        } else {
            self.command_list
                .visible_len()
                .div_ceil(width.clamp(1, COMMAND_GRID_MAX_VISIBLE_COLS))
                .clamp(1, COMMAND_GRID_MAX_VISIBLE_ROWS)
        };

        let help_rows = self.help_overlay_spec().map_or(0, |spec| {
            Self::help_overlay_required_rows(spec, width, theme)
        });

        rows.saturating_add(help_rows).min(max_height) as u16
    }

    pub fn render(
        &self,
        f: &mut Frame,
        content_area: Rect,
        input_area: Rect,
        theme: &crate::config::theme::ThemeStyles,
    ) {
        let palette_height = self.palette_height(content_area, theme);
        let palette_y = content_area
            .y
            .saturating_add(content_area.height.saturating_sub(palette_height));
        let palette_area = Rect::new(
            content_area.x,
            palette_y,
            content_area.width,
            palette_height,
        );

        Clear.render(palette_area, f.buffer_mut());

        self.render_palette_region(f, palette_area, theme);
        self.render_input(f, input_area, theme);
    }
}
