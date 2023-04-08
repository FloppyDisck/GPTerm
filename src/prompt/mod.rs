mod scroll_prompt;
mod static_prompt;

use arboard::Clipboard;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
pub use scroll_prompt::ScrollPrompt;
pub use static_prompt::StaticPrompt;

// TODO: implement three types of prompts
//  One that can autoresize box according to total text
//  One that auto scrolls to the right as i type beyond boundaries - easier
//  One that stays static - current

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
            KeyCode::Up => self.up(),
            KeyCode::Down => self.down(),
            _ => {}
        }
    }

    fn is_empty(&self) -> bool;

    fn str(&self) -> &str;

    fn flush(&mut self) -> String;

    fn down(&mut self);

    fn up(&mut self);

    fn left(&mut self);

    fn ctrl_left(&mut self);

    fn right(&mut self);

    fn ctrl_right(&mut self);

    fn add_char(&mut self, c: char);

    fn add_str(&mut self, s: &str);

    fn remove_char(&mut self);
}
