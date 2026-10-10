mod commands;
mod completers;
mod runners;

use anyhow::{Context as _, Result, bail, format_err};
use ratatui::buffer::CellWidth;

use self::commands::{Action, COMMANDS};
use crate::{
    command::Command,
    core::session::Session,
    ui::{
        bars::{Notification, NotificationLevel},
        popups::Context,
        truncate_label,
    },
};

fn run_line(line: &str, ctx: &Context) -> Vec<Command> {
    if line.trim().is_empty() {
        return vec![Command::ClosePopup];
    }
    let command = parse_line(line, ctx).unwrap_or_else(|error| {
        Command::ShowNotification(Notification {
            level: NotificationLevel::Error,
            message: error.to_string(),
        })
    });
    vec![Command::ClosePopup, command]
}

fn parse_line(line: &str, ctx: &Context) -> Result<Command> {
    let line = line.trim();
    let (name, rest) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
    let (name, rest) = if name.bytes().all(|byte| byte.is_ascii_digit()) {
        ("jump-position", line)
    } else {
        (name, rest)
    };
    let command = COMMANDS
        .iter()
        .find(|command| command.name == name || command.aliases.contains(&name))
        .with_context(|| format!("no such command: '{name}'"))?;

    let arguments = parse_arguments(rest);
    let count = arguments.len();
    match (&command.action, arguments.as_slice()) {
        (Action::Fixed(fixed), []) => Ok(fixed.clone()),
        (Action::Optional { run, .. }, []) => run(ctx, None),
        (Action::Optional { run, .. }, [argument]) => run(ctx, Some(argument)),
        (Action::Required { run, .. }, [argument]) => run(ctx, argument),
        (Action::Fixed(_), _) => bail!("expected no arguments, got {count}"),
        (Action::Optional { .. }, _) => bail!("expected at most 1 argument, got {count}"),
        (Action::Required { .. }, _) => bail!("expected 1 argument, got {count}"),
    }
    .map_err(|error| format_err!("'{}': {error}", command.name))
}

fn parse_arguments(input: &str) -> Vec<String> {
    let mut arguments = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;

    for character in input.chars() {
        match quote {
            Some(open) if character == open => quote = None,
            None if character == '"' || character == '\'' => quote = Some(character),
            None if character.is_whitespace() => {
                if !current.is_empty() {
                    arguments.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(character),
        }
    }
    if !current.is_empty() {
        arguments.push(current);
    }
    arguments
}

fn row_named(session: &Session, rows: &[usize], name: &str) -> Result<usize> {
    rows.iter()
        .copied()
        .find(|&row| session.base_alignment().id(row) == name)
        .with_context(|| format!("no sequence named '{name}'"))
}

fn names(session: &Session, rows: &[usize]) -> Vec<String> {
    rows.iter()
        .map(|&row| session.base_alignment().id(row).to_string())
        .collect()
}

fn pad_label(label: &str, width: usize) -> (String, String) {
    let text = truncate_label(label, width);
    let padding = width.saturating_sub(usize::from(text.cell_width()));
    (text, " ".repeat(padding))
}

fn wrap_text(text: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return Vec::new();
    }

    let mut lines = Vec::new();
    for raw_line in text.split('\n') {
        let mut current = String::new();
        for word in raw_line.split_whitespace() {
            if current.is_empty() {
                current.push_str(word);
                continue;
            }
            if current.len() + 1 + word.len() > width {
                lines.push(current);
                current = word.to_string();
            } else {
                current.push(' ');
                current.push_str(word);
            }
        }
        if !current.is_empty() {
            lines.push(current);
        }
    }

    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;
    use crate::test_utils::{pinned_session, with_context};

    fn run(line: &str) -> Vec<Command> {
        let session = pinned_session(3, &[1]);
        with_context(Some(&session), None, |ctx| run_line(line, ctx))
    }

    #[rstest]
    #[case::fixed("quit", Command::Quit)]
    #[case::alias("q", Command::Quit)]
    #[case::required("jump-position 3", Command::JumpToColumn(2))]
    #[case::required_alias("jp 3", Command::JumpToColumn(2))]
    #[case::bare_number("42", Command::JumpToColumn(41))]
    #[case::optional_without_argument("filter-rows", Command::SetRowFilter(None))]
    #[case::optional_with_argument("filter-rows s0", Command::SetRowFilter(Some("s0".to_string())))]
    #[case::quoted_argument("filter-rows 's 0'", Command::SetRowFilter(Some("s 0".to_string())))]
    #[case::surrounding_space("  quit  ", Command::Quit)]
    #[case::reads_session("jump-sequence s2", Command::JumpToSequence(2))]
    fn run_line_works(#[case] line: &str, #[case] expected: Command) {
        assert_eq!(run(line), vec![Command::ClosePopup, expected]);
    }

    #[test]
    fn run_line_closes_on_empty_line() {
        assert_eq!(run("   "), vec![Command::ClosePopup]);
    }

    #[rstest]
    #[case::unknown_command("nope")]
    #[case::partial_name("jump-pos 3")]
    #[case::argument_to_fixed("quit now")]
    #[case::two_arguments_to_optional("filter-rows a b")]
    #[case::missing_required("jump-sequence")]
    #[case::two_arguments_to_required("jump-sequence s0 s2")]
    #[case::bare_number_with_argument("42 7")]
    #[case::runner_error("jump-position 0")]
    fn run_line_rejects(#[case] line: &str) {
        let commands = run(line);

        assert!(matches!(
            commands.as_slice(),
            [
                Command::ClosePopup,
                Command::ShowNotification(Notification {
                    level: NotificationLevel::Error,
                    ..
                })
            ]
        ));
    }

    #[rstest]
    #[case::empty("", &[])]
    #[case::words("a b", &["a", "b"])]
    #[case::extra_whitespace("  a   b ", &["a", "b"])]
    #[case::single_quoted("'a b' c", &["a b", "c"])]
    #[case::double_quoted_with_single_quote("\"it's\"", &["it's"])]
    #[case::quote_inside_word("a'b c'd", &["ab cd"])]
    #[case::unterminated_quote("'a b", &["a b"])]
    #[case::empty_quotes("''", &[])]
    fn parse_arguments_works(#[case] input: &str, #[case] expected: &[&str]) {
        assert_eq!(parse_arguments(input), expected);
    }
}
