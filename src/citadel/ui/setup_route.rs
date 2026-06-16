use std::cmp::PartialEq;
use crate::citadel::state::BackendState;
use crate::citadel::ui::dialogue_box::DialogueBox;
use crate::citadel::ui::ui_main::KeyResult::{AddScreen, Handled, ReplaceScreen};
use crate::citadel::ui::ui_main::{KeyResult, RenderWidget};
use crate::common::cmd::exec;
use crossterm::event::{KeyCode, KeyEvent};
use std::io::{Stdout};
use tui::backend::CrosstermBackend;
use tui::layout::{Alignment, Constraint, Direction, Layout};
use tui::style::{Color, Style};
use tui::text::{Span, Spans, Text};
use tui::widgets::{Block, BorderType, Borders, List, ListItem, ListState, Paragraph, Wrap};
use tui::Frame;
use crate::citadel::ui::cursor::Cursor;

pub struct RouteSetupScreen {
    current: usize,
    current_selected: Vec<usize>,
    mode: Mode,
    settings_current: usize,
    target_address: String,
    target_addr_cursor: Cursor<RouteSetupScreen>
}
#[derive(Eq, PartialEq)]
enum Mode {
    RoutePicker,
    Settings
}
impl RouteSetupScreen {
    pub fn new(state: &mut BackendState) -> Self {
        let current_selected = state.current_wg_ids.iter().map(|it|
            state.known_generators.iter().position(|g| g.id.eq(it)).expect(&format!("could not find generator for id {}", it))
        ).collect();
        let target_addr_cursor = Cursor::new(|it: &RouteSetupScreen| it.mode == Mode::Settings && it.settings_current == 0);
        Self { current: 0, current_selected, mode: Mode::RoutePicker, settings_current: 0, target_address: "0.0.0.0/0".to_string(), target_addr_cursor }
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
    fn selected_text(&self, state: &mut BackendState, index: usize) -> Text {
        let style = self.selected_style(index);
        let it = &state.known_generators[index];
        let desc = match &it.description {
            None => {""}
            Some(it) => {&format!(" - {}", it)}
        };
        Text::styled(format!("{}: {}{}", it.id, it.pub_ip, desc), style)
    }
    fn increment_selected(&mut self, increment: bool) {
        match self.mode {
            Mode::RoutePicker => {if increment {self.current += 1;} else {self.current -= 1;}}
            Mode::Settings => {if increment {self.settings_current += 1;} else {self.settings_current -= 1;}}
        }
    }
}


impl RenderWidget for RouteSetupScreen {
    fn render(&mut self, rect: &mut Frame<CrosstermBackend<Stdout>>, state: &mut BackendState) {
        self.current += state.known_generators.len();
        self.current %= state.known_generators.len();
        self.settings_current += 1;//will add more later, probably.
        self.settings_current %= 1;

        let size = rect.size();
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(
                [
                    Constraint::Percentage(50),
                    Constraint::Percentage(50),
                ]
                    .as_ref(),
            )
            .split(size);
        let horizontal_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(
                [
                    Constraint::Percentage(50),
                    Constraint::Percentage(50),
                ]
                    .as_ref(),
            )
            .split(chunks[1]);

        let items = (0..state.known_generators.len())
            .map(|id| self.selected_text(state, id))
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
            Spans::from(vec![Span::raw(format!("{}", state.known_generators[*it].id))])
        ).collect::<Vec<_>>();
        let picked_list = Paragraph::new(picked_list_text)
            .style(Style::default().fg(Color::White).bg(Color::Black))
            .block(
                Block::default()
                    .borders(Borders::ALL - Borders::RIGHT)
                    .style(Style::default().fg(Color::White))
                    .title("Selected Generators")
                    .border_type(BorderType::Plain),
            )
            .alignment(Alignment::Left)
            .wrap(Wrap { trim: false });
        rect.render_widget(picked_list, horizontal_chunks[0]);

        let block = Block::default()
                .borders(Borders::ALL)
                .style(Style::default().fg(Color::White))
                .title("Settings")
                .border_type(BorderType::Plain);
        let settings_size = block.inner(horizontal_chunks[1]);
        rect.render_widget(block, horizontal_chunks[1]);


        let settings_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(&[Constraint::Min(1), /*Constraint::Min(1)*/])
            .split(settings_size);

        let fg = if self.mode == Mode::Settings && self.settings_current == 0 { Color::White } else { Color::Gray };
        let addr_text = Spans::from(self.target_addr_cursor.render(vec![Span::raw(self.target_address.clone())], &self));
        let target_addr_box = Paragraph::new(addr_text)
            .style(Style::default().fg(fg).bg(Color::Black))
            .alignment(Alignment::Left)
            .wrap(Wrap { trim: false });
        rect.render_widget(target_addr_box, settings_chunks[0]);
    }
    fn handle_input(&mut self, key_event: KeyEvent, state: &mut BackendState) -> KeyResult {
        match key_event.code {
            KeyCode::Down => { self.increment_selected(true); Handled}
            KeyCode::Up => { self.increment_selected(false); Handled}
            KeyCode::Enter => {
                if self.mode == Mode::RoutePicker {
                    self.mode = Mode::Settings;
                    return Handled;
                }
                match state.create_wg_setup(self.current_selected.clone(), self.target_address.to_string()) {
                    Ok(_) => {
                        let out = exec("ip route".into());
                        let output = format!("`ip route` responded with {} lines:\n{}", out.len(), out);
                        ReplaceScreen(Box::new(DialogueBox::new("Wireguard Setup Successful", &output)))
                    }
                    Err(it) => {
                        let output = format!("Error occurred: {}", it);
                        AddScreen(Box::new(DialogueBox::new("Wireguard Setup Failed", &output)))
                    }
                }
            }
            KeyCode::Char(it) => {
                match self.mode {
                    Mode::RoutePicker => {
                        if it == ' ' {
                            self.alternate_selected();
                        }
                    }
                    Mode::Settings => {
                        if self.settings_current == 0 {
                            self.target_address.push(it);
                            self.target_addr_cursor.update_key();
                        }
                    }
                }
                Handled
            }
            KeyCode::Backspace => {
                if self.mode == Mode::Settings && self.settings_current == 0 {
                    self.target_address.pop();
                }
                Handled
            }
            KeyCode::Esc => KeyResult::Exited,
            _ => KeyResult::Passup(key_event),
        }
    }
}