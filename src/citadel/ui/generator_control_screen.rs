use crate::citadel::control_connection::ControlConnection;
use crate::citadel::handshaker::{Endpoint, Generator};
use crate::citadel::state::BackendState;
use crate::citadel::ui::cursor::Cursor;
use crate::citadel::ui::dialogue_box::DialogueBox;
use crate::citadel::ui::generator_control_screen::ScreenSelected::{Control, Data, Routes};
use crate::citadel::ui::ui_main::KeyResult::{AddScreen, Exited, Handled, ReplaceScreen};
use crate::citadel::ui::ui_main::{KeyResult, RenderWidget};
use crossterm::event::KeyCode::Up;
use crossterm::event::{KeyCode, KeyEvent};
use std::cmp::PartialEq;
use std::env::args;
use std::io::Stdout;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering::SeqCst;
use tui::backend::CrosstermBackend;
use tui::layout::{Alignment, Constraint, Direction, Layout};
use tui::style::{Color, Style};
use tui::text::{Span, Spans};
use tui::widgets::{Block, BorderType, Borders, List, ListItem, ListState, Paragraph, Wrap};
use tui::Frame;
use crate::common::errors::FFResult;

pub struct GeneratorControlScreen {
    gen_id: String,
    connection: Option<ControlConnection>,
    section: ScreenSelected,
    data_selected: usize,
    command_selected: usize,

    description_text: String,
    description_cursor: Cursor<Self>,

    endpoint_selected: usize,
    peer_via_endpoint: Option<(String, SocketAddr)>,
    new_endpoint_ip: String,
    new_endpoint_ip_cursor: Cursor<Self>,
    new_endpoint_ip_cursor_len: Arc<AtomicUsize>
}
#[repr(u8)]
#[derive(Copy, Clone, Eq, PartialEq)]
enum ScreenSelected {
    Data,
    Routes,
    Control,
}
impl ScreenSelected {
    fn next(&mut self, do_controls: bool) {
        *self = match self {
            Data => Routes,
            Routes => if do_controls {Control} else {Data},
            Control => Data,
        }
    }
}
impl GeneratorControlScreen {
    pub fn new(gen_id: String, connection: Option<ControlConnection>, state: &mut BackendState) -> GeneratorControlScreen {
        let len = state.get_by_id(&gen_id).unwrap().endpoints.len();
        let cursor_len = Arc::new(AtomicUsize::new(len));
        let mut gs = GeneratorControlScreen {
            gen_id,
            connection,
            description_text: "".into(),
            section: Data,
            data_selected: 0,
            command_selected: 0,
            description_cursor: Cursor::new(|it: &Self| {
                it.section == Data && it.data_selected == 0
            }),
            endpoint_selected: 0,
            new_endpoint_ip: "".into(),
            new_endpoint_ip_cursor_len: cursor_len.clone(),
            new_endpoint_ip_cursor: Cursor::new(move |it: &Self| it.section == Routes && it.endpoint_selected == cursor_len.load(SeqCst) + if it.peer_via_endpoint.is_some() { 2 } else { 1 } - 1),
            peer_via_endpoint: None,
        };
        let ge = gs.get_gen(state);
        if gs.connection.is_some() {
            gs.peer_via_endpoint = Self::can_add_via_peer_endpoint(ge, state);
        }
        gs.description_text = match &ge.description {
            None => "".into(),
            Some(it) => it.clone(),
        };
        gs
    }
    fn get_gen_mut<'a>(&self, state: &'a mut BackendState) -> &'a mut Generator {
        state.get_by_id_mut(&self.gen_id).unwrap()
    }
    fn get_gen<'a>(&self, state: &'a BackendState) -> &'a Generator {
        state.get_by_id(&self.gen_id).unwrap()
    }
    fn allowed_to_edit(ge: &Generator, state: &BackendState, endpoint: usize) -> bool {
        //you cannot remove the endpoint we're using to talk to the device, that's dumb
        for it in 0..state.current_wg_ids.len() {
            if !state.current_wg_ids[it].eq(&ge.id) {
                continue;
            }
            if state.endpoints_used[it] == ge.endpoints[endpoint] {
                return false;
            }
        }
        true
    }
    fn try_gen_next_peer(ge: &Generator, state: &BackendState) -> Option<(String, Endpoint)> {
        let our_id = state.current_wg_ids.iter().position(|it| it.eq(&ge.id))?;
        let next_endpoint = state.endpoints_used.get(our_id + 1)?;
        let next_id = state.current_wg_ids[our_id + 1].as_str();
        Some((next_id.into(), next_endpoint.clone()))
    }
    ///for this to work, the endpoint immediately after ours must be a public endpoint which
    /// we are connected to. there also can't already be one
    fn can_add_via_peer_endpoint(
        ge: &Generator,
        state: &BackendState,
    ) -> Option<(String, SocketAddr)> {
        let (next_id, next_endpoint) = Self::try_gen_next_peer(ge, state)?;
        let sock_addr = match next_endpoint {
            Endpoint::PublicEndpoint(it) => {it}
            Endpoint::ViaPeer(_) => {return None}
            Endpoint::FromPeer(it, _) => {it}
        };
        if let None = ge.endpoints.iter().position(|it| it == &Endpoint::ViaPeer(next_id.to_string())) {
                Some((next_id.to_string(), sock_addr.clone()))
        } else {
            None
        }
    }
    fn routes_len_add(&self, ge: &Generator) -> usize {
        ge.endpoints.len() + if self.peer_via_endpoint.is_some() {
            2
        } else {
            1
        }
    }
    fn make_peer_connection(&mut self, state: &mut BackendState, peer_via_endpoint: SocketAddr) -> FFResult<String> {
        let us = self.get_gen(state);
        let next = state.get_by_id(&self.peer_via_endpoint.as_ref().unwrap().0).unwrap();
        let us_conn = self.connection.as_mut().unwrap();
        let mut next_conn = ControlConnection::connect((next.internal_ip, next.config_port).into(), state)?;
        let n_routes = next_conn.order_create_wg(&us.wg_public_key, us.internal_ip, None)?;
        let u_routes = us_conn.order_create_wg(&next.wg_public_key, next.internal_ip, Some(peer_via_endpoint))?;
        let next = next.id.clone();//yeah lifetime name shadowing shut up
        let us = self.get_gen_mut(state);
        us.endpoints.push(Endpoint::ViaPeer(next.clone()));
        let n_routes = n_routes.into_iter().map(|it| it.to_string()).collect::<Vec<_>>().join("\n");
        let u_routes = u_routes.into_iter().map(|it| it.to_string()).collect::<Vec<_>>().join("\n");
        Ok(format!("Success!\nNext ({}) routes:\n{}\nOur ({}) routes:\n{}", next, n_routes, us.id, u_routes))
    }
    fn remove_peer_connection(&mut self, state: &BackendState, endpoint_peer: &str) -> FFResult<String> {
        let us = self.get_gen(state);
        let next = state.get_by_id(endpoint_peer).unwrap();
        let us_conn = self.connection.as_mut().unwrap();
        let mut next_conn = ControlConnection::connect((next.internal_ip, next.config_port).into(), state)?;
        let n_routes = next_conn.order_delete_wg(&us.wg_public_key)?;
        let u_routes = us_conn.order_delete_wg(&next.wg_public_key)?;
        let n_routes = n_routes.into_iter().map(|it| it.to_string()).collect::<Vec<_>>().join("\n");
        let u_routes = u_routes.into_iter().map(|it| it.to_string()).collect::<Vec<_>>().join("\n");
        Ok(format!("Success!\nNext ({}) routes:\n{}\nOur ({}) routes:\n{}", next.id, n_routes, us.id, u_routes))
    }
}
impl RenderWidget for GeneratorControlScreen {
    fn render(&mut self, rect: &mut Frame<CrosstermBackend<Stdout>>, state: &mut BackendState) {
        let ge = self.get_gen(state);
        let do_controls = self.connection.is_some();

        let desc_same = ge
            .description
            .as_ref()
            .unwrap_or(&"".into())
            .eq(&self.description_text);
        let unsaved = !desc_same;
        let unnsaved_text = if unsaved { " (Unsaved)" } else { "" };

        //if not doing controls, don't split vertical layout
        let vertical_layout = if do_controls {Layout::default()
            .direction(Direction::Vertical)
            .constraints(&[Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(rect.size())} else {vec![rect.size()]};
        let horizontal_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(&[Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(vertical_layout[0]);

        let id_text = Spans::from(vec![
            Span::raw("id: "),
            Span::raw(self.gen_id.clone()),
        ]);
        let desc_text = Spans::from(self.description_cursor.render(
            vec![
                Span::raw("desc: "),
                Span::raw(self.description_text.clone()),
            ],
            self,
        ));
        let data = Paragraph::new(vec![id_text, desc_text])
            .style(Style::default().fg(Color::White).bg(Color::Black))
            .alignment(Alignment::Left)
            .block(
                Block::default()
                    .border_type(BorderType::Plain)
                    .borders(Borders::ALL - Borders::BOTTOM - Borders::RIGHT)
                    .title(format!("Configure Generator{}", unnsaved_text)),
            )
            .wrap(Wrap { trim: false });
        rect.render_widget(data, horizontal_layout[0]);

        let mut routes_txt: Vec<_> = ge
            .endpoints
            .iter()
            .enumerate()
            .map(|(it, end)| {
                if self.section == Routes && self.endpoint_selected == it {
                    ListItem::new(if Self::allowed_to_edit(ge, state, it) {
                        format!("{} - press <enter> to delete", end)
                    } else {
                        format!("{} - in use", end)
                    })
                } else {
                    ListItem::new(end.to_string())
                }
            })
            .collect();
        if let Some((id, addr)) = &self.peer_via_endpoint {
            routes_txt.push(ListItem::new(format!(
                "Add Reverse Route Via {} (at {})",
                id, addr
            )));
        }
        let text = if self.new_endpoint_ip_cursor.is_active(self) {
            vec![
                Span::raw("Add Route - "),
                Span::styled(
                    &self.new_endpoint_ip,
                    Style::default().fg(Color::LightCyan),
                ),
            ]
        } else {
            vec![Span::raw("Add Route")]
        };
        routes_txt.push(ListItem::new(Spans::from(
            self.new_endpoint_ip_cursor.render(text, self),
        )));
        let routes = List::new(routes_txt)
            .style(Style::default().fg(Color::White))
            .block(
                Block::default()
                    .borders(Borders::TOP | Borders::RIGHT | Borders::LEFT)
                    .style(Style::default().fg(Color::White))
                    .title("Routes")
                    .border_type(BorderType::Plain),
            )
            .highlight_symbol("> ");
        if self.section == Routes {
            let mut list_state = ListState::default();
            list_state.select(Some(self.endpoint_selected));
            rect.render_stateful_widget(routes, horizontal_layout[1], &mut list_state);
        } else {
            rect.render_widget(routes, horizontal_layout[1]);
        }

        if !do_controls {
            return;
        }
        let control_list = vec!["Heartbeat", "Get Ip", "Get Routes", "Kill"]
            .into_iter()
            .map(|it| ListItem::new(it))
            .collect::<Vec<_>>();
        let color = if self.section == Control {
            Color::White
        } else {
            Color::LightCyan
        };
        let picker = List::new(control_list)
            .style(Style::default().fg(color))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .style(Style::default().fg(Color::White))
                    .title("Send Command")
                    .border_type(BorderType::Plain),
            )
            .highlight_symbol("> ");

        let mut list_state = ListState::default();
        list_state.select(Some(self.command_selected));

        if self.section == Control {
            rect.render_stateful_widget(picker, vertical_layout[1], &mut list_state);
        } else {
            rect.render_widget(picker, vertical_layout[1]);
        }
    }

    fn handle_input(&mut self, key_event: KeyEvent, state: &mut BackendState) -> KeyResult {
        match key_event.code {
            Up | KeyCode::Down => {
                let len = match self.section {
                    Data => 1,
                    Routes => self.routes_len_add(self.get_gen(state)),
                    Control => 4,
                };
                let it = match self.section {
                    Data => &mut self.data_selected,
                    Routes => &mut self.endpoint_selected,
                    Control => &mut self.command_selected,
                };
                *it = (*it + if key_event.code == Up { len - 1 } else { len + 1 }) % len;
                Handled
            }
            KeyCode::Tab => {
                self.section.next(self.connection.is_some());
                Handled
            }
            KeyCode::Char(ch) => {
                if self.section == Data && self.data_selected == 0 {
                    self.description_cursor.update_key();
                    self.description_text.push(ch);
                }
                //getting if we're on the last is kinda complex here whoops
                if self.section == Routes
                    && self.endpoint_selected == self.routes_len_add(self.get_gen(state)) - 1
                {
                    self.new_endpoint_ip_cursor.update_key();
                    self.new_endpoint_ip.push(ch);
                }
                Handled
            }
            KeyCode::Backspace => {
                if self.section == Data && self.data_selected == 0 {
                    self.description_cursor.update_key();
                    self.description_text.pop();
                }
                if self.section == Routes
                    && self.endpoint_selected == self.routes_len_add(self.get_gen(state)) - 1
                {
                    self.new_endpoint_ip_cursor.update_key();
                    self.new_endpoint_ip.pop();
                }
                Handled
            }
            KeyCode::Enter => {
                match self.section {
                    Data => {
                        if self.data_selected == 0 {
                            self.get_gen_mut(state).description = Some(self.description_text.clone());
                            state.save();
                        }
                        Handled
                    }
                    Control => {
                        let connection =self.connection.as_mut().unwrap();
                        let out = if self.command_selected == 0 {
                            connection
                                .send_heartbeat()
                                .map(|_| "Heartbeat Response Good".into())
                        } else if self.command_selected == 1 {
                            connection.send_get_ip()
                        } else if self.command_selected == 2 {
                            match connection.send_get_routes() {
                                Ok(it) => {
                                    let mgen = self.get_gen_mut(state);
                                    let str = it.iter().map(ToString::to_string).collect::<Vec<_>>().join("\n");
                                    *mgen.probable_routes.lock().unwrap() = it;
                                    Ok(str)
                                }
                                Err(it) => {Err(it)}
                            }
                        } else {
                            connection
                                .send_kill()
                                .map(|_| "Shutdown Server".into())
                        };
                        let screen = match out {
                            Ok(it) => DialogueBox::new("Command Succeeded", &format!("{}", it)),
                            Err(it) => DialogueBox::new("Command Failed", &format!("{}", it)),
                        };
                        if self.command_selected == 3 {
                            ReplaceScreen(Box::new(screen))
                        } else {
                            AddScreen(Box::new(screen))
                        }
                    }
                    Routes => {
                        let len = self.routes_len_add(self.get_gen(state));
                        if self.endpoint_selected == len - 1 {
                            let sock_addr = self.new_endpoint_ip.parse();
                            if let Err(err) = sock_addr {
                                return AddScreen(Box::new(DialogueBox::new("Failed to add route", &format!("error processing ip: {}", err))))
                            }
                            let sock_addr: SocketAddr = sock_addr.unwrap();
                            if sock_addr.ip().is_global() {
                                self.get_gen_mut(state).endpoints.push(Endpoint::PublicEndpoint(sock_addr))
                            } else {
                                let l_id = state.current_wg_ids.last().map(|it| it.to_string());
                                self.get_gen_mut(state).endpoints.push(Endpoint::FromPeer(sock_addr, l_id));
                            }
                            state.save();
                            self.new_endpoint_ip_cursor_len.fetch_add(1, SeqCst);
                            return Handled
                        } else if self.endpoint_selected == len - 2 && self.peer_via_endpoint.is_some() {
                            //return Handled
                            let pve = self.peer_via_endpoint.as_ref().unwrap();
                            return AddScreen(Box::new(match Self::make_peer_connection(self, state, pve.1) {
                                Ok(it) => {
                                    state.save();
                                    self.peer_via_endpoint.take();
                                    DialogueBox::new("Route Creation Succeeded", &format!("{}", it))
                                },
                                Err(it) => DialogueBox::new("Route Creation Failed", &format!("{}", it)),
                            }));
                        }
                        let ge = self.get_gen(state);
                        if !Self::allowed_to_edit(ge, state, self.endpoint_selected) {
                            return Handled
                        }
                        let ge = self.get_gen(state);
                        let removed = &ge.endpoints[self.endpoint_selected];
                        if let Endpoint::ViaPeer(peer) = removed {
                            return AddScreen(Box::new(match Self::remove_peer_connection(self, state, peer) {
                                Ok(it) => {
                                    self.peer_via_endpoint = Self::can_add_via_peer_endpoint(ge, state);
                                    self.get_gen_mut(state).endpoints.remove(self.endpoint_selected);
                                    state.save();
                                    DialogueBox::new("Route Creation Succeeded", &format!("{}", it))
                                },
                                Err(it) => {DialogueBox::new("Route Creation Failed", &format!("{}", it))},
                            }));
                        };
                        //fix index so it doesn't point at something nonexistent
                        self.endpoint_selected = self.endpoint_selected.saturating_sub(1);
                        self.new_endpoint_ip_cursor_len.fetch_sub(1, SeqCst);
                        Handled
                    },
                }
            }
            KeyCode::Esc => Exited,
            _ => Handled,
        }
    }
}
