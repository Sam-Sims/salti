use fuzzy_matcher::{FuzzyMatcher, skim::SkimMatcherV2};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{
        Block, Borders, Clear, List, ListItem, ListState, Paragraph, StatefulWidget, Widget,
    },
};

use crate::state::State;

const JUMP_WIDTH: u16 = 70;
const JUMP_HEIGHT: u16 = 25;

#[derive(Clone, Debug)]
pub struct JumpComponent {
    pos_input: String,
    seq_input: String,
    current_mode: InputMode,
    filtered_sequences: Vec<(usize, String)>,
    selected_sequence: usize,
}

#[derive(Clone, Debug, PartialEq)]
enum InputMode {
    Position,
    Sequence,
}

impl JumpComponent {
    pub fn new() -> Self {
        Self {
            pos_input: String::new(),
            seq_input: String::new(),
            current_mode: InputMode::Position,
            filtered_sequences: Vec::new(),
            selected_sequence: 0,
        }
    }

    fn centered_rect(area: Rect) -> Rect {
        let popup_width = JUMP_WIDTH.min(area.width.saturating_sub(4));
        let popup_height = JUMP_HEIGHT.min(area.height.saturating_sub(4));

        let x = area.x + (area.width - popup_width) / 2;
        let y = area.y + (area.height - popup_height) / 2;

        Rect {
            x,
            y,
            width: popup_width,
            height: popup_height,
        }
    }

    fn has_sequence_selection(&self) -> bool {
        !self.filtered_sequences.is_empty()
            && self.selected_sequence < self.filtered_sequences.len()
            && (self.current_mode == InputMode::Sequence || !self.seq_input.is_empty())
    }

    fn is_valid_position_char(c: char) -> bool {
        c.is_ascii_digit()
    }

    fn update_sequences(&mut self, sequence_names: &[String]) {
        if self.seq_input.is_empty() {
            self.filtered_sequences = sequence_names
                .iter()
                .enumerate()
                .map(|(i, name)| (i, name.clone()))
                .collect();
        } else {
            let matcher = SkimMatcherV2::default();
            let query = &self.seq_input;

            let mut scored_sequences: Vec<(i64, usize, String)> = sequence_names
                .iter()
                .enumerate()
                .filter_map(|(i, name)| {
                    matcher
                        .fuzzy_match(name, query)
                        .map(|score| (score, i, name.clone()))
                })
                .collect();

            scored_sequences.sort_by(|a, b| b.0.cmp(&a.0));

            self.filtered_sequences = scored_sequences
                .into_iter()
                .map(|(_, i, name)| (i, name))
                .collect();
        }
    }

    pub fn get_position(&self) -> Option<usize> {
        self.pos_input.parse().ok()
    }

    pub fn get_selected_sequence_name(&self) -> Option<&str> {
        if self.filtered_sequences.is_empty() {
            return None;
        }
        self.filtered_sequences
            .get(self.selected_sequence)
            .map(|(_, name)| name.as_str())
    }

    pub fn get_selected_sequence_index(&self) -> Option<usize> {
        if self.has_sequence_selection() {
            self.filtered_sequences
                .get(self.selected_sequence)
                .map(|(index, _)| *index)
        } else {
            None
        }
    }

    pub fn toggle_mode(&mut self) {
        self.current_mode = match self.current_mode {
            InputMode::Position => InputMode::Sequence,
            InputMode::Sequence => InputMode::Position,
        };
    }

    pub fn move_selection_up(&mut self) {
        if !self.filtered_sequences.is_empty() {
            self.selected_sequence = self.selected_sequence.saturating_sub(1);
        }
    }

    pub fn move_selection_down(&mut self) {
        if !self.filtered_sequences.is_empty() {
            self.selected_sequence =
                (self.selected_sequence + 1).min(self.filtered_sequences.len().saturating_sub(1));
        }
    }

    pub fn add_char(&mut self, c: char, sequence_names: &[String]) {
        match self.current_mode {
            InputMode::Position => {
                if Self::is_valid_position_char(c) {
                    self.pos_input.push(c);
                }
            }
            InputMode::Sequence => {
                self.seq_input.push(c);
                self.update_sequences(sequence_names);
                self.selected_sequence = 0;
            }
        }
    }

    pub fn backspace(&mut self, sequence_names: &[String]) {
        match self.current_mode {
            InputMode::Position => {
                self.pos_input.pop();
            }
            InputMode::Sequence => {
                self.seq_input.pop();
                self.update_sequences(sequence_names);
                self.selected_sequence = 0;
            }
        }
    }

    pub fn clear(&mut self, sequence_names: &[String]) {
        self.pos_input.clear();
        self.seq_input.clear();
        self.current_mode = InputMode::Position;
        self.selected_sequence = 0;
        self.update_sequences(sequence_names);
    }

    pub fn render(&mut self, f: &mut Frame, full_area: Rect, data_store: &State) {
        let alignments = &data_store.alignments;
        let sequence_names: Vec<String> = alignments.iter().map(|a| a.id.to_string()).collect();

        self.update_sequences(&sequence_names);

        let popup_area = JumpComponent::centered_rect(full_area);

        Clear.render(popup_area, f.buffer_mut());

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan))
            .title(Line::from(vec![Span::styled(
                " Jump to Position / Sequence ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )]))
            .style(Style::default().bg(Color::Black));

        let inner_area = block.inner(popup_area);

        let chunks = Layout::vertical([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(3),
            Constraint::Length(2),
        ])
        .split(inner_area);

        let pos_content = vec![Line::from(vec![
            Span::raw("Position: "),
            Span::styled(
                &self.pos_input,
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            if self.current_mode == InputMode::Position {
                Span::styled("█", Style::default().fg(Color::Yellow))
            } else {
                Span::raw(" ")
            },
        ])];

        let seq_content = vec![Line::from(vec![
            Span::raw("Sequence: "),
            Span::styled(
                &self.seq_input,
                Style::default()
                    .fg(if self.current_mode == InputMode::Sequence {
                        Color::Yellow
                    } else {
                        Color::Gray
                    })
                    .add_modifier(if self.current_mode == InputMode::Sequence {
                        Modifier::BOLD
                    } else {
                        Modifier::empty()
                    }),
            ),
            if self.current_mode == InputMode::Sequence {
                Span::styled("█", Style::default().fg(Color::Yellow))
            } else {
                Span::raw(" ")
            },
        ])];

        let list_items: Vec<ListItem> = self
            .filtered_sequences
            .iter()
            .enumerate()
            .map(|(i, (_, name))| {
                let style = if i == self.selected_sequence && self.has_sequence_selection() {
                    Style::default().bg(Color::Blue).fg(Color::White)
                } else {
                    Style::default().fg(Color::Gray)
                };
                ListItem::new(name.as_str()).style(style)
            })
            .collect();

        let sequence_list = List::new(list_items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Matching Sequences")
                    .border_style(Style::default().fg(Color::Gray)),
            )
            .style(Style::default());

        let instructions = vec![Line::from(vec![
            Span::styled("Tab", Style::default().fg(Color::Cyan).bold()),
            Span::raw(": Switch mode | "),
            Span::styled("↑↓", Style::default().fg(Color::Cyan).bold()),
            Span::raw(": Navigate | "),
            Span::styled("Enter", Style::default().fg(Color::Cyan).bold()),
            Span::raw(": Jump | "),
            Span::styled("Esc", Style::default().fg(Color::Cyan).bold()),
            Span::raw(": Cancel"),
        ])];

        f.render_widget(block, popup_area);

        f.render_widget(
            Paragraph::new(pos_content)
                .alignment(Alignment::Left)
                .style(Style::default().bg(Color::Black)),
            chunks[0],
        );

        f.render_widget(
            Paragraph::new(seq_content)
                .alignment(Alignment::Left)
                .style(Style::default().bg(Color::Black)),
            chunks[1],
        );

        let mut list_state = ListState::default();
        list_state.select(Some(self.selected_sequence));
        StatefulWidget::render(sequence_list, chunks[2], f.buffer_mut(), &mut list_state);

        f.render_widget(
            Paragraph::new(instructions)
                .alignment(Alignment::Center)
                .style(Style::default().bg(Color::Black)),
            chunks[3],
        );
    }
}
