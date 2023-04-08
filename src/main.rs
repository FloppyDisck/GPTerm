mod chat;
mod creator;
mod prompt;
mod settings;

use crate::{
    chat::Chats,
    creator::{Action, Creator},
    settings::Settings,
};
use arboard::Clipboard;
use crossterm::event::poll;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::io;
use std::time::Duration;
use tui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout, Rect},
    Frame, Terminal,
};

// TODO: improve naming on many of these
pub trait Window {
    type InputReturn;

    // Updates the screen
    fn update<B: Backend>(&mut self, f: &mut Frame<B>) {
        self.update_size(f.size());
        self.draw(f);
    }
    // Handle screen updates, useful when caching
    fn update_size(&mut self, size: Rect) {}
    // Handle the screen writing
    fn draw<B: Backend>(&self, f: &mut Frame<B>) {}
    // Handle the input handling and processing
    fn input(&mut self, key: &KeyEvent, clipboard: &mut Clipboard) -> Self::InputReturn;
}

struct App {
    view_state: ViewState,
    chats: Chats,
    creator: Creator,
    settings: Settings,
}

enum ViewState {
    // Using the menu
    Chats,
    // Settings page
    Settings,
    // Creating a chat
    NewChat,
}

impl App {
    fn new() -> Self {
        Self {
            view_state: ViewState::Chats,
            chats: Chats::new(),
            creator: Creator::new(),
            settings: Settings {},
        }
    }
}

impl App {
    fn run<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> io::Result<()> {
        let mut clipboard = Clipboard::new().unwrap();

        loop {
            terminal.draw(|f| self.update(f)).unwrap();
            // TODO setting for this
            if poll(Duration::from_millis(100))? {
                if let Event::Key(key) = event::read()? {
                    if self.input(&key, &mut clipboard) {
                        break;
                    }
                }
            }
        }

        Ok(())
    }
}

impl Window for App {
    type InputReturn = bool;

    fn update<B: Backend>(&mut self, f: &mut Frame<B>) {
        match &self.view_state {
            ViewState::Chats => self.chats.update(f),
            ViewState::Settings => self.settings.update(f),
            ViewState::NewChat => self.creator.update(f),
        }
    }

    fn input(&mut self, key: &KeyEvent, clipboard: &mut Clipboard) -> Self::InputReturn {
        // Only input routing is done here
        match &self.view_state {
            ViewState::Chats => {
                if key.code == KeyCode::Char('a') && !self.chats.writing {
                    self.view_state = ViewState::NewChat
                }
                if key.code == KeyCode::Esc && !self.chats.writing {
                    self.view_state = ViewState::Settings
                } else {
                    // Simply returns a bool
                    if self.chats.input(key, clipboard) {
                        return true;
                    }
                }
            }
            ViewState::Settings => {
                // Figure out how this works later, maybe return a type that mutates everything
                if self.settings.input(key, clipboard) {
                    self.view_state = ViewState::Chats;
                }
            }
            ViewState::NewChat => {
                // Returns an option enum with an action,
                // None means do nothing, Creator::Quit means go back and
                // Creator::New() means add a new chat
                if let Some(action) = self.creator.input(key, clipboard) {
                    match action {
                        Action::Quit => self.view_state = ViewState::Chats,
                        Action::New { title, system } => {
                            let system = if system.is_empty() {
                                None
                            } else {
                                Some(system)
                            };
                            self.chats.add_chat(title, system);
                            self.view_state = ViewState::Chats;
                        }
                    }
                }
            }
        }

        return false;
    }
}

// TODO: process when stop signal is end
// TODO: copy/paste support

// Optional stuff for when everything works
// TODO: have a main thread for UI writing and input handling
// TODO: have a secondary thread for query processing
// TODO: message streaming so we see the text appear as it received it

pub fn popup(percent_x: u16, height: u16, r: Rect) -> Rect {
    let layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(
            [
                Constraint::Percentage((100 - percent_x) / 2),
                Constraint::Percentage(percent_x),
                Constraint::Percentage((100 - percent_x) / 2),
            ]
            .as_ref(),
        )
        .split(r);

    // Avoid weird layout issues
    let total_height = r.height - height;
    let half_height = total_height / 2;

    Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Length(half_height),
                Constraint::Min(height),
                Constraint::Length(total_height - half_height),
            ]
            .as_ref(),
        )
        .split(layout[1])[1]
}

//#[tokio::main]
fn main() -> Result<(), io::Error> {
    // setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();
    app.run(&mut terminal).unwrap();

    // restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}
