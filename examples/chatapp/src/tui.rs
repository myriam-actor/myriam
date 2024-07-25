use color_eyre::Result;
use myriam::{actors::remote::address::ActorAddress, messaging::Message};
use ratatui::{
    crossterm::{
        event::{self, KeyCode, KeyEventKind, KeyModifiers},
        execute,
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    },
    layout::{Constraint, Layout},
    prelude::CrosstermBackend,
    style::{Style, Stylize},
    text::{Line, Span},
    widgets::{Block, List, ListItem, ListState, Paragraph},
    Frame, Terminal,
};
use std::{
    io::{stdout, Stdout},
    time::Duration,
};
use tokio::sync::mpsc::Receiver;

use crate::{
    messaging::{MessengerCmd, MessengerHandle},
    models::{Command, Report, ReportKind},
};

pub type Term = Terminal<CrosstermBackend<Stdout>>;

pub fn init() -> Result<Term> {
    execute!(stdout(), EnterAlternateScreen)?;
    enable_raw_mode()?;

    Ok(Terminal::new(CrosstermBackend::new(stdout()))?)
}

pub fn restore() -> Result<()> {
    execute!(stdout(), LeaveAlternateScreen)?;
    disable_raw_mode()?;
    Ok(())
}

#[derive(Debug)]
pub struct App {
    address: ActorAddress,
    messenger: MessengerHandle,
    receiver: Receiver<Report>,
    input_buffer: Vec<char>,
    input_index: usize,
    messages: Vec<Report>,
    exit: bool,
}

impl App {
    pub fn new(
        address: ActorAddress,
        messenger: MessengerHandle,
        receiver: Receiver<Report>,
    ) -> Self {
        Self {
            address,
            messenger,
            receiver,
            input_buffer: vec![],
            input_index: 0,
            messages: vec![],
            exit: false,
        }
    }

    pub fn run(&mut self, terminal: &mut Term) -> Result<()> {
        self.messages.push(Report::info(
            "enter !add <address> to connect to peer, !q to quit".into(),
        ));

        while !self.exit {
            terminal.draw(|frame| self.render_frame(frame))?;
            self.handle_events()?;
            self.handle_reports()?;
        }
        Ok(())
    }

    fn render_frame(&self, frame: &mut Frame) {
        let vertical = Layout::vertical([
            Constraint::Min(1),
            Constraint::Length(3),
            Constraint::Length(1),
        ]);
        let [msgs_area, input_area, address_area] = vertical.areas(frame.size());

        // message window

        let messages: Vec<ListItem> = self
            .messages
            .iter()
            .map(|m| {
                let style = match m.kind() {
                    ReportKind::Echo => Style::default(),
                    ReportKind::Peer => Style::new().blue(),
                    ReportKind::Info => Style::new().green(),
                    ReportKind::Error => Style::new().red(),
                };

                ListItem::new(Line::from(vec![
                    Span::styled(m.marker(), Style::default()),
                    Span::styled(m.body(), style),
                ]))
            })
            .collect();

        let messages = List::new(messages).block(Block::bordered().title("messages"));

        let count = self.messages.len();
        let height = msgs_area.height as usize - 2;
        let offset = if count <= height { 0 } else { count - height };
        let mut msgs_state = ListState::default().with_offset(offset);
        frame.render_stateful_widget(messages, msgs_area, &mut msgs_state);

        // input widget + offset

        let count = self.input_buffer.len();
        let width = input_area.width as usize - 2;
        let offset = if count <= width { 0 } else { count - width };

        let input = Paragraph::new(self.buffer_to_string())
            .block(Block::bordered().title("input"))
            .scroll((0, offset as u16));

        frame.render_widget(input, input_area);
        frame.set_cursor(input_area.x + self.input_index as u16 + 1, input_area.y + 1);

        // address display

        let address = Paragraph::new(self.address.to_string()).centered();
        frame.render_widget(address, address_area);
    }

    fn handle_reports(&mut self) -> Result<()> {
        if let Ok(rep) = self.receiver.try_recv() {
            self.messages.push(rep);
        }

        Ok(())
    }

    fn handle_events(&mut self) -> Result<()> {
        if event::poll(Duration::from_millis(16))? {
            if let event::Event::Key(key) = event::read()? {
                // quit action
                if key.kind == KeyEventKind::Press
                    && key.code == KeyCode::Char('q')
                    && key.modifiers.contains(KeyModifiers::CONTROL)
                {
                    self.exit = true;
                }

                // text input
                if let KeyCode::Char(ch) = key.code {
                    if key.kind == KeyEventKind::Press {
                        self.text_input(ch);
                    }
                }

                // deletion
                if key.kind == KeyEventKind::Press && key.code == KeyCode::Backspace {
                    self.backspace();
                }

                // cursor movement
                if key.kind == KeyEventKind::Press && key.code == KeyCode::Left {
                    self.cursor_move(-1);
                }

                if key.kind == KeyEventKind::Press && key.code == KeyCode::Right {
                    self.cursor_move(1);
                }

                // cmd send
                if key.kind == KeyEventKind::Press && key.code == KeyCode::Enter {
                    if !self.input_buffer.is_empty() {
                        let msg = self.buffer_flush();
                        match msg.parse::<Command>() {
                            Ok(cmd) => self.handle_command(cmd),
                            Err(err) => self.messages.push(Report::error(err.to_string())),
                        }
                    }
                }
            }
        }

        Ok(())
    }

    fn buffer_flush(&mut self) -> String {
        let msg = self.buffer_to_string();
        self.input_buffer.clear();
        self.input_index = 0;

        msg
    }

    fn handle_command(&mut self, cmd: Command) {
        match cmd {
            Command::Msg(body) => {
                self.messages.push(Report::echo(body.clone()));
                let messenger = self.messenger.clone();
                std::thread::spawn(move || {
                    let _ = messenger.blocking_send(Message::Task(MessengerCmd::Outgoing(body)));
                });
            }
            Command::Hello(addr) => {
                let messenger = self.messenger.clone();
                std::thread::spawn(move || {
                    let _ = messenger.blocking_send(Message::TaskMut(MessengerCmd::Register(addr)));
                });
            }
            Command::Quit => {
                self.exit = true;
            }
        }
    }

    fn text_input(&mut self, ch: char) {
        self.input_buffer.insert(self.input_index, ch);
        self.input_index += 1;
    }

    fn backspace(&mut self) {
        if !self.input_buffer.is_empty() {
            self.input_buffer.remove(self.input_index - 1);
            self.input_index = if self.input_index == 0 {
                0
            } else {
                self.input_index - 1
            };
        }
    }

    fn cursor_move(&mut self, n: isize) {
        self.input_index =
            (self.input_index as isize + n).clamp(0, self.input_buffer.len() as isize) as usize;
    }

    fn buffer_to_string(&self) -> String {
        self.input_buffer.iter().collect()
    }
}
