use std::io::Stdout;
use std::net::SocketAddr;
use crossterm::event::{KeyCode, KeyEvent};
use crossterm::event::KeyCode::Up;
use tui::backend::CrosstermBackend;
use tui::Frame;
use tui::layout::{Constraint, Direction, Layout};
use tui::style::{Color, Style};
use tui::text::{Span, Spans, Text};
use tui::widgets::{Block, BorderType, Borders, List, ListItem, ListState, Paragraph};
use crate::citadel::control_connection::ControlConnection;
use crate::citadel::handshaker::{Endpoint, Generator};
use crate::citadel::state::BackendState;
use crate::citadel::ui::cursor::Cursor;
use crate::citadel::ui::dialogue_box::DialogueBox;
use crate::citadel::ui::generator_control_screen::GeneratorControlScreen;
use crate::citadel::ui::ui_main::{KeyResult, RenderWidget};
use crate::citadel::ui::ui_main::KeyResult::{AddScreen, Exited, Handled, Passup};
use crate::common::errors::FFResult;
use crate::common::wireguard::{get_routes, Route};

pub struct ControlConnectionScreen {
    is_on_preroutes: bool,
    preroute_selected: usize,
    direct_addr: String,
    direct_addr_cursor: Cursor<Self>,
    routes: Vec<Route>
}
impl ControlConnectionScreen {
    pub fn new() -> ControlConnectionScreen {
        ControlConnectionScreen {
            is_on_preroutes: true,
            preroute_selected: 0,
            direct_addr: "".to_string(),
            direct_addr_cursor: Cursor::new(|it: &Self| !it.is_on_preroutes).set_deactive_full_text(),
            routes: get_routes()
        }
    }
    fn is_device_available(state: &BackendState, generator: &Generator) -> Option<SocketAddr> {
        let ip = generator.internal_ip;
        let address = SocketAddr::new(ip, generator.config_port);
        if let Some(it) = &state.current_wg_setup {
            for route in &it.routes {
                if let Some(it) = route.via && it.eq(&ip) {
                    return Some(address)
                }
            }
            None
        } else {None}
    }
    fn get_available_device_indexes(state: &BackendState) -> Vec<usize> {
        let mut list = vec![];
        for i in 0..state.known_generators.len() {
            if Self::is_device_available(state, &state.known_generators[i]).is_some() {
                list.push(i);
            }
        }
        list
    }
}
impl RenderWidget for ControlConnectionScreen {
    fn render(&mut self, rect: &mut Frame<CrosstermBackend<Stdout>>, state: &mut BackendState) {
        let available_device_indexes = Self::get_available_device_indexes(state);

        let vertical_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(
                &[Constraint::Min((2 + state.known_generators.len()) as u16), Constraint::Percentage(50)]
            )
            .split(rect.size());

        let items: Vec<_> = state.known_generators.iter().map(|ge| {
            let style = if let Some(it) = Self::is_device_available(state, ge) {
                Style::default().fg(Color::Blue)
            } else {Style::default().fg(Color::Gray)};
            let desc = ge.description.clone().map(|it| format!(" - {}", it)).unwrap_or("".to_string());
            ListItem::new(Text::styled(format!("{}: {}{}", ge.id, ge.internal_ip, desc), style))
        }).collect();
        let generators = List::new(items)
            .style(Style::default().fg(Color::White))
            .block(
                Block::default()
                    .borders(Borders::ALL - Borders::BOTTOM)
                    .style(Style::default().fg(Color::White))
                    .title("Select From Connected Peers")
                    .border_type(BorderType::Plain),
            )
            .highlight_symbol("> ");
        if /* !available_device_indexes.is_empty() && */ self.is_on_preroutes {
            let mut list_state = ListState::default();
            list_state.select(Some(/*available_device_indexes[*/self.preroute_selected/*]*/));
            rect.render_stateful_widget(generators, vertical_layout[0], &mut list_state);
        } else {
            rect.render_widget(generators, vertical_layout[0]);
        }

        let text = Spans::from(self.direct_addr_cursor.render(vec![Span::raw(&self.direct_addr)], &self));
        let ip_entry = Paragraph::new(text)
            .block(Block::default().borders(Borders::ALL).title("Direct Connection").border_type(BorderType::Plain));
        rect.render_widget(ip_entry, vertical_layout[1])
    }

    fn handle_input(&mut self, key_event: KeyEvent, state: &mut BackendState) -> KeyResult {
        match key_event.code {
            KeyCode::Tab => {
                self.is_on_preroutes = !self.is_on_preroutes;
                Handled
            },
            Up | KeyCode::Down => {
                //let available_device_indexes = Self::get_available_device_indexes(state);
                //let len = available_device_indexes.len();
                let len = state.known_generators.len();
                if !self.is_on_preroutes /*|| available_device_indexes.is_empty()*/ {
                    return Handled
                }
                self.preroute_selected = (self.preroute_selected + if key_event.code == Up {len - 1} else {len + 1}) % len;
                Handled
            },
            KeyCode::Char(it) => {
                if !self.is_on_preroutes {
                    self.direct_addr.push(it);
                    self.direct_addr_cursor.update_key();
                }
                Handled
            },
            KeyCode::Enter => {
                let addr = if self.is_on_preroutes {
                    //let available_device_indexes = Self::get_available_device_indexes(state);
                    //if available_device_indexes.is_empty() {
                    //    return AddScreen(Box::new(DialogueBox::new("Error", "No devices directly connected by Wireguard")))
                    //}
                    let ge = &state.known_generators[/*available_device_indexes[*/self.preroute_selected/*]*/];
                    SocketAddr::new(ge.internal_ip, ge.config_port)
                } else {
                    if let Ok(it) = self.direct_addr.parse() {
                        let end = Endpoint::from_initial_ip(it, state.known_generators.last());
                        let gene: &mut Generator = &mut state.known_generators[self.preroute_selected];
                        gene.endpoints.push(end);
                        it
                    } else {
                        return AddScreen(Box::new(DialogueBox::new("Error", "Invalid Ip/Port Entered")))
                    }
                };
                if !Self::get_available_device_indexes(state).contains(&self.preroute_selected) && self.is_on_preroutes {
                    let ge = &state.known_generators[self.preroute_selected];
                    return AddScreen(Box::new(GeneratorControlScreen::new(ge.id.clone(), None, state)))
                }
                match ControlConnection::connect(addr.clone(), &state) {
                    Ok(it) => {
                        AddScreen(Box::new(GeneratorControlScreen::new(it.server_id.clone(), Some(it), state)))
                    }
                    Err(it) => {
                        AddScreen(Box::new(DialogueBox::new("Error", &format!("could not connect to device - `{}`", it))))
                    }
                }
            }
            KeyCode::Esc => {
                Exited
            },
            _ => Passup(key_event),
        }
    }
}