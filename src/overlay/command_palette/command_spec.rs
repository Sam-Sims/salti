use crate::app::Action;

use super::command_error::CommandError;
use super::input::CommandPaletteState;

pub(super) type CompleterFunc = fn(&CommandPaletteState, &str) -> Vec<String>;
pub(super) type RunnerFunc = fn(&CommandPaletteState, &str) -> Result<Action, CommandError>;

#[derive(Debug, Clone, Copy)]
pub(super) struct StaticCommand {
    pub(super) name: &'static str,
    pub(super) help_text: &'static str,
    pub(super) aliases: &'static [&'static str],
    pub(super) run: RunnerFunc,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct TypableCommand {
    pub(super) name: &'static str,
    pub(super) help_text: &'static str,
    pub(super) aliases: &'static [&'static str],
    pub(super) completer: Option<CompleterFunc>,
    pub(super) static_candidates: &'static [&'static str],
    pub(super) run: RunnerFunc,
}

impl TypableCommand {
    pub(super) fn candidates(self, state: &CommandPaletteState, arguments: &str) -> Vec<String> {
        if let Some(completer) = self.completer {
            completer(state, arguments)
        } else {
            self.static_candidates
                .iter()
                .map(std::string::ToString::to_string)
                .collect()
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) enum PaletteCommand {
    Static(StaticCommand),
    Typable(TypableCommand),
}

impl PaletteCommand {
    #[must_use]
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Static(spec) => spec.name,
            Self::Typable(spec) => spec.name,
        }
    }

    #[must_use]
    pub(super) fn help_text(self) -> &'static str {
        match self {
            Self::Static(spec) => spec.help_text,
            Self::Typable(spec) => spec.help_text,
        }
    }

    #[must_use]
    pub(super) fn aliases(self) -> &'static [&'static str] {
        match self {
            Self::Static(spec) => spec.aliases,
            Self::Typable(spec) => spec.aliases,
        }
    }

    pub(super) fn run(
        self,
        state: &CommandPaletteState,
        arguments: &str,
    ) -> Result<Action, CommandError> {
        match self {
            Self::Static(spec) => (spec.run)(state, arguments),
            Self::Typable(spec) => (spec.run)(state, arguments),
        }
    }

    #[must_use]
    pub(super) fn typable(self) -> Option<TypableCommand> {
        match self {
            Self::Static(_) => None,
            Self::Typable(spec) => Some(spec),
        }
    }
}
