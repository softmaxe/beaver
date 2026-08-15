//! A one-line text field: the directory path, and nothing else.
//!
//! Kept deliberately small — enough editing to type or paste a path comfortably,
//! with a horizontal window so a long path scrolls instead of overflowing.

use unicode_width::UnicodeWidthChar;

#[derive(Debug, Default)]
pub struct TextInput {
    characters: Vec<char>,
    /// Cursor position, counted in characters, in `0..=characters.len()`.
    cursor: usize,
    /// First visible character, so a long path scrolls with the cursor.
    offset: usize,
}

impl TextInput {
    pub fn value(&self) -> String {
        self.characters.iter().collect()
    }

    pub fn is_empty(&self) -> bool {
        self.characters.is_empty()
    }

    pub fn set_value(&mut self, value: &str) {
        self.characters = value.chars().collect();
        self.cursor = self.characters.len();
        self.offset = 0;
    }

    pub fn insert(&mut self, character: char) {
        self.characters.insert(self.cursor, character);
        self.cursor += 1;
    }

    pub fn backspace(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
            self.characters.remove(self.cursor);
        }
    }

    pub fn delete(&mut self) {
        if self.cursor < self.characters.len() {
            self.characters.remove(self.cursor);
        }
    }

    pub fn move_left(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    pub fn move_right(&mut self) {
        self.cursor = (self.cursor + 1).min(self.characters.len());
    }

    pub fn move_home(&mut self) {
        self.cursor = 0;
    }

    pub fn move_end(&mut self) {
        self.cursor = self.characters.len();
    }

    /// Drop the last path segment, the way `Ctrl+W` behaves in a shell.
    pub fn delete_previous_word(&mut self) {
        while self.cursor > 0 && is_separator(self.characters[self.cursor - 1]) {
            self.backspace();
        }
        while self.cursor > 0 && !is_separator(self.characters[self.cursor - 1]) {
            self.backspace();
        }
    }

    /// The slice that fits in `width` columns, and where the cursor sits in it.
    ///
    /// Scrolling is decided here rather than at edit time, because it depends on
    /// how wide the field ends up being drawn.
    pub fn view(&mut self, width: usize) -> (String, usize) {
        if width == 0 {
            return (String::new(), 0);
        }
        // Keep one column spare so the cursor at the end of the text is visible.
        let usable = width.saturating_sub(1).max(1);
        if self.cursor < self.offset {
            self.offset = self.cursor;
        }
        while self.columns(self.offset, self.cursor) > usable {
            self.offset += 1;
        }
        let mut text = String::new();
        let mut used = 0;
        for character in &self.characters[self.offset.min(self.characters.len())..] {
            let character_width = character.width().unwrap_or(0);
            if used + character_width > width {
                break;
            }
            used += character_width;
            text.push(*character);
        }
        (text, self.columns(self.offset, self.cursor))
    }

    /// Display width of `characters[from..to]`.
    fn columns(&self, from: usize, to: usize) -> usize {
        self.characters[from.min(to)..to]
            .iter()
            .map(|character| character.width().unwrap_or(0))
            .sum()
    }
}

/// Path segments, not words: a folder called "season one" comes off in one go.
fn is_separator(character: char) -> bool {
    character == '/'
}

#[cfg(test)]
mod tests {
    use super::TextInput;

    fn input(value: &str) -> TextInput {
        let mut input = TextInput::default();
        input.set_value(value);
        input
    }

    #[test]
    fn edits_at_the_cursor() {
        let mut field = input("abc");
        field.move_left();
        field.insert('X');
        assert_eq!(field.value(), "abXc");
        field.backspace();
        assert_eq!(field.value(), "abc");
        field.delete();
        assert_eq!(field.value(), "ab");
    }

    #[test]
    fn deletes_one_path_segment_at_a_time() {
        let mut field = input("/tmp/shows/season one/");
        field.delete_previous_word();
        assert_eq!(field.value(), "/tmp/shows/");
        field.delete_previous_word();
        assert_eq!(field.value(), "/tmp/");
    }

    #[test]
    fn scrolls_to_keep_the_cursor_in_view() {
        let mut field = input("/a/very/long/path/that/does/not/fit");
        let (text, cursor) = field.view(10);
        assert!(text.ends_with("not/fit"));
        assert!(cursor < 10);

        field.move_home();
        let (text, cursor) = field.view(10);
        assert!(text.starts_with("/a/very"));
        assert_eq!(cursor, 0);
    }

    #[test]
    fn measures_wide_characters_by_column() {
        let mut field = input("字幕");
        let (_, cursor) = field.view(10);
        assert_eq!(cursor, 4);
    }
}
