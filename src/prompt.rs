use arboard::Clipboard;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

pub struct TextPrompt {
    pub text: String,
    pub cursor: usize,
}

impl TextPrompt {
    pub fn new() -> Self {
        TextPrompt {
            text: String::default(),
            cursor: 0,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    pub fn str(&self) -> &str {
        self.text.as_str()
    }

    pub fn flush(&mut self) -> String {
        let s = self.text.clone();
        self.cursor = 0;
        self.text.clear();
        s
    }
}

impl Prompt for TextPrompt {
    fn start(&mut self) {
        self.cursor = 0;
    }

    fn end(&mut self) {
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

    fn step_right(&mut self, step: usize) {
        // TODO: add security
        self.cursor += step;
    }

    fn right(&mut self) {
        if self.cursor < self.text.len() {
            self.step_right(1);
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
        self.text += s;
        self.step_right(s.len());
    }

    fn remove_char(&mut self) {
        if !self.text.is_empty() {
            self.text.remove(self.cursor - 1);
            self.left();
        }
    }
}

pub trait Prompt {
    // Process input, also requests a clipboard to process pasting
    fn input(&mut self, key: &KeyEvent, clipboard: &mut Clipboard) {
        match key.code {
            KeyCode::Char(c) => {
                if key.modifiers.contains(KeyModifiers::CONTROL) && c == 'v' {
                    let paste = clipboard.get_text().unwrap();
                    self.add_str(paste.as_str())
                } else {
                    self.add_char(c)
                }
            }
            KeyCode::Backspace => self.remove_char(),
            KeyCode::Right => {
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    self.ctrl_right()
                } else {
                    self.right()
                }
            }
            KeyCode::Left => {
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    self.ctrl_left()
                } else {
                    self.left()
                }
            }
            KeyCode::Up => self.end(),
            KeyCode::Down => self.start(),
            _ => {}
        }
    }

    fn start(&mut self);

    fn end(&mut self);

    fn left(&mut self);

    fn ctrl_left(&mut self);

    fn step_right(&mut self, step: usize);

    fn right(&mut self);

    fn ctrl_right(&mut self);

    fn add_char(&mut self, c: char);

    fn add_str(&mut self, s: &str);

    fn remove_char(&mut self);
}
