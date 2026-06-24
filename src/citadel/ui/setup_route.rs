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
use crate::citadel::handshaker::Endpoint;
use crate::citadel::ui::cursor::Cursor;
use crate::common::wireguard::{get_routes, Route};

pub struct RouteSetupScreen {
    current: usize,
    current_selected: Vec<(usize, Endpoint)>,
    mode: Mode,
    settings_current: usize,
    target_address: String,
    target_addr_cursor: Cursor<RouteSetupScreen>,
    routes: Vec<Route>
}
#[derive(Eq, PartialEq)]
enum Mode {
    RoutePicker,
    Settings
}
impl RouteSetupScreen {
    pub fn new(state: &mut BackendState) -> Self {
        let current_selected: Vec<(usize, Endpoint)> = state.current_wg_ids.iter().enumerate().map(|(index, it)|
            (state.known_generators.iter().position(|g| g.id.eq(it)).unwrap(), state.endpoints_used[index].clone())
        ).collect();
        let target_addr_cursor = Cursor::new(|it: &RouteSetupScreen| it.mode == Mode::Settings && it.settings_current == 0);
        let routes = get_routes();
        let mut se = Self { current: 0, current_selected, mode: Mode::RoutePicker, settings_current: 0, target_address: "0.0.0.0/0".to_string(), target_addr_cursor, routes };
        se.current = if let Some((id, _)) = se.current_selected.last() {
            *id
        } else {*se.get_allowed_ids(state).get(0).unwrap_or(&0)};
        se
    }
    fn current_selected_to_ids<'a>(&self, state: &'a BackendState) -> Vec<&'a String> {
        self.current_selected.iter().map(|it| &state.known_generators[it.0].id).collect()
    }
    fn get_allowed_ids(&self, state: &BackendState) -> Vec<usize> {
        let mut data = Vec::with_capacity(state.known_generators.len());
        for i in 0..state.known_generators.len() {
            if let Some((id, _)) = self.current_selected.last() && *id == i {
                data.push(i);
                continue
            }
            if self.current_selected.iter().position(|it| it.0 == i).is_some() {
                continue;
            }
            let last_id = self.current_selected.last().map(|it| state.known_generators[it.0].id.clone());
            let available_routes = last_id.as_ref().map(|it| state.get_by_id(&it)).flatten()
                .map(|it| it.probable_routes.lock().ok()).flatten();
            if state.known_generators[i].find_best_endpoint(&self.routes, Err(last_id)).is_none() {
                continue;
            }
            data.push(i);
        }
        data
    }
    fn alternate_selected(&mut self, state: &BackendState) {
        if let Some(ind) = self.is_selected(self.current) {
            self.current_selected.remove(ind);
        } else {
            let last_id = self.current_selected.last().map(|it| state.known_generators[it.0].id.clone());
            let best = state.known_generators[self.current].find_best_endpoint(&self.routes, Err(last_id));
            self.current_selected.push((self.current, best.unwrap().clone()));
        }
    }
    fn is_selected(&self, index: usize) -> Option<usize> {
        self.current_selected.iter().position(|(i, _)| *i == index)
    }
    fn selected_style(&self, index: usize, has_route: bool) -> Style {
        if self.is_selected(index).is_some() {
            Style::default().fg(Color::Blue)
        } else if has_route {
            Style::default().fg(Color::White)
        } else {
            Style::default().fg(Color::DarkGray)
        }
    }
    fn selected_text(&self, state: &mut BackendState, index: usize) -> Text {
        let last_id = self.current_selected.last().map(|it| state.known_generators[it.0].id.clone());
        let best = state.known_generators[index].find_best_endpoint(&self.routes, Err(last_id));
        let style = self.selected_style(index, best.is_some());
        let it = &state.known_generators[index];
        Text::styled(it.get_generator_text(&best), style)
    }
    fn safe_inc_selected(&mut self, increment: bool, gens: usize) {
        if increment {self.current += 1;} else {self.current -= 1;}
        self.current += gens;
        self.current %= gens;
    }
    fn increment_selected(&mut self, increment: bool, state: &BackendState) {
        match self.mode {
            Mode::RoutePicker => {
                let allowed = self.get_allowed_ids(state);
                self.safe_inc_selected(increment, state.known_generators.len());
                while !allowed.contains(&self.current) {
                    self.safe_inc_selected(increment, state.known_generators.len());
                }
            }
            Mode::Settings => {if increment {self.settings_current += 1;} else {self.settings_current -= 1;}}
        }
    }
}


impl RenderWidget for RouteSetupScreen {
    fn render(&mut self, rect: &mut Frame<CrosstermBackend<Stdout>>, state: &mut BackendState) {
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
            Spans::from(vec![Span::raw(format!("{} - {}", state.known_generators[it.0].id, it.1))])
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
            KeyCode::Down => { self.increment_selected(true, state); Handled}
            KeyCode::Up => { self.increment_selected(false, state); Handled}
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
                            self.alternate_selected(state);
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