use crate::prompt::{Prompt, TextPrompt};
use crate::{popup, Window};
use arboard::Clipboard;
use crossterm::event::{KeyCode, KeyEvent};
use tui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

enum Focus {
    Title,
    System,
}

pub enum Action {
    Quit,
    New { title: String, system: String },
}

/// Chat creation window
pub struct Creator {
    focus: Focus,
    title: TextPrompt,
    system: TextPrompt,
}

impl Creator {
    pub fn new() -> Self {
        Self {
            focus: Focus::Title,
            title: TextPrompt::new(),
            system: TextPrompt::new(),
        }
    }

    fn flush(&mut self) -> Action {
        Action::New {
            title: self.title.flush(),
            system: self.system.flush(),
        }
    }

    fn reset(&mut self) {
        self.focus = Focus::Title;
        self.title = TextPrompt::new();
        self.system = TextPrompt::new();
    }

    fn next(&mut self) {
        match self.focus {
            Focus::Title => self.focus = Focus::System,
            Focus::System => self.focus = Focus::Title,
        }
    }

    fn current_prompt(&mut self) -> &mut TextPrompt {
        match self.focus {
            Focus::Title => &mut self.title,
            Focus::System => &mut self.system,
        }
    }
}

impl Window for Creator {
    type InputReturn = Option<Action>;

    fn draw<B: Backend>(&self, f: &mut Frame<B>) {
        let area = popup(60, 6, f.size());
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(3), Constraint::Min(3)].as_ref())
            .split(area);

        let (chat_style, system_style) = match self.focus {
            Focus::Title => (Style::default(), Style::default().fg(Color::DarkGray)),
            Focus::System => (Style::default().fg(Color::DarkGray), Style::default()),
        };

        let new_chat = Paragraph::new(self.title.str())
            .style(chat_style)
            .block(Block::default().title("New Chat").borders(Borders::ALL));
        f.render_widget(new_chat, layout[0]);

        let system = Paragraph::new(self.system.str())
            .style(system_style)
            .block(Block::default().title("System").borders(Borders::ALL));
        f.render_widget(system, layout[1]);

        let (selected, cursor) = match self.focus {
            Focus::Title => (layout[0], self.title.cursor),
            Focus::System => (layout[1], self.system.cursor),
        };

        f.set_cursor(
            // Cursor goes past the
            selected.x + cursor as u16 + 1,
            // Move to where the text is
            selected.y + 1,
        )
    }

    fn input(&mut self, key: &KeyEvent, clipboard: &mut Clipboard) -> Self::InputReturn {
        match key.code {
            KeyCode::Esc => {
                self.reset();
                Some(Action::Quit)
            }
            KeyCode::Tab => {
                self.next();
                None
            }
            KeyCode::Enter => {
                if !self.title.is_empty() {
                    Some(self.flush())
                } else {
                    Some(Action::Quit)
                }
            }
            _ => {
                self.current_prompt().input(key, clipboard);
                None
            }
        }
    }
}
