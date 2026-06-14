use crate::citadel::state::BackendState;
use crate::citadel::ui::dialogue_box::DialogueBox;
use crate::citadel::ui::ui_main::KeyResult::{AddScreen, Handled, ReplaceScreen};
use crate::citadel::ui::ui_main::{KeyResult, RenderWidget};
use crate::common::cmd::exec;
use crossterm::event::{KeyCode, KeyEvent};
use std::io::Stdout;
use tui::backend::CrosstermBackend;
use tui::layout::{Alignment, Constraint, Direction, Layout};
use tui::style::{Color, Style};
use tui::text::{Span, Spans, Text};
use tui::widgets::{Block, BorderType, Borders, List, ListItem, ListState, Paragraph, Wrap};
use tui::Frame;

pub struct RouteSetupScreen {
    current: usize,
    backend_state: BackendState,
    current_selected: Vec<usize>
}
impl RouteSetupScreen {
    pub fn new(backend_state: &mut BackendState) -> Self {
        Self { current: 0, backend_state: backend_state.clone(), current_selected: vec![] }
    }
    fn alternate_selected(&mut self) {
        if let Some(ind) = self.is_selected(self.current) {
            self.current_selected.remove(ind);
        } else {
            self.current_selected.push(self.current);
        }
    }
    fn is_selected(&self, index: usize) -> Option<usize> {
        self.current_selected.iter().position(|it| *it == index)
    }
    fn selected_style(&self, index: usize) -> Style {
        if self.is_selected(index).is_some() {
            Style::default().fg(Color::Blue)
        } else {
            Style::default().fg(Color::White)
        }
    }
    fn selected_text(&self, index: usize) -> Text {
        let style = self.selected_style(index);
        let it = &self.backend_state.known_generators[index];
        let desc = match &it.description {
            None => {""}
            Some(it) => {&format!(" - {}", it)}
        };
        Text::styled(format!("{}: {}{}", it.id, it.pub_ip, desc), style)
    }
}
impl RenderWidget for RouteSetupScreen {
    fn render(&mut self, rect: &mut Frame<CrosstermBackend<Stdout>>) {
        self.current += self.backend_state.known_generators.len();
        self.current %= self.backend_state.known_generators.len();

        let size = rect.size();
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(2)
            .constraints(
                [
                    Constraint::Percentage(50),
                    Constraint::Percentage(50),
                ]
                    .as_ref(),
            )
            .split(size);
        let items = (0..self.current_selected.len())
            .map(|id| self.selected_text(id))
            .map(ListItem::new).collect::<Vec<_>>();
        let picker = List::new(items)
            .style(Style::default().fg(Color::White))
            .block(
                Block::default()
                    .borders(Borders::RIGHT | Borders::TOP | Borders::LEFT)
                    .style(Style::default().fg(Color::White))
                    .title("Available Generators")
                    .border_type(BorderType::Plain),
            )
            .highlight_symbol("> ");

        let mut list_state = ListState::default();
        list_state.select(Some(self.current));

        rect.render_stateful_widget(picker, chunks[0], &mut list_state);

        let picked_list_text = self.current_selected.iter().map(|it|
            Spans::from(vec![Span::raw(format!("{}", self.backend_state.known_generators[*it].id))])
        ).collect::<Vec<_>>();
        let picked_list = Paragraph::new(picked_list_text)
            .style(Style::default().fg(Color::White).bg(Color::Black))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .style(Style::default().fg(Color::White))
                    .title("Selected Generators")
                    .border_type(BorderType::Plain),
            )
            .alignment(Alignment::Left)
            .wrap(Wrap { trim: false });
        rect.render_widget(picked_list, chunks[1]);
    }

    fn handle_input(&mut self, key_event: KeyEvent, state: &mut BackendState) -> KeyResult {
        match key_event.code {
            KeyCode::Down => {self.current += 1; Handled}
            KeyCode::Up => {self.current -= 1; Handled}
            KeyCode::Enter => {
                match state.create_wg_setup(self.current_selected.clone()) {
                    Ok(_) => {
                        let output = exec("ip route".into());
                        ReplaceScreen(Box::new(DialogueBox::new("Wireguard Setup Successful".to_string(), output)))
                    }
                    Err(it) => {
                        let output = format!("Error occurred: {}", it);
                        AddScreen(Box::new(DialogueBox::new("Wireguard Setup Failed".to_string(), output)))
                    }
                }
            }
            KeyCode::Char(' ') => {
                self.alternate_selected();
                Handled
            }
            KeyCode::Esc => KeyResult::Exited,
            _ => KeyResult::Passup(key_event),
        }
    }
}