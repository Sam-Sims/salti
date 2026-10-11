use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Widget,
};

#[derive(Debug, Default)]
pub(super) struct TextInput {
    text: String,
    cursor: usize,
}

impl TextInput {
    pub(super) fn text(&self) -> &str {
        &self.text
    }

    pub(super) fn set_text(&mut self, text: &str) {
        self.text = text.to_string();
        self.cursor = self.text.len();
    }

    pub(super) fn handle_key(&mut self, key: KeyEvent) -> bool {
        let control = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        match key.code {
            KeyCode::Char('a') if control => self.cursor = 0,
            KeyCode::Char('e') if control => self.cursor = self.text.len(),
            KeyCode::Char('u') if control => self.delete_back_to(0),
            KeyCode::Char('k') if control => self.text.truncate(self.cursor),
            KeyCode::Char('w') if control => self.delete_back_to(self.word_start()),
            KeyCode::Char(character) if !control && !alt => {
                self.text.insert(self.cursor, character);
                self.cursor += character.len_utf8();
            }
            KeyCode::Backspace => self.delete_back_to(self.previous_boundary()),
            KeyCode::Delete => {
                let end = self.next_boundary();
                self.text.drain(self.cursor..end);
            }
            KeyCode::Left => self.cursor = self.previous_boundary(),
            KeyCode::Right => self.cursor = self.next_boundary(),
            KeyCode::Home => self.cursor = 0,
            KeyCode::End => self.cursor = self.text.len(),
            _ => return false,
        }
        true
    }

    pub(super) fn render(&self, area: Rect, buf: &mut Buffer, style: Style) {
        let (before, rest) = self.text.split_at(self.cursor);
        let cursor_len = rest.chars().next().map_or(0, char::len_utf8);
        let (under_cursor, after) = rest.split_at(cursor_len);
        let under_cursor = if under_cursor.is_empty() {
            " "
        } else {
            under_cursor
        };
        Line::from(vec![
            Span::styled(before, style),
            Span::styled(under_cursor, style.add_modifier(Modifier::REVERSED)),
            Span::styled(after, style),
        ])
        .render(area, buf);
    }

    fn delete_back_to(&mut self, start: usize) {
        self.text.drain(start..self.cursor);
        self.cursor = start;
    }

    fn previous_boundary(&self) -> usize {
        self.text[..self.cursor]
            .char_indices()
            .next_back()
            .map_or(0, |(index, _)| index)
    }

    fn next_boundary(&self) -> usize {
        self.text[self.cursor..]
            .chars()
            .next()
            .map_or(self.cursor, |character| self.cursor + character.len_utf8())
    }

    fn word_start(&self) -> usize {
        self.text[..self.cursor]
            .trim_end()
            .trim_end_matches(|character: char| !character.is_whitespace())
            .len()
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    const NONE: KeyModifiers = KeyModifiers::NONE;
    const CONTROL: KeyModifiers = KeyModifiers::CONTROL;

    fn input(marked: &str) -> TextInput {
        TextInput {
            text: marked.replace('|', ""),
            cursor: marked.find('|').unwrap(),
        }
    }

    fn marked(mut input: TextInput) -> String {
        input.handle_key(KeyEvent::from(KeyCode::Char('|')));
        input.text().to_string()
    }

    #[rstest]
    #[case::types_at_end(KeyCode::Char('x'), NONE, "ab|", "abx|")]
    #[case::types_mid_line(KeyCode::Char('x'), NONE, "a|b", "ax|b")]
    #[case::types_shifted(KeyCode::Char('X'), KeyModifiers::SHIFT, "a|", "aX|")]
    #[case::types_multibyte(KeyCode::Char('é'), NONE, "a|b", "aé|b")]
    #[case::backspace(KeyCode::Backspace, NONE, "ab|c", "a|c")]
    #[case::backspace_at_start(KeyCode::Backspace, NONE, "|ab", "|ab")]
    #[case::backspace_multibyte(KeyCode::Backspace, NONE, "aé|", "a|")]
    #[case::delete(KeyCode::Delete, NONE, "a|bc", "a|c")]
    #[case::delete_at_end(KeyCode::Delete, NONE, "ab|", "ab|")]
    #[case::delete_multibyte(KeyCode::Delete, NONE, "a|éb", "a|b")]
    #[case::left(KeyCode::Left, NONE, "ab|", "a|b")]
    #[case::left_at_start(KeyCode::Left, NONE, "|ab", "|ab")]
    #[case::left_over_multibyte(KeyCode::Left, NONE, "aé|", "a|é")]
    #[case::right(KeyCode::Right, NONE, "a|b", "ab|")]
    #[case::right_at_end(KeyCode::Right, NONE, "ab|", "ab|")]
    #[case::right_over_multibyte(KeyCode::Right, NONE, "a|é", "aé|")]
    #[case::home(KeyCode::Home, NONE, "ab|c", "|abc")]
    #[case::end(KeyCode::End, NONE, "a|bc", "abc|")]
    #[case::control_a(KeyCode::Char('a'), CONTROL, "ab|c", "|abc")]
    #[case::control_e(KeyCode::Char('e'), CONTROL, "a|bc", "abc|")]
    #[case::control_u(KeyCode::Char('u'), CONTROL, "ab|cd", "|cd")]
    #[case::control_k(KeyCode::Char('k'), CONTROL, "ab|cd", "ab|")]
    #[case::control_w(KeyCode::Char('w'), CONTROL, "pin seq|", "pin |")]
    #[case::control_w_skips_trailing_space(KeyCode::Char('w'), CONTROL, "pin a b |", "pin a |")]
    #[case::control_w_mid_word(KeyCode::Char('w'), CONTROL, "ab cd|ef", "ab |ef")]
    #[case::control_w_first_word(KeyCode::Char('w'), CONTROL, "abc|", "|")]
    fn handle_key_works(
        #[case] code: KeyCode,
        #[case] modifiers: KeyModifiers,
        #[case] before: &str,
        #[case] expected: &str,
    ) {
        let mut input = input(before);

        assert!(input.handle_key(KeyEvent::new(code, modifiers)));
        assert_eq!(marked(input), expected);
    }

    #[rstest]
    #[case::up(KeyCode::Up, NONE)]
    #[case::down(KeyCode::Down, NONE)]
    #[case::tab(KeyCode::Tab, NONE)]
    #[case::back_tab(KeyCode::BackTab, KeyModifiers::SHIFT)]
    #[case::enter(KeyCode::Enter, NONE)]
    #[case::esc(KeyCode::Esc, NONE)]
    #[case::unbound_control(KeyCode::Char('x'), CONTROL)]
    #[case::alt(KeyCode::Char('b'), KeyModifiers::ALT)]
    fn handle_key_rejects(#[case] code: KeyCode, #[case] modifiers: KeyModifiers) {
        let mut input = input("a|b");

        assert!(!input.handle_key(KeyEvent::new(code, modifiers)));
        assert_eq!(marked(input), "a|b");
    }

    #[test]
    fn set_text_moves_cursor_to_end() {
        let mut input = input("|old");

        input.set_text("new");

        assert_eq!(marked(input), "new|");
    }
}
