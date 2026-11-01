use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::text::Span;

/// A single-line text field with a cursor, edited with readline-style keys.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TextInput {
    value: String,
    /// Cursor position as a byte offset, always on a char boundary.
    cursor: usize,
    max_chars: usize,
}

impl TextInput {
    pub fn new(max_chars: usize) -> Self {
        Self {
            value: String::new(),
            cursor: 0,
            max_chars,
        }
    }

    #[must_use]
    pub fn with_value(mut self, value: &str) -> Self {
        self.value.clear();
        self.cursor = 0;
        self.insert_str(value);
        self
    }

    pub fn value(&self) -> &str {
        &self.value
    }

    pub fn is_empty(&self) -> bool {
        self.value.is_empty()
    }

    pub fn clear(&mut self) {
        self.value.clear();
        self.cursor = 0;
    }

    /// Display columns before the cursor, for placing the terminal cursor.
    pub fn cursor_column(&self) -> usize {
        Span::raw(&self.value[..self.cursor]).width()
    }

    /// Inserts pasted or typed text, dropping line breaks and control characters.
    pub fn insert_str(&mut self, text: &str) {
        for c in text.chars() {
            let c = if c == '\n' || c == '\t' { ' ' } else { c };
            if !c.is_control() {
                self.insert(c);
            }
        }
    }

    /// Applies an editing key. Returns `false` if the key is not an edit.
    pub fn handle_key(&mut self, key: KeyEvent) -> bool {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Char('a') if ctrl => self.cursor = 0,
            KeyCode::Char('e') if ctrl => self.cursor = self.value.len(),
            KeyCode::Char('u') if ctrl => {
                self.value.drain(..self.cursor);
                self.cursor = 0;
            }
            KeyCode::Char('w') if ctrl => self.delete_word(),
            KeyCode::Char(c) if !ctrl && !key.modifiers.contains(KeyModifiers::ALT) => {
                self.insert(c);
            }
            KeyCode::Backspace => {
                if let Some(start) = self.previous_boundary() {
                    self.value.drain(start..self.cursor);
                    self.cursor = start;
                }
            }
            KeyCode::Delete => {
                if let Some(end) = self.next_boundary() {
                    self.value.drain(self.cursor..end);
                }
            }
            KeyCode::Left => self.cursor = self.previous_boundary().unwrap_or(self.cursor),
            KeyCode::Right => self.cursor = self.next_boundary().unwrap_or(self.cursor),
            KeyCode::Home => self.cursor = 0,
            KeyCode::End => self.cursor = self.value.len(),
            _ => return false,
        }
        true
    }

    fn insert(&mut self, c: char) {
        if self.value.chars().count() < self.max_chars {
            self.value.insert(self.cursor, c);
            self.cursor += c.len_utf8();
        }
    }

    fn delete_word(&mut self) {
        let before = &self.value[..self.cursor];
        let trimmed = before.trim_end();
        let start = trimmed
            .rfind(char::is_whitespace)
            .map_or(0, |index| index + 1);
        self.value.drain(start..self.cursor);
        self.cursor = start;
    }

    fn previous_boundary(&self) -> Option<usize> {
        self.value[..self.cursor]
            .char_indices()
            .last()
            .map(|(i, _)| i)
    }

    fn next_boundary(&self) -> Option<usize> {
        self.value[self.cursor..]
            .chars()
            .next()
            .map(|c| self.cursor + c.len_utf8())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(input: &mut TextInput, code: KeyCode) {
        input.handle_key(KeyEvent::from(code));
    }

    fn ctrl(input: &mut TextInput, c: char) {
        input.handle_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL));
    }

    #[test]
    fn edits_at_the_cursor() {
        let mut input = TextInput::new(40).with_value("bench");
        press(&mut input, KeyCode::Home);
        input.insert_str("incline ");
        assert_eq!(input.value(), "incline bench");
        press(&mut input, KeyCode::Backspace);
        assert_eq!(input.value(), "inclinebench");
        press(&mut input, KeyCode::Delete);
        assert_eq!(input.value(), "inclineench");
    }

    #[test]
    fn handles_multibyte_text() {
        let mut input = TextInput::new(40).with_value("Füße 腿");
        assert_eq!(
            input.cursor_column(),
            7,
            "the CJK character is two columns wide"
        );
        press(&mut input, KeyCode::Backspace);
        press(&mut input, KeyCode::Left);
        press(&mut input, KeyCode::Backspace);
        assert_eq!(input.value(), "Füß ");
    }

    #[test]
    fn readline_shortcuts() {
        let mut input = TextInput::new(40).with_value("cable chest fly");
        ctrl(&mut input, 'w');
        assert_eq!(input.value(), "cable chest ");
        ctrl(&mut input, 'u');
        assert!(input.is_empty());
    }

    #[test]
    fn paste_is_flattened_and_capped() {
        let mut input = TextInput::new(5);
        input.insert_str("ab\ncd\u{7}efgh");
        assert_eq!(input.value(), "ab cd");
    }
}
