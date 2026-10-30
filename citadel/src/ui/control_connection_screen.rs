use crate::control_connection::ControlConnection;
use crate::handshaker::{Endpoint, Generator, Peer, PeerData};
use crate::state::BackendState;
use crate::ui::dialogue_box::DialogueBox;
use crate::ui::generator_control_screen_2::GeneratorControlScreen2;
use crate::ui::ui_main::KeyResult::{AddScreen, Exited, Handled, Passup};
use crate::ui::ui_main::{KeyResult, RenderWidget};
use crate::ui_utils::cursor::Cursor;
use common::wireguard::{Route, get_routes};
use crossterm::event::KeyCode::Up;
use crossterm::event::{KeyCode, KeyEvent};
use std::io::Stdout;
use std::net::{IpAddr, SocketAddr};
use tui::Frame;
use tui::backend::CrosstermBackend;
use tui::layout::{Constraint, Direction, Layout};
use tui::style::{Color, Style};
use tui::text::{Span, Spans, Text};
use tui::widgets::{Block, BorderType, Borders, List, ListItem, ListState, Paragraph};

pub struct ControlConnectionScreen {
    preroute_selected: usize,
    routes: Vec<Route>
}
impl ControlConnectionScreen {
    pub fn new() -> ControlConnectionScreen {
        ControlConnectionScreen {
            preroute_selected: 0,
            routes: get_routes()
        }
    }
    fn is_device_available(state: &BackendState, generator: &Peer) -> Option<SocketAddr> {
        let generator = if let Peer::Ge(ge) = generator {
            ge
        } else {return None;};
        let ip = generator.internal_ip_v4.into();
        let address = SocketAddr::new(ip, generator.config_port);
        if let Some(it) = &state.current_wg_setup {
            for route in &it.routes {
                if let Some(it) = route.via && it.eq(&ip) {
                    return Some(address)
                }
            }
            if state.current_wg_ids.contains(&generator.id) {
                return Some(address)
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

    fn press_enter(&mut self, state: &mut BackendState) -> KeyResult {
        if !Self::get_available_device_indexes(state).contains(&self.preroute_selected) {
            let ge = &state.known_generators[self.preroute_selected];
            return AddScreen(Box::new(GeneratorControlScreen2::new(ge.get_id().clone(), None, state)))
        }
        let addr = if let Peer::Ge(ge) = &state.known_generators[self.preroute_selected] {
            SocketAddr::new(IpAddr::V4(ge.internal_ip_v4), ge.config_port)
        } else {return Handled};//shouldn't get here, get_available_device_indexes doesn't return Servers
        match ControlConnection::connect(addr, state) {
            Ok(it) => {
                AddScreen(Box::new(GeneratorControlScreen2::new(it.server_id.clone(), Some(it), state)))
            }
            Err(it) => {
                AddScreen(Box::new(DialogueBox::new("Error", &format!("could not connect to device - `{}`", it))))
            }
        }
    }
}
impl RenderWidget for ControlConnectionScreen {
    fn render(&mut self, rect: &mut Frame<CrosstermBackend<Stdout>>, state: &mut BackendState) {
        let items: Vec<_> = state.known_generators.iter().map(|ge| {
            let style = if let Some(it) = Self::is_device_available(state, ge) {
                Style::default().fg(Color::Blue)
            } else {Style::default().fg(Color::Gray)};
            let ip_str = match ge {
                Peer::Ge(it) => {it.internal_ip_v4.to_string()}
                Peer::Server(it) => {"N/A".to_string()}
            };
            ListItem::new(Text::styled(format!("{}: {} - ({})", ge.get_id(), ip_str, ge.get_description()), style))
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
        let mut list_state = ListState::default();
        list_state.select(Some(/*available_device_indexes[*/self.preroute_selected/*]*/));
        rect.render_stateful_widget(generators, rect.size(), &mut list_state);
    }

    fn handle_input(&mut self, key_event: KeyEvent, state: &mut BackendState) -> KeyResult {
        match key_event.code {
            Up | KeyCode::Down => {
                //let available_device_indexes = Self::get_available_device_indexes(state);
                //let len = available_device_indexes.len();
                let len = state.known_generators.len();
                self.preroute_selected = (self.preroute_selected + if key_event.code == Up {len - 1} else {len + 1}) % len;
                Handled
            },
            KeyCode::Enter => {
                self.press_enter(state)
            }
            KeyCode::Esc => {
                Exited
            },
            _ => Passup(key_event),
        }
    }
}