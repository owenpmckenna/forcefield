use std::error::Error;
use std::io::SeekFrom::End;
use std::io::Stdout;
use std::net::{IpAddr, SocketAddr};
use crossterm::event::KeyEvent;
use tui::backend::CrosstermBackend;
use tui::Frame;
use crate::bvec;
use crate::control_connection::ControlConnection;
use crate::handshaker::{Endpoint, Generator};
use crate::state::BackendState;
use crate::ui::dialogue_box::DialogueBox;
use crate::ui::main_options_screen::MainOptionsScreen;
use crate::ui::ui_main::{add_screen, RenderWidget, KeyResult, get_from_queue};
use crate::ui_utils::screen::{ButtonElement, Element, Pane, Screen, TextInputElement, TextView};
use common::errors::FFResult;
use common::wireguard::EndpointAddr::{Active, Passive};

pub struct GeneratorControlScreen2 {
	gen_id: usize,
	connection: Option<ControlConnection>,
	new_endpoint_str: String,
	screen: Option<Screen<Self>>,
	to_add_endpoints: Option<Box<dyn Element<Self>>>
}
impl GeneratorControlScreen2 {
	fn endpoint_element(id: usize, ge: &Generator) -> Box<dyn Element<Self>> {
		let end = ge.endpoints[id].clone();
		let btn: ButtonElement<Self> = ButtonElement::new_(&format!("{}", end), false, move |btn, cgs: &mut Self, state: &mut BackendState| {
			if !cgs.allowed_to_edit(state, id) {
				return;
			}
			let ge = cgs.get_gen_mut(state);
			let id = ge.endpoints.iter().position(|it| it.eq(&end)).unwrap();
			let via_peer = if let Endpoint::ViaPeer(it) = &ge.endpoints[id] {
				Some(it.clone())
			} else {
				None
			};
			if let Some(via_peer) = via_peer {
				match Self::remove_peer_connection(cgs, state, &via_peer) {
					Ok(it) => {
						add_screen(DialogueBox::new("Route Creation Succeeded", &it.to_string()))
					}
					Err(it) => {
						add_screen(DialogueBox::new("Route Creation Failed", &it.to_string()))
					}
				};
			}
			cgs.get_gen_mut(state).endpoints.remove(id);
			btn.marked_for_removal = true;
			state.save();
		});
		Box::new(btn)
	}
	fn handle_data(data: Result<String, Box<dyn Error>>) {
		let data = match data {
			Ok(it) => DialogueBox::new("Command Succeeded", &format!("{}", it)),
			Err(it) => DialogueBox::new("Command Failed", &format!("{}", it)),
		};
		add_screen(data);
	}
	fn get_conn(&mut self) -> &mut ControlConnection {
		self.connection.as_mut().unwrap()
	}
	fn gen_ctrl_btns() -> Vec<Box<dyn Element<Self>>>{
		//"Heartbeat", "Get Ip", "Get Routes", "Get Ipv6 Address", "Kill"
		bvec![
			ButtonElement::new_("Heartbeat", false, |_, us: &mut Self, _| {
				Self::handle_data(us.get_conn().send_heartbeat().map(|_| "Heartbeat Response Good".into()));
			}),
			ButtonElement::new_("Get Ip", false, |_, us: &mut Self, _| {
				Self::handle_data(us.get_conn().send_get_ip())
			}),
			ButtonElement::new_("Get Routes", false, |_, us: &mut Self, _| {
				let data = us.get_conn().send_get_routes().map(|it| {
					it.iter().map(ToString::to_string).collect::<Vec<_>>().join("\n")
				});
				Self::handle_data(data);
			}),
			ButtonElement::new_("Get Ipv6 Address", false, |_, us: &mut Self, _| {
				let data = us.get_conn().send_get_ipv6().map(|it| {
					it.map(|it| format!("Ipv6 address found: {}", it))
						.unwrap_or_else(|| "No Ipv6 Address Found".to_string())
				});
				Self::handle_data(data);
			}),
			ButtonElement::new_("Kill", false, |_, us: &mut Self, _| {
				Self::handle_data(us.get_conn().send_kill().map(|_| "Shutdown Server".into()))
			})
		]
	}
	fn gen_add_new_peer_endpoint_btn(next_peer_id: String, np_endpoint: SocketAddr) -> Box<dyn Element<Self>> {
		let text = format!("add via peer: {} ({})", next_peer_id, np_endpoint);
		let mut btn = ButtonElement::new_("", false, move |_, us: &mut Self, state: _| {
			let conn_stat = us.make_peer_connection(state, np_endpoint);
			if let Err(it) = conn_stat {
				add_screen(DialogueBox::new("Failed to setup peer", &it.to_string()));
				return;
			}
			let ge = us.get_gen_mut(state);
			let epid = ge.endpoints.len() - 1;
			us.to_add_endpoints = Some(Self::endpoint_element(epid, ge));
			state.save();
			add_screen(DialogueBox::new("Setup peer", &conn_stat.unwrap()));
		});
		btn.text = text;
		Box::new(btn)
	}
	pub(crate) fn new(gen_id: String, connection: Option<ControlConnection>, state: &mut BackendState) -> Self {
		let gen_id = state.get_index_by_id(&gen_id).unwrap();
		let configs: Vec<Box<dyn Element<Self>>> = bvec![
			TextView::new(|_a: _, _b: _| "id: ", true).add_new(|a: &Self, b: &BackendState| &a.get_gen(b).id),
			TextInputElement::new("desc: ", |a: &mut Self, b: _| &mut a.get_gen_mut(b).description, |a: &Self, b: _| &a.get_gen(b).description)
		];
		let configure_pane = Pane::new("Configure Generator", configs);

		let ge = &state.known_generators[gen_id];
		let mut endpoints: Vec<Box<dyn Element<Self>>> = (0..ge.endpoints.len()).map(|i| {
			Self::endpoint_element(i, ge)
		}).collect();
		if let Some((next_peer_id, np_endpoint)) = Self::can_add_via_peer_endpoint(ge, state) {
			endpoints.push(Self::gen_add_new_peer_endpoint_btn(next_peer_id, np_endpoint));
		}
		let mut endpoint_add = TextInputElement::new("Add Route: ", |it: &mut Self, _| &mut it.new_endpoint_str, |it, _| &it.new_endpoint_str);
		endpoint_add.on_enter = Some(Box::new(move |us: _, state: _| {
			let sock_addr = us.new_endpoint_str.parse();
			if let Err(err) = sock_addr {
				add_screen(DialogueBox::new("Failed to add route", &format!("error processing ip: {}", err)));
				return;
			}
			let sock_addr: SocketAddr = sock_addr.unwrap();
			if sock_addr.ip().is_global() {
				us.get_gen_mut(state).endpoints.push(Endpoint::PublicEndpoint(sock_addr))
			} else {
				let l_id = state.current_wg_ids.last().map(|it| it.to_string());
				us.get_gen_mut(state).endpoints.push(Endpoint::FromPeer(sock_addr, l_id));
			}
			let next_endpoint_id = us.get_gen(state).endpoints.len() - 1;
			let ge = &state.known_generators[gen_id];
			us.to_add_endpoints = Some(Self::endpoint_element(next_endpoint_id, ge));
			state.save();
		}));
		endpoints.push(Box::new(endpoint_add));
		let endpoints_pane = Pane::new("Endpoints", endpoints);

		let control_pane = if let Some(conn) = &connection {
			let elements = Self::gen_ctrl_btns();
			let mut control_pane = Pane::new("Controls", elements);
			control_pane.render_as_list = true;
			Some(control_pane)
		} else {None};

		let mut screen = Screen::new(vec![configure_pane, endpoints_pane]).unwrap();
		if let Some(pane) = control_pane {
			screen.panes.push(pane);
		}
		Self {
			gen_id, screen: Some(screen), new_endpoint_str: "".to_string(), connection, to_add_endpoints: None
		}
	}
	fn get_gen<'a>(&self, state: &'a BackendState) -> &'a Generator {
		&state.known_generators[self.gen_id]
	}
	fn get_gen_mut<'a>(&mut self, state: &'a mut BackendState) -> &'a mut Generator {
		&mut state.known_generators[self.gen_id]
	}

	fn make_peer_connection(&mut self, state: &mut BackendState, peer_via_endpoint: SocketAddr) -> FFResult<String> {
		let us = self.get_gen(state);
		let next = state.get_by_id(&state.get_next_gen(&us.id).unwrap()).unwrap();
		let us_conn = self.connection.as_mut().unwrap();
		let mut next_conn = ControlConnection::connect((IpAddr::V4(next.internal_ip_v4), next.config_port).into(), state)?;
		let n_routes = next_conn.order_create_wg(&us.wg_public_key, us.internal_ip_v4, us.internal_ip_v6, Passive)?;
		let u_routes = us_conn.order_create_wg(&next.wg_public_key, next.internal_ip_v4, next.internal_ip_v6, Active(peer_via_endpoint))?;
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
		us_conn.send_heartbeat()?;
		let mut next_conn = ControlConnection::connect((next.internal_ip_v4, next.config_port).into(), state)?;
		let n_routes = next_conn.order_delete_wg(&us.wg_public_key)?;
		let u_routes = us_conn.order_delete_wg(&next.wg_public_key)?;
		let n_routes = n_routes.into_iter().map(|it| it.to_string()).collect::<Vec<_>>().join("\n");
		let u_routes = u_routes.into_iter().map(|it| it.to_string()).collect::<Vec<_>>().join("\n");
		Ok(format!("Success!\nNext ({}) routes:\n{}\nOur ({}) routes:\n{}", next.id, n_routes, us.id, u_routes))
	}
	fn allowed_to_edit(&self, state: &BackendState, endpoint: usize) -> bool {
		let ge = self.get_gen(state);
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
		if ge.endpoints.iter().position(|it| it == &Endpoint::ViaPeer(next_id.to_string())).is_none() {
			Some((next_id.to_string(), sock_addr))
		} else {
			None
		}
	}
}
impl RenderWidget for GeneratorControlScreen2 {
	fn render(&mut self, rect: &mut Frame<CrosstermBackend<Stdout>>, state: &mut BackendState) {
		let size = rect.size();
		let mut screen = self.screen.take().unwrap();
		screen.render(rect, vec![size], self, state);
		let _ = self.screen.insert(screen);
	}

	fn handle_input(&mut self, key_event: KeyEvent, state: &mut BackendState) -> KeyResult {
		let mut screen = self.screen.take().unwrap();
		screen.on_key(key_event, self, state);
		let _ = self.screen.insert(screen);

		if let Some(screen) = self.to_add_endpoints.take() {
			let pane = &mut self.screen.as_mut().unwrap().panes[1];
			pane.elements.insert(0, screen);
		}

		get_from_queue().unwrap_or(KeyResult::Passup(key_event))
	}
}