use crate::prompt::{Prompt, ScrollPrompt, StaticPrompt};
use crate::Window;
use arboard::Clipboard;
use async_openai::error::{ApiError, OpenAIError};
use async_openai::types::{
    ChatCompletionResponseStream, CreateChatCompletionRequest, CreateChatCompletionStreamResponse,
};
use async_openai::{
    types::{
        ChatCompletionRequestMessage as Message, CreateChatCompletionRequestArgs as ChatModel, Role,
    },
    Client,
};
use crossterm::event::{KeyCode, KeyEvent};
use futures::StreamExt;
use std::borrow::Cow;
use std::io::{stdout, Write};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;
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
    prompt: ScrollPrompt,
    // Used to lock the prompt when the API is loading a response
    loading: bool,

    // Will be some when there is something to be read
    reader: Option<Receiver<Result<Option<String>, OpenAIError>>>,
    // Contains the AI's buffer
    answer_buffer: String,
}

// TODO: replace string with a Result<String, Error>
fn stream_answer(
    client: Client,
    tx: Sender<Result<Option<String>, OpenAIError>>,
    messages: Vec<Message>,
) {
    // TODO: if any of these two error out then return that also
    // TODO: when error is detected create a pop-up that explains what happened

    // TODO: improve this
    match ChatModel::default()
        .max_tokens(500u16)
        .model("gpt-3.5-turbo")
        .messages(messages)
        .build()
    {
        Ok(model) => {
            match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(rt) => {
                    rt.block_on(async {
                        match client.chat().create_stream(model).await {
                            Ok(mut stream) => {
                                while let Some(result) = stream.next().await {
                                    match result {
                                        Ok(res) => {
                                            for c in res.choices.iter() {
                                                if let Some(ref content) = c.delta.content {
                                                    tx.send(Ok(Some(content.to_string()))).unwrap();
                                                }
                                            }
                                        }
                                        Err(err) => tx.send(Err(err)).unwrap(),
                                    };
                                }
                                // Notify stream is over
                                tx.send(Ok(None)).unwrap();
                            }
                            Err(err) => tx.send(Err(err)).unwrap(),
                        }
                    });
                }
                Err(err) => {}
            }
        }
        Err(err) => tx.send(Err(err)).unwrap(),
    }
}

impl Default for Chat {
    fn default() -> Self {
        Chat::new("New Chat".to_string(), None)
    }
}

fn wrapped_text(text: &String, size: u16) -> Vec<Cow<str>> {
    textwrap::wrap(&text, (size - 6) as usize)
}

fn wrapped_msg(msg: &Message, size: &Rect) -> Vec<MessageContent> {
    let mut msgs = vec![];

    msgs.append(&mut vec![
        MessageContent::Divider,
        MessageContent::Sender(msg.role.clone()),
    ]);

    for line in wrapped_text(&msg.content, size.width) {
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
            prompt: ScrollPrompt::new(1),
            loading: false,
            reader: None,
            answer_buffer: "".to_string(),
        }
    }

    fn new_message(&mut self, message: Message) {
        self.wrapped_messages
            .append(&mut wrapped_msg(&message, &self.last_size));
        self.messages.push(message);
        self.message_offset = 0;
    }

    fn update_last(&mut self, message: String) {
        // Pop the last message cause its easier to work with that way
        let last = self.wrapped_messages.pop().unwrap();
        self.messages.last_mut().unwrap().content += &message;
        match last {
            MessageContent::Line(mut line) => {
                line += &message;
                let wrap = wrapped_text(&line, self.last_size.width);

                for s in wrap {
                    self.wrapped_messages
                        .push(MessageContent::Line(s.to_string()));
                }
            }
            _ => {
                self.wrapped_messages.push(last);
                self.wrapped_messages.push(MessageContent::Line(message));
            }
        }
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

            let mut wrap = vec![];
            for msg in self.messages.iter() {
                wrap.append(&mut wrapped_msg(msg, &size))
            }

            self.message_offset = self.message_offset.min(wrap.len().saturating_sub(1));
            self.wrapped_messages = wrap;
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

    fn update<B: Backend>(&mut self, f: &mut Frame<B>) {
        // Update all chats
        for chat in self.chats.iter_mut() {
            chat.prompt.update_size(f.size().width - 8);
            if let Some(reader) = chat.reader.as_mut() {
                if let Ok(res) = reader.recv() {
                    let res = res.unwrap();
                    if let Some(msg) = res {
                        chat.update_last(msg);
                    } else {
                        // None means its over
                        chat.reader = None;
                    }
                }
            }
        }

        self.update_size(f.size());
        self.draw(f);
    }

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
                KeyCode::Enter => {
                    if !self.chat().reader.is_some() {
                        self.writing = true
                    }
                }
                KeyCode::Up => self.chat_mut().scroll_up(),
                KeyCode::Down => self.chat_mut().scroll_down(),
                _ => {}
            }
        } else {
            match key.code {
                KeyCode::Esc => self.writing = false,
                KeyCode::Enter => {
                    if !self.chat().loading && !self.chat().prompt.is_empty() {
                        let client = self.client.clone();
                        let chat = self.chat_mut();
                        chat.user();
                        let (send, recv) = channel();
                        chat.reader = Some(recv);
                        chat.new_message(assistant_msg("".to_string()));

                        let messages = chat.messages.clone();
                        thread::spawn(move || {
                            stream_answer(client, send, messages);
                        });

                        self.writing = false;
                    }
                }
                _ => self.chat_mut().prompt.input(&key, clipboard),
            }
        }

        return false;
    }
}
