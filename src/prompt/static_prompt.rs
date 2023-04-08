use crate::prompt::Prompt;

pub struct StaticPrompt {
    pub text: String,
    pub cursor: usize,
}

impl StaticPrompt {
    pub fn new() -> Self {
        Self {
            text: String::default(),
            cursor: 0,
        }
    }
}

impl Prompt for StaticPrompt {
    fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    fn str(&self) -> &str {
        self.text.as_str()
    }

    fn flush(&mut self) -> String {
        let s = self.text.clone();
        self.cursor = 0;
        self.text.clear();
        s
    }

    fn down(&mut self) {
        self.cursor = 0;
    }

    fn up(&mut self) {
        self.cursor = self.text.len();
    }

    fn left(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    fn ctrl_left(&mut self) {
        if self.cursor > 0 {
            let (left, _) = self.text.split_at(self.cursor);
            self.cursor = left.rfind(char::is_whitespace).unwrap_or(0);
        }
    }

    fn right(&mut self) {
        if self.cursor < self.text.len() {
            self.cursor += 1;
        }
    }

    fn ctrl_right(&mut self) {
        if self.cursor < self.text.len() {
            let (_, right) = self.text.split_at(self.cursor + 1);
            if let Some(n) = right.find(char::is_whitespace) {
                self.cursor += n + 1;
            } else {
                self.cursor = self.text.len();
            }
        }
    }

    fn add_char(&mut self, c: char) {
        self.text.insert(self.cursor, c);
        self.right();
    }

    fn add_str(&mut self, s: &str) {
        if self.cursor == 0 {
            self.cursor = s.len();
            self.text = s.to_string() + self.text.as_str();
        } else if self.cursor == self.text.len() {
            self.text += s;
            self.cursor = self.text.len();
        } else {
            let (left, right) = self.text.split_at(self.cursor);
            self.cursor += s.len();
            self.text = left.to_owned() + s + right;
        }
    }

    fn remove_char(&mut self) {
        if !self.text.is_empty() && self.cursor > 0 {
            self.text.remove(self.cursor - 1);
            self.left();
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::prompt::{Prompt, StaticPrompt};
    use async_openai::types::Stop::String;

    fn prompt(s: &str) -> StaticPrompt {
        StaticPrompt {
            text: s.to_string(),
            cursor: 0,
        }
    }

    #[test]
    fn basic() {
        let mut p = prompt("Testing");

        assert_eq!(p.str(), "Testing");
        assert_eq!(p.flush(), "Testing".to_string());
        assert_eq!(p.str().len(), 0);
    }

    #[test]
    fn up() {
        let mut p = prompt("Go up");
        assert_eq!(p.cursor, 0);
        p.up();
        assert_eq!(p.cursor, 5);
    }

    #[test]
    fn down() {
        let mut p = prompt("Go down");
        p.up();
        assert_eq!(p.cursor, 7);
        p.down();
        assert_eq!(p.cursor, 0);
    }

    #[test]
    fn left() {
        let mut p = prompt("Go left");
        p.cursor = 3;
        p.left();
        assert_eq!(p.cursor, 2);

        // Test at limit
        p.cursor = 0;
        p.left();
        assert_eq!(p.cursor, 0);
    }

    #[test]
    fn ctrl_left() {
        let mut p = prompt("This has many spaces");
        p.cursor = 8;
        p.ctrl_left();
        assert_eq!(p.cursor, 4);
    }

    #[test]
    fn right() {
        let mut p = prompt("Go right");
        p.cursor = 3;
        p.right();
        assert_eq!(p.cursor, 4);

        p.cursor = 8;
        p.right();
        assert_eq!(p.cursor, 8);
    }

    #[test]
    fn ctrl_right() {
        let mut p = prompt("This has many spaces");
        p.cursor = 6;
        p.ctrl_right();
        assert_eq!(p.cursor, 8);
    }

    #[test]
    fn add_text() {
        let mut p = StaticPrompt::new();
        for s in "Testing".chars() {
            p.add_char(s);
        }
        assert_eq!(p.str(), "Testing");
        p.left();
        p.add_char('b');
        assert_eq!(p.str(), "Testinbg")
    }

    #[test]
    fn delete_text() {
        let mut p = prompt("Delete This");
        p.remove_char();
        assert_eq!(p.str(), "Delete This");
        p.right();
        p.right();
        p.remove_char();
        assert_eq!(p.str(), "Dlete This");
    }

    #[test]
    fn paste() {
        let mut p = prompt(" here");
        p.add_str("Paste");
        assert_eq!(p.str(), "Paste here");
        assert_eq!(p.cursor, 5);
    }
}
