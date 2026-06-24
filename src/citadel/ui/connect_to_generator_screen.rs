use crate::citadel::handshaker::Generator;
use crate::citadel::state::BackendState;
use crate::citadel::ui::dialogue_box::DialogueBox;
use crate::citadel::ui::ui_main::{KeyResult, RenderWidget};
use crate::common::ip::IpQuery;
use crossterm::event::{KeyCode, KeyEvent};
use std::io::Stdout;
use tui::backend::CrosstermBackend;
use tui::layout::{Alignment, Constraint, Direction, Layout};
use tui::style::{Color, Style};
use tui::text::{Span, Spans};
use tui::widgets::{Block, BorderType, Borders, Paragraph, Wrap};
use tui::Frame;
use crate::citadel::ui::cursor::Cursor;

pub struct ConnectToGeneratorScreen {
    entered_ip: String,
    cursor: Cursor<ConnectToGeneratorScreen>
}
impl ConnectToGeneratorScreen {
    pub fn new() -> ConnectToGeneratorScreen {
        ConnectToGeneratorScreen { entered_ip: "".to_string(), cursor: Cursor::new(|_| true) }
    }
}
impl RenderWidget for ConnectToGeneratorScreen {
    fn render(&mut self, rect: &mut Frame<CrosstermBackend<Stdout>>, _: &mut BackendState) {
        let size = rect.size();
        let surround = Block::default()
            .borders(Borders::ALL)
            .style(Style::default().fg(Color::White))
            .title("Connect To Generator")
            .border_type(BorderType::Plain);
        let inner_size = surround.inner(size);
        rect.render_widget(surround, size);

        let vertical_layout = Layout::default()
            .direction(Direction::Vertical)
            .margin(0)
            .constraints(
                [
                    Constraint::Min(1),
                    Constraint::Min(1)
                ]
                    .as_ref(),
            )
            .split(inner_size);

        let mut ip_text = vec![
            Spans::from(self.cursor.render(vec![
                Span::raw(&self.entered_ip)
            ], &self)),
            //add more lines if you want
        ];
        let entered_text = Paragraph::new(ip_text)
            .style(Style::default().fg(Color::White).bg(Color::Black))
            .alignment(Alignment::Left)
            .wrap(Wrap { trim: true });
        rect.render_widget(entered_text, vertical_layout[0]);

        let button = vec![Spans::from(vec![Span::raw("Connect")])];
        let button_text = Paragraph::new(button)
            .style(Style::default().fg(Color::White).bg(Color::Black))
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true });
        rect.render_widget(button_text, vertical_layout[1]);
    }

    fn handle_input(&mut self, key_event: KeyEvent, state: &mut BackendState) -> KeyResult {
        match key_event.code {
            KeyCode::Esc => KeyResult::Exited,
            KeyCode::Char(char) => {
                self.entered_ip.push(char);
                self.cursor.update_key();
                KeyResult::Handled
            },
            KeyCode::Enter => {
                let ip = self.entered_ip.clone();
                match Generator::connect_to_generator(ip.clone(), state) {
                    Ok(mut it) => {
                        let ip = IpQuery::query(&ip);
                        let desc = match &ip {
                            Ok(it) => {it.to_normal_name()}
                            Err(it) => {format!("Error fetching ip: {}", it)}
                        };
                        let info = format!("Connected to `{}` as {} successfully! Internal IP `{}`. Location: `{}`\nEndpoint: {}", self.entered_ip, it.id, it.internal_ip, desc, it.endpoints[0]);
                        it.description = if let Ok(it) = ip {Some(desc)} else {None};
                        state.known_generators.push(it);
                        state.save();
                        KeyResult::ReplaceScreen(Box::new(DialogueBox::new("Connection success", &info)))
                    }
                    Err(it) => {
                        let error = format!("failed connecting to server `{}`. Error: {}", self.entered_ip, it);
                        KeyResult::AddScreen(Box::new(DialogueBox::new("Connection failed", &error)))
                    }
                }
            },
            KeyCode::Backspace => {
                self.cursor.update_key();
                self.entered_ip.pop();
                KeyResult::Handled
            },
            _ => KeyResult::Passup(key_event),
        }
    }
}