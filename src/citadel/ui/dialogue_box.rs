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
use crate::citadel::ui::ui_main::KeyResult::Handled;

pub struct DialogueBox {
    title: String,
    message: String,
    light: bool,
    scroll: usize
}
impl DialogueBox {
    pub fn new(title: &str, message: &str) -> DialogueBox {
        DialogueBox { title: title.to_string(), message: message.to_string(), light: false, scroll: 0 }
    }
}
impl RenderWidget for DialogueBox {
    fn render(&mut self, rect: &mut Frame<CrosstermBackend<Stdout>>, _: &mut BackendState) {
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

        let error_txt = self.message.split("\n").into_iter()
            .map(|it| it.trim())
            .filter(|it| !it.is_empty())
            .map(|line| Spans::from(vec![Span::raw(line)]))
            .collect::<Vec<_>>();
        let entered_text = Paragraph::new(error_txt[self.scroll.min(error_txt.len() - 1)..error_txt.len()].to_vec())
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
            KeyCode::Enter | KeyCode::Esc => {
                KeyResult::Exited
            }
            KeyCode::Up => {
                self.scroll = self.scroll.saturating_sub(1);
                Handled
            }
            KeyCode::Down => {
                self.scroll = self.scroll + 1;
                Handled
            }
            _ => {KeyResult::Passup(key_event)}
        }
    }
}