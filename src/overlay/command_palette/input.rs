use crossterm::event::{KeyCode, KeyEvent};

use crate::app::Action;
use crate::core::VisibleSequence;
use crate::core::parser::SequenceType;
use crate::core::search::{FilterMode, SearchableList};
use crate::overlay::{Notification, NotificationLevel};
use crate::ui::UiAction;

use super::command_definitions::COMMAND_SPECS;
use super::command_error::CommandError;
use super::command_spec::{PaletteCommand, TypableCommand};
use super::utils::parse_argument;

#[derive(Debug, Clone, Copy)]
pub(super) enum PaletteState {
    Command,
    Argument { command: TypableCommand },
}

#[derive(Debug)]
pub struct CommandPaletteState {
    pub(super) command_input: String,
    pub(super) argument_input: String,
    pub(super) phase: PaletteState,
    pub(super) command_list: SearchableList,
    pub(super) completion_list: SearchableList,
    pub(super) selectable_sequences: Vec<VisibleSequence>,
    pub(super) pinned_sequences: Vec<VisibleSequence>,
    pub(super) sequence_type: SequenceType,
}

impl CommandPaletteState {
    #[must_use]
    pub fn new(
        selectable_sequences: Vec<VisibleSequence>,
        pinned_sequences: Vec<VisibleSequence>,
        sequence_type: SequenceType,
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
            selectable_sequences,
            pinned_sequences,
            sequence_type,
        }
    }

    fn current_typable_command(&self) -> Option<TypableCommand> {
        let PaletteState::Argument { command } = self.phase else {
            return None;
        };
        Some(command)
    }

    fn parse_command_input(&self) -> Option<(String, Option<String>)> {
        let input = self.command_input.trim();
        if input.is_empty() {
            return None;
        }

        if let Some((command_name, rest)) = input.split_once(char::is_whitespace) {
            let arguments = rest.trim();
            let arguments = (!arguments.is_empty()).then_some(arguments.to_string());
            Some((command_name.to_string(), arguments))
        } else {
            Some((input.to_string(), None))
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

    fn close_palette_with(&mut self, action: Action) -> Vec<Action> {
        self.reset_palette();
        vec![action, Action::Ui(UiAction::CloseCommandPalette)]
    }

    fn command_error(&mut self, error: &CommandError) -> Vec<Action> {
        self.reset_palette();
        vec![
            Action::Ui(UiAction::CloseCommandPalette),
            Action::Ui(UiAction::ShowNotification(Notification {
                level: NotificationLevel::Error,
                message: error.message.clone(),
            })),
        ]
    }

    fn enter_argument_mode(&mut self, command: TypableCommand) {
        self.phase = PaletteState::Argument { command };
        self.argument_input.clear();
        self.update_active_list();
    }

    fn cycle_selection(&mut self, forwards: bool) {
        match self.phase {
            PaletteState::Command => {
                self.command_list.move_selection_wrapped(forwards);
                self.sync_command_from_selection();
            }
            PaletteState::Argument { .. } => {
                self.completion_list.move_selection_wrapped(forwards);
                self.sync_argument_from_selection();
            }
        }
    }

    fn handle_global_key(&mut self, code: KeyCode) -> Option<Vec<Action>> {
        match code {
            KeyCode::Esc => Some(vec![Action::Ui(UiAction::CloseCommandPalette)]),
            KeyCode::Tab => {
                self.cycle_selection(true);
                Some(Vec::new())
            }
            KeyCode::BackTab => {
                self.cycle_selection(false);
                Some(Vec::new())
            }
            _ => None,
        }
    }

    fn submit_argument_command(&mut self) -> Vec<Action> {
        let Some(spec) = self.current_typable_command() else {
            return Vec::new();
        };
        let arguments = self.argument_input.trim();
        match (spec.run)(self, arguments) {
            Ok(action) => self.close_palette_with(action),
            Err(error) => self.command_error(&error),
        }
    }

    fn submit_command_selection(&mut self) -> Vec<Action> {
        let Some((command_name, arguments)) = self.parse_command_input() else {
            return self.command_error(&CommandError::new("No command selected"));
        };
        let Some(spec) = resolve_command(command_name.as_str()) else {
            return self.command_error(&CommandError::new("Unknown command"));
        };
        let command_selected = self.command_list.selected_display_index().is_some();

        self.set_command_input(spec.name());

        if let Some(arguments) = arguments {
            if spec.typable().is_none() {
                return self.command_error(&CommandError::new("Expected 0 arguments, got 1"));
            }
            return match spec.run(self, arguments.as_str()) {
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

    fn handle_command_input(&mut self, key: KeyEvent) -> Vec<Action> {
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

    fn handle_argument_input(&mut self, key: KeyEvent) -> Vec<Action> {
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

    pub fn handle_key_event(&mut self, key: KeyEvent) -> Vec<Action> {
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
