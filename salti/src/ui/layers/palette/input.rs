use anyhow::format_err;
use crossterm::event::{KeyCode, KeyEvent};

use super::{
    command_definitions::COMMAND_SPECS,
    command_spec::{PaletteCommand, TypableCommand},
    utils::parse_argument,
};
use crate::{
    command::Command,
    core::{
        gff::Gff,
        search::{Direction, FilterMode, SearchableList},
        session::Session,
    },
    ui::layers::notification::{Notification, NotificationLevel},
};

#[derive(Debug, Clone, Copy)]
pub(super) enum PaletteState {
    Command,
    Argument { command: TypableCommand },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct VisibleSequence {
    pub(super) sequence_id: usize,
    pub(super) sequence_name: String,
}

#[derive(Debug)]
pub struct CommandPaletteState {
    pub(super) command_input: String,
    pub(super) argument_input: String,
    pub(super) phase: PaletteState,
    pub(super) command_list: SearchableList,
    pub(super) completion_list: SearchableList,
    pub(super) rows: Vec<VisibleSequence>,
    pub(super) pinned: usize,
    pub(super) feature_names: Option<Vec<String>>,
}

impl CommandPaletteState {
    pub fn empty() -> Self {
        Self::new(Vec::new(), 0, None)
    }

    pub fn from_session(session: &Session, gff: Option<&Gff>) -> Self {
        let layout = session.layout();
        let rows = layout
            .rows()
            .iter()
            .map(|&row| VisibleSequence {
                sequence_id: row,
                sequence_name: session.base_alignment().id(row).to_string(),
            })
            .collect();
        let feature_names = gff.map(|gff| {
            gff.features
                .iter()
                .map(|feature| feature.name.clone())
                .collect()
        });

        Self::new(rows, layout.pinned(), feature_names)
    }

    pub(super) fn new(
        rows: Vec<VisibleSequence>,
        pinned: usize,
        feature_names: Option<Vec<String>>,
    ) -> Self {
        let mut command_list = SearchableList::new(FilterMode::Fuzzy, None);
        command_list.set_items(display_command_names());
        let completion_list = SearchableList::new(FilterMode::Fuzzy, None);

        Self {
            command_input: String::new(),
            argument_input: String::new(),
            phase: PaletteState::Command,
            command_list,
            completion_list,
            rows,
            pinned,
            feature_names,
        }
    }

    fn current_typable_command(&self) -> Option<TypableCommand> {
        let PaletteState::Argument { command } = self.phase else {
            return None;
        };
        Some(command)
    }

    fn parse_command_input(input: &str) -> Option<(&str, Option<&str>)> {
        let input = input.trim();
        if input.is_empty() {
            return None;
        }

        if let Some((command_name, rest)) = input.split_once(char::is_whitespace) {
            let arguments = rest.trim();
            Some((command_name, (!arguments.is_empty()).then_some(arguments)))
        } else {
            Some((input, None))
        }
    }

    fn set_command_input(&mut self, name: &'static str) {
        self.command_input.clear();
        self.command_input.push_str(name);
    }

    fn sync_command_from_selection(&mut self) {
        if let Some(selection) = self.command_list.selected_label() {
            self.command_input.clear();
            self.command_input.push_str(selection);
        }
    }

    fn sync_argument_from_selection(&mut self) {
        if let Some(label) = self.completion_list.selected_label() {
            self.argument_input.clear();
            self.argument_input.push_str(label);
        }
    }

    fn update_command_filter(&mut self) {
        self.command_list.update_query(self.command_input.as_str());
        self.command_list.reset_selection();
    }

    fn update_argument_completion(&mut self) {
        let Some(spec) = self.current_typable_command() else {
            return;
        };

        let arguments = self.argument_input.as_str();
        let query = parse_argument(arguments);
        self.completion_list.set_items_and_query(
            spec.candidates(self, arguments),
            query.as_deref().unwrap_or_default(),
        );
        self.completion_list.reset_selection();
    }

    fn update_active_list(&mut self) {
        match self.phase {
            PaletteState::Command => self.update_command_filter(),
            PaletteState::Argument { .. } => self.update_argument_completion(),
        }
    }

    fn reset_palette(&mut self) {
        self.phase = PaletteState::Command;
        self.command_input.clear();
        self.argument_input.clear();
        self.completion_list.set_items(Vec::new());
        self.update_command_filter();
    }

    fn close_palette_with(&mut self, command: Command) -> Vec<Command> {
        self.reset_palette();
        vec![command, Command::CloseOverlay]
    }

    fn command_error(&mut self, error: &anyhow::Error) -> Vec<Command> {
        self.reset_palette();
        vec![
            Command::CloseOverlay,
            Command::ShowNotification(Notification {
                level: NotificationLevel::Error,
                message: error.to_string(),
            }),
        ]
    }

    fn enter_argument_mode(&mut self, command: TypableCommand) {
        self.phase = PaletteState::Argument { command };
        self.argument_input.clear();
        self.update_active_list();
    }

    fn cycle_selection(&mut self, direction: Direction) {
        match self.phase {
            PaletteState::Command => {
                self.command_list.move_selection_wrapped(direction);
                self.sync_command_from_selection();
            }
            PaletteState::Argument { .. } => {
                self.completion_list.move_selection_wrapped(direction);
                self.sync_argument_from_selection();
            }
        }
    }

    fn handle_global_key(&mut self, code: KeyCode) -> Option<Vec<Command>> {
        match code {
            KeyCode::Esc => Some(vec![Command::CloseOverlay]),
            KeyCode::Tab => {
                self.cycle_selection(Direction::Forward);
                Some(Vec::new())
            }
            KeyCode::BackTab => {
                self.cycle_selection(Direction::Backward);
                Some(Vec::new())
            }
            _ => None,
        }
    }

    fn submit_argument_command(&mut self) -> Vec<Command> {
        let Some(spec) = self.current_typable_command() else {
            return Vec::new();
        };
        let arguments = self.argument_input.trim();
        match (spec.run)(self, arguments) {
            Ok(action) => self.close_palette_with(action),
            Err(error) => self.command_error(&error),
        }
    }

    fn submit_command_selection(&mut self) -> Vec<Command> {
        let input = std::mem::take(&mut self.command_input);
        let Some((command_name, arguments)) = Self::parse_command_input(input.as_str()) else {
            return self.command_error(&format_err!("No command selected"));
        };
        let Some(spec) = resolve_command(command_name) else {
            return self.command_error(&format_err!("Unknown command"));
        };
        let command_selected = self.command_list.selected_display_index().is_some();

        self.set_command_input(spec.name());

        if let Some(arguments) = arguments {
            if spec.typable().is_none() {
                return self.command_error(&format_err!("Expected 0 arguments, got 1"));
            }
            return match spec.run(self, arguments) {
                Ok(action) => self.close_palette_with(action),
                Err(error) => self.command_error(&error),
            };
        }

        if command_selected && let Some(command) = spec.typable() {
            self.enter_argument_mode(command);
            return Vec::new();
        }

        match spec.run(self, "") {
            Ok(action) => self.close_palette_with(action),
            Err(error) => self.command_error(&error),
        }
    }

    fn handle_command_input(&mut self, key: KeyEvent) -> Vec<Command> {
        match key.code {
            KeyCode::Enter => self.submit_command_selection(),
            KeyCode::Backspace => {
                self.command_input.pop();
                self.update_command_filter();
                Vec::new()
            }
            KeyCode::Char(':') => {
                self.command_input.clear();
                self.update_command_filter();
                Vec::new()
            }
            KeyCode::Char(' ') => {
                let trimmed_start = self.command_input.trim_start();
                if trimmed_start.is_empty() {
                    self.command_input.push(' ');
                    self.update_command_filter();
                    return Vec::new();
                }

                let (command_name, has_arguments) =
                    if let Some((name, _)) = trimmed_start.split_once(char::is_whitespace) {
                        (name, true)
                    } else {
                        (trimmed_start, false)
                    };
                let Some(spec) = resolve_command(command_name) else {
                    self.command_input.push(' ');
                    self.update_command_filter();
                    return Vec::new();
                };

                if !has_arguments {
                    self.set_command_input(spec.name());
                }
                if let Some(command) = spec.typable() {
                    self.enter_argument_mode(command);
                    return Vec::new();
                }

                self.command_input.push(' ');
                self.update_command_filter();
                Vec::new()
            }
            KeyCode::Char(c) => {
                if self.command_list.selected_display_index().is_some() {
                    self.command_input.clear();
                }
                self.command_input.push(c);
                self.update_command_filter();
                Vec::new()
            }
            _ => Vec::new(),
        }
    }

    fn handle_argument_input(&mut self, key: KeyEvent) -> Vec<Command> {
        match key.code {
            KeyCode::Enter => self.submit_argument_command(),
            KeyCode::Backspace => {
                if self.argument_input.is_empty() {
                    self.phase = PaletteState::Command;
                    self.update_active_list();
                    return Vec::new();
                }

                self.argument_input.pop();
                self.update_active_list();
                Vec::new()
            }
            KeyCode::Char(c) => {
                self.argument_input.push(c);
                self.update_active_list();
                Vec::new()
            }
            _ => Vec::new(),
        }
    }

    pub(super) fn command_exact_match(&self) -> Option<PaletteCommand> {
        if !matches!(self.phase, PaletteState::Command) {
            return None;
        }
        resolve_command(self.command_input.as_str())
    }

    pub(super) fn current_command_context(&self) -> Option<PaletteCommand> {
        self.current_typable_command().map(PaletteCommand::Typable)
    }

    pub(super) fn has_active_completions(&self) -> bool {
        self.completion_list.has_visible_items()
    }

    pub fn handle_key_event(&mut self, key: KeyEvent) -> Vec<Command> {
        if let Some(actions) = self.handle_global_key(key.code) {
            return actions;
        }

        match self.phase {
            PaletteState::Command => self.handle_command_input(key),
            PaletteState::Argument { .. } => self.handle_argument_input(key),
        }
    }
}

fn display_command_names() -> Vec<String> {
    COMMAND_SPECS
        .iter()
        .map(|spec| spec.name().to_string())
        .collect()
}

fn resolve_command(name: &str) -> Option<PaletteCommand> {
    COMMAND_SPECS
        .iter()
        .copied()
        .find(|spec| spec.name() == name || spec.aliases().contains(&name))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::from(code)
    }

    #[test]
    fn submit_returns_expected_command() {
        let mut palette = CommandPaletteState::new(CommandPaletteSnapshot {
            selectable_sequences: Vec::new(),
            pinned_sequences: Vec::new(),
            active_type: libmsa::AlignmentType::Dna,
            is_reloaded_as_protein: false,
            visible_columns: vec![0, 3, 4],
            gff_feature_targets: None,
        });
        palette.command_input = "jump-position 2".to_string();

        let commands = palette.handle_key_event(key(KeyCode::Enter));

        assert_eq!(
            commands,
            vec![Command::JumpToPosition(1), Command::CloseOverlay]
        );
    }

    #[test]
    fn submit_jump_feature_alias_returns_the_expected_command() {
        let mut palette = CommandPaletteState::new(CommandPaletteSnapshot {
            selectable_sequences: Vec::new(),
            pinned_sequences: Vec::new(),
            active_type: libmsa::AlignmentType::Dna,
            is_reloaded_as_protein: false,
            visible_columns: vec![0, 1, 2],
            gff_feature_targets: Some(vec![GffFeatureTarget {
                feature_name: "gene-a".into(),
                target_col: Some(2),
            }]),
        });
        palette.command_input = "jf gene-a".to_string();

        let commands = palette.handle_key_event(key(KeyCode::Enter));

        assert_eq!(
            commands,
            vec![Command::JumpToPosition(2), Command::CloseOverlay]
        );
    }

    #[test]
    fn submit_success_appends_close_command_palette() {
        let mut palette = CommandPaletteState::empty();
        palette.command_input = "quit".to_string();

        let commands = palette.handle_key_event(key(KeyCode::Enter));

        assert_eq!(commands, vec![Command::Quit, Command::CloseOverlay]);
    }

    #[test]
    fn invalid_input_closes_palette_and_shows_notification() {
        let mut palette = CommandPaletteState::empty();
        palette.command_input = "wat".to_string();

        let commands = palette.handle_key_event(key(KeyCode::Enter));

        assert_eq!(
            commands,
            vec![
                Command::CloseOverlay,
                Command::ShowNotification(Notification {
                    level: NotificationLevel::Error,
                    message: "Unknown command".to_string(),
                }),
            ]
        );
    }
}
