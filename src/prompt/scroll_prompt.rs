use crate::prompt::Prompt;

pub struct ScrollPrompt {
    pub text: String,
    // Total allowed chars in the prompt
    pub size: usize,
    // Prompt cursor
    pub cursor: usize,
    // Prompt front cutoff
    pub offset: usize,
}

impl ScrollPrompt {
    pub fn new(size: u16) -> Self {
        Self {
            text: String::default(),
            size: size as usize,
            cursor: 0,
            offset: 0,
        }
    }

    pub fn update_size(&mut self, size: u16) {
        let size = size as usize;
        if self.size != size {
            let old = self.real_cursor();

            self.size = size;
            self.cursor = old % self.size;
            self.offset = old / self.size;

            if self.offset > self.max_offset() {
                self.up();
            }
        }
    }

    fn max_offset(&self) -> usize {
        self.text.len().saturating_sub(self.size)
    }

    // End of line
    fn eol(&self) -> bool {
        (self.cursor + self.offset) == self.text.len()
    }

    fn real_cursor(&self) -> usize {
        self.cursor + self.offset
    }

    fn max_cursor(&mut self) {
        self.cursor = if self.size > self.text.len() {
            self.text.len()
        } else {
            self.size
        }
    }

    fn overflow_right(&mut self, n: usize) {
        self.cursor += n;
        if self.cursor > self.size {
            self.offset += self.cursor - self.size;
            self.cursor = self.size;
        }
    }
}

impl Prompt for ScrollPrompt {
    fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    fn str(&self) -> &str {
        if self.offset == 0 {
            self.text.as_str()
        } else {
            self.text.split_at(self.offset).1
        }
    }

    fn flush(&mut self) -> String {
        let s = self.text.clone();
        self.down();
        self.text.clear();
        s
    }

    fn down(&mut self) {
        self.cursor = 0;
        self.offset = 0;
    }

    fn up(&mut self) {
        self.max_cursor();
        self.offset = self.max_offset();
    }

    fn left(&mut self) {
        if self.cursor != 0 {
            self.cursor -= 1;
        } else {
            self.offset = self.offset.saturating_sub(1);
        }
    }

    fn ctrl_left(&mut self) {
        if !(self.cursor == 0 && self.offset == 0) {
            let (left, _) = self.text.split_at(self.cursor + self.offset);
            let found = left.rfind(char::is_whitespace).unwrap_or(0);

            if found < self.offset {
                // Means cursor has reached the leftmost space
                self.cursor = 0;
                self.offset = found;
            } else if found > self.offset {
                self.cursor = found - self.offset;
            } else {
                // Cursor is in the start
                self.cursor = found;
                self.offset = 0;
            }
        }
    }

    fn right(&mut self) {
        if !self.eol() {
            if self.cursor < self.size {
                self.cursor += 1;
            } else {
                self.offset += 1;
            }
        }
    }

    fn ctrl_right(&mut self) {
        if !self.eol() {
            let (_, right) = self.text.split_at(self.cursor + self.offset);
            if let Some(n) = right.find(char::is_whitespace) {
                self.overflow_right(n + 1);
            } else {
                self.up();
            }
        }
    }

    fn add_char(&mut self, c: char) {
        self.text.insert(self.cursor + self.offset, c);
        self.right();
    }

    fn add_str(&mut self, s: &str) {
        let c = self.real_cursor();
        if c == 0 {
            self.text = s.to_string() + self.text.as_str();
            self.overflow_right(s.len())
        } else if c == self.text.len() {
            self.text += s;
            self.offset = self.max_offset();
        } else {
            let (left, right) = self.text.split_at(c);
            self.text = left.to_owned() + s + right;
            self.overflow_right(s.len())
        }
    }

    fn remove_char(&mut self) {
        if !self.text.is_empty() {
            let c = self.real_cursor();
            if c > 0 {
                self.text.remove(c - 1);
                self.left();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::prompt::{Prompt, ScrollPrompt};

    fn prompt(s: &str) -> ScrollPrompt {
        ScrollPrompt {
            text: s.to_string(),
            size: 5,
            cursor: 0,
            offset: 0,
        }
    }

    #[test]
    fn basic() {
        let mut p = prompt("Basic");
        assert_eq!(p.str(), "Basic");
        p.offset = 2;
        assert_eq!(p.str(), "sic");
        assert_eq!(p.flush(), "Basic".to_string());
        assert_eq!(p.str().len(), 0);
    }

    #[test]
    fn up() {
        let mut p = prompt("Direction test");
        assert_eq!(p.cursor, 0);
        assert_eq!(p.offset, 0);
        p.up();
        assert_eq!(p.cursor, 5);
        assert_eq!(p.offset, 9);
        assert_eq!(p.str(), " test");
    }

    #[test]
    fn down() {
        let mut p = prompt("Direction test");
        p.up();
        p.down();
        assert_eq!(p.cursor, 0);
        assert_eq!(p.offset, 0);
    }

    #[test]
    fn left() {
        let mut p = prompt("Go left");
        // Cursor out of bounds without offset
        p.left();
        assert_eq!(p.cursor, 0);

        // Cursor within bounds without offset
        p.cursor = 3;
        p.left();
        assert_eq!(p.cursor, 2);

        // Cursor out of bounds with offset
        p.cursor = 0;
        p.offset = 2;
        p.left();
        assert_eq!(p.cursor, 0);
        assert_eq!(p.offset, 1);

        // Cursor within bounds with offset
        p.cursor = 2;
        p.offset = 2;
        p.left();
        assert_eq!(p.cursor, 1);
        assert_eq!(p.offset, 2);
    }

    #[test]
    fn ctrl_left() {
        let mut p = prompt("many words here");
        p.size = 7;
        // Cursor out of bounds without offset
        p.ctrl_left();
        assert_eq!(p.cursor, 0);
        assert_eq!(p.offset, 0);

        // Cursor within bounds without offset
        p.cursor = 7;
        p.ctrl_left();
        assert_eq!(p.cursor, 4);

        // Cursor out of bounds with offset
        p.cursor = 1;
        p.offset = 6;
        p.ctrl_left();
        assert_eq!(p.cursor, 0);
        assert_eq!(p.offset, 4);
        assert_eq!(p.str(), " words here");

        // Cursor within bounds with offset
        p.offset = 5;
        p.cursor = 7;
        p.ctrl_left();
        assert_eq!(p.offset, 5);
        assert_eq!(p.cursor, 5);
    }

    #[test]
    fn right() {
        let mut p = prompt("Go right");
        // Cursor out of bounds without offset
        p.cursor = p.size;
        p.right();
        assert_eq!(p.cursor, p.size);
        assert_eq!(p.offset, 1);

        // Cursor within bounds without offset
        p.cursor = 2;
        p.offset = 0;
        p.right();
        assert_eq!(p.cursor, 3);
        assert_eq!(p.offset, 0);

        // Cursor out of bounds with offset
        p.cursor = p.size;
        p.offset = 2;
        p.right();
        assert_eq!(p.cursor, p.size);
        assert_eq!(p.offset, 3);

        p.cursor = p.size;
        p.offset = 3;
        p.right();
        assert_eq!(p.cursor, p.size);
        assert_eq!(p.offset, 3);

        // Cursor within bounds with offset
        p.cursor = 2;
        p.offset = 1;
        p.right();
        assert_eq!(p.cursor, 3);
        assert_eq!(p.offset, 1);
    }

    #[test]
    fn ctrl_right() {
        let mut p = prompt("many words here");
        // Cursor out of bounds without offset
        p.cursor = 5;
        p.ctrl_right();
        assert_eq!(p.offset, 6);
        assert_eq!(p.cursor, 5);
        assert_eq!(p.str(), "ords here");

        // Cursor within bounds without offset
        p.offset = 0;
        p.cursor = 0;
        p.ctrl_right();
        assert_eq!(p.offset, 0);
        assert_eq!(p.cursor, 5);

        // Cursor out of bounds with offset
        p.cursor = 5;
        p.offset = 1;
        p.ctrl_right();
        assert_eq!(p.offset, 6);
        assert_eq!(p.cursor, 5);
        assert_eq!(p.str(), "ords here");

        // Cursor within bounds with offset
        p.cursor = 1;
        p.offset = 1;
        p.ctrl_right();
        assert_eq!(p.str(), "any words here");
        assert_eq!(p.offset, 1);
        assert_eq!(p.cursor, 4);
    }

    #[test]
    fn add_text() {
        let mut p = prompt("");
        // Cursor out of bounds without offset
        p.add_char('a');
        assert_eq!(p.offset, 0);
        assert_eq!(p.cursor, 1);

        // Cursor within bounds without offset
        p.left();
        p.add_char('b');
        assert_eq!(p.offset, 0);
        assert_eq!(p.cursor, 1);
        assert_eq!(p.str(), "ba");

        // Cursor out of bounds with offset
        let mut p = prompt("some mising word");
        p.offset = 11;
        p.cursor = 5;
        p.add_char('s');
        assert_eq!(p.str(), "words");

        // Cursor within bounds with offset
        p.offset = 5;
        p.cursor = 3;
        p.add_char('s');
        assert_eq!(p.str(), "missing words");
    }

    #[test]
    fn delete_text() {
        let mut p = prompt("Some text here");
        // Cursor out of bounds without offset
        p.offset = 0;
        p.cursor = 0;
        p.remove_char();
        assert_eq!(p.str(), "Some text here");

        // Cursor within bounds without offset
        p.offset = 0;
        p.cursor = 2;
        p.remove_char();
        assert_eq!(p.str(), "Sme text here");

        // Cursor out of bounds with offset
        p.offset = 3;
        p.cursor = 0;
        p.remove_char();
        assert_eq!(p.str(), " text here");

        // Cursor within bounds with offset
        p.offset = 2;
        p.cursor = 2;
        p.remove_char();
        assert_eq!(p.str(), " ext here");
    }

    #[test]
    fn paste() {
        let mut p = prompt("Fnish pasting t t");
        // Cursor out of bounds without offset
        p.cursor = 5;
        p.add_str("ed");
        assert_eq!(p.str(), "ished pasting t t");

        // Cursor within bounds without offset
        p.cursor = 1;
        p.offset = 0;
        p.add_str("iii");
        assert_eq!(p.str(), "Fiiinished pasting t t");

        // Cursor out of bounds with offset
        p.up();
        p.add_str("ext");
        assert_eq!(p.str(), " text");

        // Cursor within bounds with offset
        p.offset -= 1;
        p.cursor = 1;
        p.add_str("his");
        assert_eq!(p.text.as_str(), "Fiiinished pasting this text");
        assert_eq!(p.str(), "this text");
    }

    #[test]
    fn update_size() {
        let mut p = prompt("123456789");
        p.offset = 4;
        assert_eq!(p.str(), "56789");
        p.update_size(20);
        assert_eq!(p.str(), "123456789");
    }
}
