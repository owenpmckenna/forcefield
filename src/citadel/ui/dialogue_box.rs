use std::io::Stdout;
use crossterm::event::{KeyCode, KeyEvent};
use tui::backend::CrosstermBackend;
use tui::Frame;
use tui::layout::{Alignment, Constraint, Direction, Layout};
use tui::style::{Color, Style};
use tui::text::{Span, Spans};
use tui::widgets::{Block, BorderType, Borders, Paragraph, Wrap};
use crate::citadel::state::BackendState;
use crate::citadel::ui::ui_main::{KeyResult, RenderWidget};

pub struct DialogueBox {
    title: String,
    message: String,
    light: bool
}
impl DialogueBox {
    pub fn new(title: String, message: String) -> DialogueBox {
        DialogueBox { title, message, light: false }
    }
}
impl RenderWidget for DialogueBox {
    fn render(&mut self, rect: &mut Frame<CrosstermBackend<Stdout>>) {
        let surround = Block::default()
            .borders(Borders::ALL)
            .style(Style::default().fg(Color::White))
            .title(self.title.clone())
            .border_type(BorderType::Plain);
        let inner_size = surround.inner(rect.size());
        rect.render_widget(surround, rect.size());

        let vertical_layout = Layout::default()
            .direction(Direction::Vertical)
            .margin(0)
            .constraints(
                [
                    Constraint::Percentage(99),
                    Constraint::Min(1)
                ]
                    .as_ref(),
            )
            .split(inner_size);

        let error_txt = vec![
            Spans::from(vec![
                Span::raw(&self.message),
            ]),
            //add more lines if you want
        ];
        let entered_text = Paragraph::new(error_txt)
            .style(Style::default().fg(Color::White).bg(Color::Black))
            .alignment(Alignment::Left)
            .wrap(Wrap { trim: false });
        rect.render_widget(entered_text, vertical_layout[0]);

        let color = if self.light {Color::Gray} else {Color::Black};
        let error_txt = vec![
            Spans::from(vec![
                Span::styled("Continue", Style::default().bg(color).fg(Color::White)),
            ]),
        ];
        let cont = Paragraph::new(error_txt)
            .style(Style::default().fg(Color::White).bg(Color::Black))
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true });
        rect.render_widget(cont, vertical_layout[1]);

        self.light = !self.light;
    }

    fn handle_input(&mut self, key_event: KeyEvent, _: &mut BackendState) -> KeyResult {
        match key_event.code {
            KeyCode::Enter => {
                KeyResult::Exited
            }
            _ => {KeyResult::Passup(key_event)}
        }
    }
}