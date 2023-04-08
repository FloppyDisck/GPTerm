use crate::Window;
use arboard::Clipboard;
use crossterm::event::{KeyCode, KeyEvent};
use tui::{backend::Backend, Frame};

pub struct Settings {}

impl Window for Settings {
    type InputReturn = bool;

    fn input(&mut self, key: &KeyEvent, clipboard: &mut Clipboard) -> Self::InputReturn {
        match key.code {
            KeyCode::Esc => true,
            _ => false,
        }
    }
}
