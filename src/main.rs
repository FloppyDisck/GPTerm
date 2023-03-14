use crossterm::event::KeyModifiers;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::{io, thread, time::Duration};
use tui::backend::Backend;
use tui::layout::Corner;
use tui::style::{Color, Modifier, Style};
use tui::text::{Span, Spans};
use tui::widgets::{List, ListItem, Paragraph, Tabs};
use tui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    widgets::{Block, Borders, Widget},
    Frame, Terminal,
};

enum InputState {
    // Using the menu
    Menu,
    // Writing an ai prompt
    Writing,
    // Settings page
    Settings,
}

enum Sender {
    User,
    System,
    Ai,
}

struct Message {
    // Message sender
    sender: Sender,
    // Message
    message: String,
}

impl Message {
    fn user(message: String) -> Message {
        Message {
            sender: Sender::User,
            message,
        }
    }
}

struct Chat {
    // Chat title
    title: String,
    // Message history
    history: Vec<Message>,
    // Message cursor
    chat_cursor: usize,
    // Prompt being written
    prompt: String,
    // Prompt cursor
    prompt_cursor: usize,
    // Used to lock the prompt when the API is loading a response
    loading: bool,
}

impl Chat {
    fn new(title: &str) -> Self {
        Self {
            title: title.to_string(),
            history: vec![],
            chat_cursor: 0,
            prompt: "".to_string(),
            prompt_cursor: 0,
            loading: false,
        }
    }

    fn messages(&self) -> &[Message] {
        &self.history[..self.history.len() - self.chat_cursor]
    }

    fn scroll_up(&mut self) {
        if self.chat_cursor < self.history.len() - 1 {
            self.chat_cursor += 1;
        }
    }

    fn scroll_down(&mut self) {
        self.chat_cursor = self.chat_cursor.saturating_sub(1);
    }

    fn cursor_left(&mut self) {
        self.prompt_cursor = self.prompt_cursor.saturating_sub(1);
    }

    fn skip_cursor_left(&mut self) {
        if self.prompt_cursor > 0 {
            let (left, _) = self.prompt.split_at(self.prompt_cursor);
            if let Some(n) = left.rfind(char::is_whitespace) {
                self.prompt_cursor = n;
            } else {
                self.prompt_cursor = 0;
            }
        }
    }

    fn cursor_right(&mut self) {
        if self.prompt_cursor < self.prompt.len() {
            self.prompt_cursor += 1;
        }
    }

    fn skip_cursor_right(&mut self) {
        if self.prompt_cursor < self.prompt.len() {
            let (_, right) = self.prompt.split_at(self.prompt_cursor + 1);
            if let Some(n) = right.find(char::is_whitespace) {
                self.prompt_cursor += n + 1;
            } else {
                self.prompt_cursor = self.prompt.len();
            }
        }
    }

    fn add_char(&mut self, c: char) {
        self.prompt.insert(self.prompt_cursor, c);
        self.cursor_right();
    }

    fn remove_char(&mut self) {
        if !self.prompt.is_empty() {
            self.prompt.remove(self.prompt_cursor - 1);
            self.cursor_left();
        }
    }
}

struct App {
    pub input_state: InputState,
    pub chats: Vec<Chat>,
    pub index: usize,
}

impl App {
    fn new() -> App {
        App {
            input_state: InputState::Menu,
            chats: vec![
                Chat::new("Temperature Sensor"),
                Chat::new("Networking"),
                Chat::new("Random topic"),
            ],
            index: 0,
        }
    }

    fn chat_mut(&mut self) -> &mut Chat {
        self.chats.get_mut(self.index).unwrap()
    }

    fn chat(&self) -> &Chat {
        self.chats.get(self.index).unwrap()
    }

    fn user_message(&mut self) {
        let chat = self.chat_mut();
        chat.history.push(Message::user(chat.prompt.clone()));
        chat.chat_cursor = 0;
        chat.prompt_cursor = 0;
        chat.prompt.clear();
    }

    fn next_tab(&mut self) {
        self.index = (self.index + 1) % self.chats.len();
    }

    fn previous_tab(&mut self) {
        if self.index == 0 {
            self.index = self.chats.len() - 1;
        } else {
            self.index -= 1;
        }
    }

    fn run<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> io::Result<()> {
        loop {
            terminal.draw(|f| self.draw(f));

            // TODO: pressing N created a new chat and D deletes the current chat
            // TODO: popup when creating and deleting, ask for a title
            if let Event::Key(key) = event::read()? {
                match self.input_state {
                    InputState::Menu => match key.code {
                        KeyCode::Char('q') => return Ok(()),
                        KeyCode::Esc => self.input_state = InputState::Settings,
                        KeyCode::Right => self.next_tab(),
                        KeyCode::Left => self.previous_tab(),
                        KeyCode::Enter => self.input_state = InputState::Writing,
                        KeyCode::Up => self.chat_mut().scroll_up(),
                        KeyCode::Down => self.chat_mut().scroll_down(),
                        _ => {}
                    },
                    InputState::Writing => match key.code {
                        KeyCode::Char('q') => return Ok(()),
                        KeyCode::Esc => self.input_state = InputState::Menu,
                        KeyCode::Char(c) => self.chat_mut().add_char(c),
                        KeyCode::Backspace => self.chat_mut().remove_char(),
                        KeyCode::Right => {
                            let chat = self.chat_mut();
                            if key.modifiers.contains(KeyModifiers::CONTROL) {
                                chat.skip_cursor_right()
                            } else {
                                chat.cursor_right()
                            }
                        }
                        KeyCode::Left => {
                            let chat = self.chat_mut();
                            if key.modifiers.contains(KeyModifiers::CONTROL) {
                                chat.skip_cursor_left()
                            } else {
                                chat.cursor_left()
                            }
                        }
                        KeyCode::Enter => {
                            let chat = self.chat();
                            if !chat.loading && !chat.prompt.is_empty() {
                                self.user_message();
                            }
                        }
                        KeyCode::Up => self.chat_mut().prompt_cursor = 0,
                        KeyCode::Down => {
                            let chat = self.chat_mut();
                            chat.prompt_cursor = chat.prompt.len();
                        }
                        _ => {}
                    },
                    InputState::Settings => match key.code {
                        KeyCode::Char('q') => return Ok(()),
                        KeyCode::Esc => self.input_state = InputState::Menu,
                        _ => {}
                    },
                }
            }
        }
    }

    fn draw<B: Backend>(&self, f: &mut Frame<B>) {
        let size = f.size();
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(2)
            .constraints(
                [
                    Constraint::Length(3),
                    // A chat is 3 lines long
                    Constraint::Min(3),
                    Constraint::Length(3),
                ]
                .as_ref(),
            )
            .split(size);

        // Display tab
        let tab_titles = self
            .chats
            .iter()
            .map(|t| Spans::from(t.title.as_str()))
            .collect();
        let tabs = Tabs::new(tab_titles)
            .block(Block::default().borders(Borders::ALL).title("Chats"))
            .select(self.index)
            .style(Style::default())
            .highlight_style(
                Style::default()
                    .add_modifier(Modifier::BOLD)
                    .bg(Color::Black),
            );
        f.render_widget(tabs, chunks[0]);

        let chat = self.chat();

        // Display message history
        let messages: Vec<ListItem> = chat
            .messages()
            .iter()
            .rev()
            .map(|message| {
                let (sender, s_color) = match message.sender {
                    Sender::User => ("User", Color::Green),
                    Sender::System => ("Systems", Color::Red),
                    Sender::Ai => ("ChatGPT", Color::Blue),
                };

                let header = Spans::from(vec![
                    Span::styled(format!("{:<9}", sender), Style::default().fg(s_color)),
                    Span::styled(
                        "Date goes here",
                        Style::default().add_modifier(Modifier::ITALIC),
                    ),
                ]);
                let msg = Spans::from(message.message.as_str());

                ListItem::new(vec![
                    Spans::from("-".repeat(chunks[2].width as usize)),
                    header,
                    Spans::from(""),
                    msg,
                ])
            })
            .collect();

        let message_box = List::new(messages)
            .block(Block::default().borders(Borders::ALL).title("Messages"))
            .start_corner(Corner::BottomLeft);
        f.render_widget(message_box, chunks[1]);

        // Display input box
        let input = Paragraph::new(chat.prompt.as_str())
            .style(match self.input_state {
                InputState::Writing => Style::default().fg(if chat.loading {
                    Color::Red
                } else {
                    Color::Blue
                }),
                _ => Style::default(),
            })
            .block(Block::default().borders(Borders::ALL).title("Input"));
        f.render_widget(input, chunks[2]);
        match self.input_state {
            InputState::Writing => {
                f.set_cursor(
                    // Cursor goes past the
                    chunks[2].x + chat.prompt_cursor as u16 + 1,
                    // Move to where the text is
                    chunks[2].y + 1,
                )
            }
            _ => {}
        }
    }
}

fn main() -> Result<(), io::Error> {
    // setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();
    app.run(&mut terminal);

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
