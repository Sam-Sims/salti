mod commands;
mod completers;
mod render;
mod runners;

use anyhow::{Context as _, Result, bail, format_err};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    macros::{horizontal, vertical},
    text::Span,
    widgets::Widget,
};

use self::commands::{Action, COMMANDS, PaletteCommand};
use crate::{
    command::Command,
    ui::{
        notification::Notification,
        popups::{Context, PopupContent, PopupFrame, fuzzy::fuzzy_match, text_input::TextInput},
    },
};

const QUOTES: [char; 2] = ['\'', '"'];

#[derive(Debug)]
pub(crate) struct PaletteState {
    input: TextInput,
    completions: Completions,
    selected: Option<usize>,
}

impl PaletteState {
    fn doc_command(&self) -> Option<&'static PaletteCommand> {
        find_command(split_command(self.input.text()).0)
    }

    fn selected_is_directory(&self) -> bool {
        self.selected
            .is_some_and(|index| self.completions.items[index].ends_with('/'))
    }

    fn refresh(&mut self, ctx: &Context) {
        self.completions = completions(self.input.text(), ctx);
        self.selected = None;
    }

    fn cycle(&mut self, forward: bool) {
        let count = self.completions.items.len();
        if count == 0 {
            return;
        }
        let index = match (self.selected, forward) {
            (None, true) => 0,
            (None, false) => count - 1,
            (Some(index), true) => (index + 1) % count,
            (Some(index), false) => (index + count - 1) % count,
        };
        self.selected = Some(index);
        let Completions {
            word_start, quote, ..
        } = self.completions;
        let line = format!(
            "{}{}",
            &self.input.text()[..word_start],
            quoted(&self.completions.items[index], quote)
        );
        self.input.set_text(&line);
    }
}

impl Default for PaletteState {
    fn default() -> Self {
        Self {
            input: TextInput::default(),
            completions: Completions {
                word_start: 0,
                quote: None,
                items: command_names(""),
            },
            selected: None,
        }
    }
}

impl PopupContent for PaletteState {
    fn frame(&self) -> PopupFrame<'_> {
        PopupFrame::Bare
    }

    fn area(&self, ctx: &Context) -> Rect {
        let overlay_area = ctx.screen.frame.overlay_area;
        let input_area = ctx.screen.frame.input_area;
        let above = render::rows_above_prompt(
            &self.completions.items,
            self.doc_command(),
            overlay_area.width,
        )
        .min(overlay_area.height);
        Rect {
            y: input_area.y - above,
            height: input_area.height + above,
            ..input_area
        }
    }

    fn handle_key(&mut self, key: KeyEvent, _: Rect, ctx: &Context) -> Vec<Command> {
        match key.code {
            KeyCode::Tab => {
                self.cycle(true);
                if self.completions.items.len() == 1 && self.selected_is_directory() {
                    self.refresh(ctx);
                }
            }
            KeyCode::BackTab => self.cycle(false),
            KeyCode::Enter if self.selected_is_directory() => self.refresh(ctx),
            KeyCode::Enter => return run_line(self.input.text(), ctx),
            _ => {
                if self.input.handle_key(key) {
                    self.refresh(ctx);
                }
            }
        }
        Vec::new()
    }

    fn render(&self, inner: Rect, buf: &mut Buffer, ctx: &Context) {
        let theme = ctx.theme;
        let items = &self.completions.items;
        let [above, prompt_row] = inner.layout(&vertical![*=1, ==1]);
        let list_rows = render::list_rows(items, above.width).min(above.height);
        let [doc_area, list_area] = above.layout(&vertical![*=1, ==list_rows]);
        if let Some(command) = self.doc_command() {
            render::draw_doc(doc_area, buf, command, theme);
        }
        render::draw_list(list_area, buf, items, self.selected, theme);

        let [prompt, line] = prompt_row.layout(&horizontal![==1, *=1]);
        Span::styled(":", theme.warning).render(prompt, buf);
        self.input.render(line, buf, theme.warning);
    }
}

#[derive(Debug)]
struct Completions {
    /// Byte index in the line where the word the completions replace starts.
    word_start: usize,
    /// The quote the user opened the word with, if any.
    quote: Option<char>,
    items: Vec<String>,
}

fn run_line(line: &str, ctx: &Context) -> Vec<Command> {
    if line.trim().is_empty() {
        return vec![Command::ClosePopup];
    }
    let command = parse_line(line, ctx)
        .unwrap_or_else(|error| Command::ShowNotification(Notification::error(error.to_string())));
    vec![Command::ClosePopup, command]
}

fn parse_line(line: &str, ctx: &Context) -> Result<Command> {
    let (name, rest) = split_command(line);
    let arguments: Vec<String> = words(rest.unwrap_or_default())
        .into_iter()
        .map(|(_, text)| text)
        .filter(|text| !text.is_empty())
        .collect();
    let (name, arguments) = if name.parse::<usize>().is_ok() && arguments.is_empty() {
        ("jump-position", vec![name.to_string()])
    } else {
        (name, arguments)
    };
    let command = find_command(name).with_context(|| format!("no such command: '{name}'"))?;

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

fn completions(line: &str, ctx: &Context) -> Completions {
    let (name, Some(rest)) = split_command(line) else {
        return Completions {
            word_start: 0,
            quote: None,
            items: command_names(line),
        };
    };
    let (start, word) = words(rest)
        .pop()
        .expect("words ends with the word being typed");
    let quote = rest[start..]
        .chars()
        .next()
        .filter(|character| QUOTES.contains(character));
    let items = find_command(name)
        .and_then(|command| command.action.completer())
        .map_or_else(Vec::new, |complete| complete(ctx, &word));
    Completions {
        word_start: line.len() - rest.len() + start,
        quote,
        items,
    }
}

fn command_names(word: &str) -> Vec<String> {
    fuzzy_match(word, COMMANDS.iter().map(|command| command.name))
        .into_iter()
        .map(str::to_string)
        .collect()
}

fn find_command(name: &str) -> Option<&'static PaletteCommand> {
    COMMANDS
        .iter()
        .find(|command| command.name == name || command.aliases.contains(&name))
}

fn split_command(line: &str) -> (&str, Option<&str>) {
    line.split_once(char::is_whitespace)
        .map_or((line, None), |(name, rest)| (name, Some(rest)))
}

fn quoted(completion: &str, quote: Option<char>) -> String {
    if quote.is_none()
        && !completion.contains(char::is_whitespace)
        && !completion.starts_with(QUOTES)
    {
        return completion.to_string();
    }
    let quote = match quote {
        Some(quote) if !completion.contains(quote) => quote,
        _ if completion.contains('\'') => '"',
        _ => '\'',
    };
    format!("{quote}{completion}{quote}")
}

fn words(line: &str) -> Vec<(usize, String)> {
    let mut words = Vec::new();
    let mut current: Option<(usize, String)> = None;
    let mut quote = None;
    for (index, character) in line.char_indices() {
        match quote {
            Some(open) if character == open => quote = None,
            None if character.is_whitespace() => words.extend(current.take()),
            None if current.is_none() && QUOTES.contains(&character) => {
                current = Some((index, String::new()));
                quote = Some(character);
            }
            _ => current
                .get_or_insert_with(|| (index, String::new()))
                .1
                .push(character),
        }
    }
    words.push(current.unwrap_or((line.len(), String::new())));
    words
}

#[cfg(test)]
mod tests {
    use std::fs;

    use rstest::rstest;

    use super::*;
    use crate::{
        test_utils::{buffer_text, pinned_session, session_with_ids, with_context},
        ui::notification::NotificationLevel,
    };

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
    #[case::trailing_space("quit  ", Command::Quit)]
    #[case::apostrophe_in_word("filter-rows it's", Command::SetRowFilter(Some("it's".to_string())))]
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
    #[case::leading_space(" quit")]
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
    #[case::empty("", &[(0, "")])]
    #[case::words("a b", &[(0, "a"), (2, "b")])]
    #[case::trailing_space("a ", &[(0, "a"), (2, "")])]
    #[case::extra_whitespace("  a   b ", &[(2, "a"), (6, "b"), (8, "")])]
    #[case::single_quoted("'a b' c", &[(0, "a b"), (6, "c")])]
    #[case::double_quoted_with_single_quote("\"it's\"", &[(0, "it's")])]
    #[case::quote_inside_word_is_literal("it's a'b", &[(0, "it's"), (5, "a'b")])]
    #[case::text_after_closing_quote_joins("'a b'c", &[(0, "a bc")])]
    #[case::unterminated_quote("ab 'c d", &[(0, "ab"), (3, "c d")])]
    #[case::after_closed_quote("ab 'c d' ", &[(0, "ab"), (3, "c d"), (9, "")])]
    #[case::empty_quotes("''", &[(0, "")])]
    fn words_works(#[case] line: &str, #[case] expected: &[(usize, &str)]) {
        let expected: Vec<(usize, String)> = expected
            .iter()
            .map(|&(start, text)| (start, text.to_string()))
            .collect();

        assert_eq!(words(line), expected);
    }

    fn complete(line: &str) -> (usize, Vec<String>) {
        let session = pinned_session(3, &[1]);
        with_context(Some(&session), None, |ctx| {
            let completions = completions(line, ctx);
            (completions.word_start, completions.items)
        })
    }

    #[rstest]
    #[case::argument_after_space("jump-sequence ", 14, &["s0", "s2"])]
    #[case::argument_filtered_by_word("jump-sequence 2", 14, &["s2"])]
    #[case::alias("js ", 3, &["s0", "s2"])]
    #[case::opened_quote("jump-sequence 's", 14, &["s0", "s2"])]
    #[case::choice("set-diff-mode r", 14, &["reference"])]
    fn completions_works(#[case] line: &str, #[case] word_start: usize, #[case] expected: &[&str]) {
        assert_eq!(
            complete(line),
            (
                word_start,
                expected.iter().map(ToString::to_string).collect()
            )
        );
    }

    #[test]
    fn completions_lists_every_command_name_on_empty_line() {
        let (word_start, items) = complete("");

        assert_eq!(word_start, 0);
        assert_eq!(items.len(), COMMANDS.len());
    }

    #[test]
    fn completions_puts_best_command_match_first() {
        let (_, items) = complete("jump-seq");

        assert_eq!(items.first().map(String::as_str), Some("jump-sequence"));
    }

    #[rstest]
    #[case::fixed_command("quit ")]
    #[case::unknown_command("nope x")]
    #[case::no_completer("jump-position ")]
    fn completions_is_empty(#[case] line: &str) {
        assert_eq!(complete(line).1, Vec::<String>::new());
    }

    #[rstest]
    #[case::plain("s0", None, "s0")]
    #[case::leading_quote("'abc", None, "\"'abc\"")]
    #[case::whitespace("my seq", None, "'my seq'")]
    #[case::single_quote_inside("it's x", None, "\"it's x\"")]
    #[case::user_opened_quote("s0", Some('"'), "\"s0\"")]
    #[case::user_quote_inside_name("it's", Some('\''), "\"it's\"")]
    fn quoted_works(#[case] completion: &str, #[case] quote: Option<char>, #[case] expected: &str) {
        assert_eq!(quoted(completion, quote), expected);
    }

    fn prompt_after(typed: &str, keys: &[KeyCode]) -> String {
        let session = session_with_ids(&[("s0", b"A"), ("my seq", b"A"), ("it's x", b"A")]);
        with_context(Some(&session), None, |ctx| {
            let mut palette = PaletteState::default();
            for code in typed.chars().map(KeyCode::Char).chain(keys.iter().copied()) {
                palette.handle_key(KeyEvent::from(code), Rect::default(), ctx);
            }
            let area = Rect::new(0, 0, 60, 1);
            let mut buffer = Buffer::empty(area);
            palette.render(area, &mut buffer, ctx);
            buffer_text(&buffer, area)
        })
    }

    #[rstest]
    #[case::command_name("jump-seq", &[KeyCode::Tab], ":jump-sequence")]
    #[case::first_completion("jump-sequence ", &[KeyCode::Tab], ":jump-sequence s0")]
    #[case::next_completion_quoted("jump-sequence ", &[KeyCode::Tab, KeyCode::Tab], ":jump-sequence 'my seq'")]
    #[case::double_quotes_around_single("jump-sequence ", &[KeyCode::Tab; 3], ":jump-sequence \"it's x\"")]
    #[case::wraps_around("jump-sequence ", &[KeyCode::Tab; 4], ":jump-sequence s0")]
    #[case::shift_tab_backwards("jump-sequence ", &[KeyCode::BackTab], ":jump-sequence \"it's x\"")]
    #[case::inside_user_quote("jump-sequence \"", &[KeyCode::Tab], ":jump-sequence \"s0\"")]
    #[case::typing_restarts(
        "jump-sequence ",
        &[KeyCode::Tab, KeyCode::Tab, KeyCode::Char(' '), KeyCode::Tab],
        ":jump-sequence 'my seq' s0"
    )]
    #[case::no_completions("quit ", &[KeyCode::Tab], ":quit")]
    #[case::up_ignored("quit", &[KeyCode::Up], ":quit")]
    fn handle_key_works(#[case] typed: &str, #[case] keys: &[KeyCode], #[case] expected: &str) {
        assert_eq!(prompt_after(typed, keys), expected);
    }

    #[rstest]
    #[case::tab_opens_only_directory(&["inner/a.fa"], &[KeyCode::Tab, KeyCode::Tab], "inner/a.fa")]
    #[case::enter_opens_selected_directory(
        &["inner/a.fa", "other/b.fa"],
        &[KeyCode::Tab, KeyCode::Enter, KeyCode::Tab],
        "inner/a.fa"
    )]
    #[case::tab_keeps_siblings(&["inner/a.fa", "other/b.fa"], &[KeyCode::Tab, KeyCode::Tab], "other/")]
    fn handle_key_opens_directory(
        #[case] files: &[&str],
        #[case] keys: &[KeyCode],
        #[case] expected: &str,
    ) {
        let dir = tempfile::tempdir().unwrap();
        for file in files {
            let path = dir.path().join(file);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, "").unwrap();
        }
        let typed = format!("load-alignment {}/", dir.path().display());

        let line = with_context(None, None, |ctx| {
            let mut palette = PaletteState::default();
            for code in typed.chars().map(KeyCode::Char).chain(keys.iter().copied()) {
                palette.handle_key(KeyEvent::from(code), Rect::default(), ctx);
            }
            palette.input.text().to_string()
        });

        assert_eq!(line, typed + expected);
    }

    #[test]
    fn handle_key_enter_runs_line() {
        let session = pinned_session(2, &[]);
        let commands = with_context(Some(&session), None, |ctx| {
            let mut palette = PaletteState::default();
            for character in "quit".chars() {
                palette.handle_key(
                    KeyEvent::from(KeyCode::Char(character)),
                    Rect::default(),
                    ctx,
                );
            }
            palette.handle_key(KeyEvent::from(KeyCode::Enter), Rect::default(), ctx)
        });

        assert_eq!(commands, vec![Command::ClosePopup, Command::Quit]);
    }
}
