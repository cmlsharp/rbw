/// A single-line text buffer with cursor position tracking.
#[derive(Debug, Clone)]
pub struct TextInput {
    text: String,
    /// Byte offset of the cursor within `text`.
    cursor: usize,
}

impl TextInput {
    pub fn new() -> Self {
        Self {
            text: String::new(),
            cursor: 0,
        }
    }

    pub fn from_str(s: &str) -> Self {
        Self {
            cursor: s.len(),
            text: s.to_string(),
        }
    }

    pub fn as_str(&self) -> &str {
        &self.text
    }

    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// Character offset of the cursor (0 = before first char, char_count = after last).
    pub fn cursor_char_offset(&self) -> usize {
        self.text[..self.cursor].chars().count()
    }

    pub fn insert(&mut self, ch: char) {
        self.text.insert(self.cursor, ch);
        self.cursor += ch.len_utf8();
    }

    pub fn backspace(&mut self) {
        if self.cursor > 0 {
            // Find the preceding char boundary
            let prev = self.text[..self.cursor]
                .char_indices()
                .next_back()
                .map_or(0, |(i, _)| i);
            self.text.drain(prev..self.cursor);
            self.cursor = prev;
        }
    }

    pub fn delete(&mut self) {
        if self.cursor < self.text.len() {
            let next = self.cursor
                + self.text[self.cursor..]
                    .chars()
                    .next()
                    .map_or(0, char::len_utf8);
            self.text.drain(self.cursor..next);
        }
    }

    pub fn move_left(&mut self) {
        if self.cursor > 0 {
            self.cursor = self.text[..self.cursor]
                .char_indices()
                .next_back()
                .map_or(0, |(i, _)| i);
        }
    }

    pub fn move_right(&mut self) {
        if self.cursor < self.text.len() {
            self.cursor += self.text[self.cursor..]
                .chars()
                .next()
                .map_or(0, char::len_utf8);
        }
    }

    pub fn move_to_start(&mut self) {
        self.cursor = 0;
    }

    pub fn move_to_end(&mut self) {
        self.cursor = self.text.len();
    }

    /// Delete from cursor to the start of the current word.
    pub fn delete_word_back(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let before = &self.text[..self.cursor];
        // Skip trailing whitespace, then skip the word.
        let end = before.trim_end().len();
        let start = before[..end]
            .rfind(|c: char| c.is_whitespace())
            .map_or(0, |i| i + 1);
        self.text.drain(start..self.cursor);
        self.cursor = start;
    }

    /// Replace entire text and put cursor at the end.
    pub fn set(&mut self, s: &str) {
        self.text = s.to_string();
        self.cursor = self.text.len();
    }

    /// Clear all text.
    pub fn clear(&mut self) {
        self.text.clear();
        self.cursor = 0;
    }
}

impl Default for TextInput {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for TextInput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.text)
    }
}
