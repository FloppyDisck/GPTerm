use crate::prompt::{Prompt, TextPrompt};
use crate::Window;
use arboard::Clipboard;
use async_openai::{
    types::{
        ChatCompletionRequestMessage as Message, CreateChatCompletionRequestArgs as ChatModel, Role,
    },
    Client,
};
use crossterm::event::{KeyCode, KeyEvent};
use tui::layout::Rect;
use tui::{
    backend::Backend,
    layout::{Constraint, Corner, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Span, Spans},
    widgets::{Block, Borders, List, ListItem, Paragraph, Tabs},
    Frame,
};

// Current OpenAI API is very unergonomic so we gotta do some helper functions
fn user_msg(msg: String) -> Message {
    Message {
        role: Role::User,
        content: msg,
        name: None,
    }
}

fn system_msg(msg: String) -> Message {
    Message {
        role: Role::System,
        content: msg,
        name: None,
    }
}

fn assistant_msg(msg: String) -> Message {
    Message {
        role: Role::Assistant,
        content: msg,
        name: None,
    }
}

enum MessageContent {
    Sender(Role),
    Line(String),
    Divider,
}

pub struct Chat {
    // Chat title
    title: String,
    // Message history
    messages: Vec<Message>,
    // Used for storing preprocessed messages
    wrapped_messages: Vec<MessageContent>,
    // Means the offset of the currently shown chat
    message_offset: usize,
    // Last known height, this will be used for reprocessing the chats
    last_size: Rect,
    // Prompt being written
    prompt: TextPrompt,
    // Used to lock the prompt when the API is loading a response
    loading: bool,
    // TODO: maybe add a streamed_answer string
}

impl Default for Chat {
    fn default() -> Self {
        Chat::new("New Chat".to_string(), None)
    }
}

fn wrapped_msg(msg: &Message, size: &Rect) -> Vec<MessageContent> {
    let mut msgs = vec![];

    msgs.append(&mut vec![
        MessageContent::Divider,
        MessageContent::Sender(msg.role.clone()),
    ]);

    for line in textwrap::wrap(&msg.content, (size.width - 4) as usize) {
        msgs.push(MessageContent::Line(line.to_string()));
    }

    msgs
}

impl Chat {
    fn new(name: String, system: Option<String>) -> Self {
        let mut messages = vec![];

        if let Some(system) = system {
            messages.push(system_msg(system));
        }

        Self {
            title: name,
            messages,
            // We avoid splitting them since we will init this when we draw
            // and last_height != height
            wrapped_messages: vec![],
            message_offset: 0,
            last_size: Rect::default(),
            prompt: TextPrompt::new(),
            loading: false,
        }
    }

    fn new_message(&mut self, message: Message) {
        self.wrapped_messages
            .append(&mut wrapped_msg(&message, &self.last_size));
        self.messages.push(message);
        self.message_offset = 0;
    }

    fn user(&mut self) {
        let msg = self.prompt.flush();
        self.new_message(user_msg(msg))
    }

    fn scroll_up(&mut self) {
        if self.message_offset < self.wrapped_messages.len().saturating_sub(1) {
            self.message_offset += 1;
        }
    }

    fn scroll_down(&mut self) {
        self.message_offset = self.message_offset.saturating_sub(1);
    }

    fn update_box(&mut self, size: Rect) {
        if size != self.last_size {
            self.last_size = size;

            let wrap = &mut self.wrapped_messages;
            for msg in self.messages.iter() {
                wrap.append(&mut wrapped_msg(msg, &size))
            }

            self.message_offset = self.message_offset.min(wrap.len().saturating_sub(1));
        }
    }
}

pub struct Chats {
    client: Client,
    // Used for ChatGPT
    chats: Vec<Chat>,
    selected_chat: usize,
    pub writing: bool,
}

impl Chats {
    pub fn new() -> Self {
        Self {
            // TODO: improve this
            client: Client::new()
                .with_api_key("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"),
            chats: vec![Chat::default()],
            selected_chat: 0,
            writing: false,
        }
    }

    fn chat_mut(&mut self) -> &mut Chat {
        self.chats.get_mut(self.selected_chat).unwrap()
    }

    fn chat(&self) -> &Chat {
        self.chats.get(self.selected_chat).unwrap()
    }

    fn next_tab(&mut self) {
        self.selected_chat = (self.selected_chat + 1) % self.chats.len();
    }

    fn previous_tab(&mut self) {
        if self.selected_chat == 0 {
            self.selected_chat = self.chats.len() - 1;
        } else {
            self.selected_chat -= 1;
        }
    }

    pub fn add_chat(&mut self, name: String, system: Option<String>) {
        self.chats.push(Chat::new(name, system));
        self.selected_chat = self.chats.len() - 1;
    }

    fn remove_chat(&mut self) {
        self.chats.remove(self.selected_chat);
        if self.chats.is_empty() {
            self.chats.push(Chat::default());
        } else {
            self.selected_chat = self.selected_chat.saturating_sub(1);
        }
    }
}

impl Window for Chats {
    type InputReturn = bool;

    fn update_size(&mut self, size: Rect) {
        self.chat_mut().update_box(size);
    }

    fn draw<B: Backend>(&self, f: &mut Frame<B>) {
        // Divide screen
        let size = f.size();
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(2)
            .constraints(
                [
                    Constraint::Length(3),
                    // Make chat take up the remaining space
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
            .select(self.selected_chat)
            .style(Style::default())
            .highlight_style(
                Style::default()
                    .add_modifier(Modifier::BOLD)
                    .bg(Color::Black),
            );
        f.render_widget(tabs, chunks[0]);

        let chat = self.chat();

        // Display message history
        // TODO: improve
        let mut messages = vec![];
        if chat.messages.len() >= 1 {
            let start = chat
                .wrapped_messages
                .len()
                .saturating_sub(1 + chat.message_offset);
            let end = start.saturating_sub(chunks[1].height as usize);

            for i in (end..=start).rev() {
                let item = &chat.wrapped_messages[i];

                let spans = match item {
                    MessageContent::Sender(role) => {
                        let (c, color) = match role {
                            Role::User => ("User", Color::Green),
                            Role::System => ("System", Color::Yellow),
                            Role::Assistant => ("ChatGPT", Color::Blue),
                        };
                        vec![
                            Spans::from(vec![Span::styled(
                                format!("{:<9}", c),
                                Style::default().fg(color),
                            )]),
                            Spans::from(""),
                        ]
                    }
                    MessageContent::Line(line) => vec![Spans::from(line.clone())],
                    MessageContent::Divider => {
                        vec![Spans::from("-".repeat(chunks[1].width as usize))]
                    }
                };

                messages.push(ListItem::new(spans))
            }
        }

        let message_box = List::new(messages)
            .block(Block::default().borders(Borders::ALL).title("Messages"))
            .start_corner(Corner::BottomLeft);
        f.render_widget(message_box, chunks[1]);

        // Display input box
        let input = Paragraph::new(chat.prompt.str())
            .style(match self.writing {
                true => Style::default().fg(if chat.loading {
                    Color::Red
                } else {
                    Color::Blue
                }),
                _ => Style::default(),
            })
            .block(Block::default().borders(Borders::ALL).title("Input"));
        f.render_widget(input, chunks[2]);
        match self.writing {
            true => {
                f.set_cursor(
                    // Cursor goes past the
                    chunks[2].x + chat.prompt.cursor as u16 + 1,
                    // Move to where the text is
                    chunks[2].y + 1,
                )
            }
            _ => {}
        }
    }

    fn input(&mut self, key: &KeyEvent, clipboard: &mut Clipboard) -> Self::InputReturn {
        if !self.writing {
            match key.code {
                KeyCode::Char('q') => return true,
                KeyCode::Char('d') => self.remove_chat(),
                KeyCode::Right => self.next_tab(),
                KeyCode::Left => self.previous_tab(),
                KeyCode::Enter => self.writing = true,
                KeyCode::Up => self.chat_mut().scroll_up(),
                KeyCode::Down => self.chat_mut().scroll_down(),
                _ => {}
            }
        } else {
            match key.code {
                KeyCode::Esc => self.writing = false,
                KeyCode::Enter => {
                    if !self.chat().loading && !self.chat().prompt.is_empty() {
                        self.chat_mut().user();
                        self.writing = false;
                        let model = ChatModel::default()
                            .max_tokens(512u16)
                            .model("gpt-3.5-turbo")
                            .messages(self.chat().messages.clone())
                            .build()
                            .unwrap();

                        let rt = tokio::runtime::Builder::new_current_thread()
                            .enable_all()
                            .build()
                            .unwrap();
                        let res = rt.block_on(self.client.chat().create(model)).unwrap();

                        self.chat_mut()
                            .new_message(assistant_msg(res.choices[0].message.content.clone()));
                    }
                }
                _ => self.chat_mut().prompt.input(&key, clipboard),
            }
        }

        return false;
    }
}
