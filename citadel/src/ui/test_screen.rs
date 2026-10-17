use crate::bvec;
use crate::control_connection::ControlConnection;
use crate::state::BackendState;
use crate::ui::ui_main::{KeyResult, RenderWidget, exit_screen, get_from_queue};
use crate::ui_utils::screen;
use crate::ui_utils::screen::{ButtonElement, Element, Pane, Screen, TextView};
use chacha20poly1305::XChaCha20Poly1305;
use crossbeam_channel::{Receiver, Sender, unbounded};
use crossterm::event::KeyEvent;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::io::Stdout;
use std::net::SocketAddr;
use std::thread;
use std::time::Instant;
use tui::Frame;
use tui::backend::CrosstermBackend;

pub struct TestScreen {
	stop_sender: Sender<()>,
	info_receiver: Receiver<Info>,
	screen: Option<Screen<Self>>,
}
impl TestScreen {
	pub fn new(state: &BackendState) -> Self {
		let (stop_sender, stop_receiver) = unbounded();
		let (info_sender, info_receiver) = unbounded();
		let end_line = ButtonElement::new_("Stop", true, |a, b, c| {
			exit_screen();
		});
		let wgs: Vec<_> = state.current_wg_ids.iter().map(|it| {
			let ge = state.get_by_id(it).unwrap();
			((ge.internal_ip_v4, ge.config_port).into(), ge.id.clone(), ge.get_cipher())
		}).collect();
		let _ = thread::spawn(move || {
			let _ = Self::thread(wgs, info_sender, stop_receiver);
		});
		let mut main_pane = Pane::new("Testing connections...", bvec![end_line]);
		main_pane.allow_updown = false;
		let screen = Some(Screen::new(vec![main_pane]).unwrap());
		Self { screen, stop_sender, info_receiver }
	}
	fn thread(endpoints: Vec<(SocketAddr, String, XChaCha20Poly1305)>, info_sender: Sender<Info>, stop_receiver: Receiver<()>) -> Result<(), Box<dyn std::error::Error>> {
		for (addr, id, cipher) in endpoints {
			info_sender.send(Info::StartingConnection {
				id: id.clone()
			})?;
			if stop_receiver.try_recv().is_ok() {
				return Ok(());
			}
			let mut x = None;
			let mut num = 0;
			while x.is_none() {
				match ControlConnection::connect_assume_id(addr, &id, cipher.clone()) {
					Ok(it) => {x = Some(it);}
					Err(it) => {
						info_sender.send(Info::RetryingConnection {
							id: id.clone(),
							retry: num,
							reason: format!("received error: {}", it)
						})?;
					}
				}
				num += 1;
			}
			info_sender.send(Info::SuccessfullyConnected {id})?;
		}
		if stop_receiver.try_recv().is_ok() {
			return Ok(());
		}
		info_sender.send(Info::LookingForIp)?;
		match Self::try_get_network() {
			Ok(resp) => {
				info_sender.send(Info::GotIp {resp})?;
			}
			Err(it) => {
				info_sender.send(Info::NetworkFailed {reason: it.to_string()})?;
			}
		}
		Ok(())
	}
	fn try_get_network() -> Result<IpInfoResponse, Box<dyn Error>> {
		let ip = minreq::get("https://ipinfo.io/json").with_timeout(5).send()
			.map(|it| String::from_utf8_lossy(it.as_bytes()).to_string())?;
		let data: IpInfoResponse = serde_json::from_str(&ip)?;
		Ok(data)
	}
	fn add_line<T>(&mut self, line: T) where T: Element<Self> + 'static {
		let mut pane = &mut self.screen.as_mut().unwrap().panes[0];
		pane.element_selected += 1;
		let mut elem = &mut pane.elements;
		elem.insert(elem.len() - 1, Box::new(line));
	}
	fn replace_last_line<T>(&mut self, line: T) where T: Element<Self> + 'static {
		let elem = &mut self.screen.as_mut().unwrap().panes[0].elements;
		let id = elem.len() - 2;
		elem[id] = Box::new(line);
	}
	fn spinning_wheel() -> impl screen::TextViewer<Self> {
		let time = 750;
		let start = Instant::now();
		move |x, y| {
			let now = start.elapsed().as_millis() % time;
			//- \ | /
			if now > time / 4 {
				"-"
			} else if now > time / 2 {
				"\\"
			} else if now > time / 4 * 3 {
				"|"
			} else {"/"}
		}
	}
	fn gen_txt_view(id: String, done: bool, retry: usize, error: Option<String>, state: &BackendState) -> TextView<Self> {
		let ge = state.get_by_id(&id).unwrap();
		let desc = ge.description.clone();
		let mut view: TextView<Self> = TextView::new(false);
		view.add(if !done {"Connecting to generator: "} else {"Connected to generator: "});
		view.add(format!("{}: ({}) ", desc, id));
		if let Some(failure) = error {
			view.add(format!("error: \"{}\" ", failure));
		}
		if retry > 0 {
			view.add(format!("retries: {} ", retry));
		}
		if !done {
			view.add_new_ref(Self::spinning_wheel());
		}
		view
	}
}
impl RenderWidget for TestScreen {
	fn render(&mut self, rect: &mut Frame<CrosstermBackend<Stdout>>, state: &mut BackendState) {
		let size = rect.size();

		let mut screen = self.screen.take().unwrap();
		screen.render(rect, vec![size], self, state);
		let _ = self.screen.insert(screen);

		let info = if let Ok(val) = self.info_receiver.try_recv() {
			val
		} else {return};

		match info {
			Info::StartingConnection { id } => {
				self.add_line(Self::gen_txt_view(id, false, 0, None, state));
			}
			Info::RetryingConnection { retry, reason, id } => {
				self.replace_last_line(Self::gen_txt_view(id, false, retry, Some(reason), state));
			}
			Info::SuccessfullyConnected {id} => {
				self.replace_last_line(Self::gen_txt_view(id, true, 0, None, state));
			}
			Info::LookingForIp => {
				self.add_line(TextView::new(false).add_("Getting public IP - ").add_new(Self::spinning_wheel()));
			}
			Info::GotIp { resp } => {
				self.replace_last_line(
					TextView::new(false).add_(
						format!("Confirmed Public Ip: {} ({}, {}, {}) - {}", resp.ip, resp.city, resp.region, resp.country, resp.org)
					)
				)
			}
			Info::NetworkFailed { reason } => {
				self.replace_last_line(
					TextView::new(false).add_(
						format!("Failed to get public IP: {}", reason)
					)
				);
			}
			Info::OtherError { error } => {
				self.replace_last_line(
					TextView::new(false).add_(
						format!("Other error occurred: {}", error)
					)
				);
			}
		}
	}
	fn handle_input(&mut self, key_event: KeyEvent, state: &mut BackendState) -> KeyResult {
		let mut screen = self.screen.take().unwrap();
		screen.on_key(key_event, self, state);
		let _ = self.screen.insert(screen);

		get_from_queue().unwrap_or(KeyResult::Passup(key_event))
	}
}

impl Drop for TestScreen {
	fn drop(&mut self) {
		let _ = self.stop_sender.send(());
	}
}
enum Info {
	StartingConnection {id: String},
	RetryingConnection {id: String, retry: usize, reason: String},
	SuccessfullyConnected {id: String},
	LookingForIp,
	GotIp {resp: IpInfoResponse},
	NetworkFailed {reason: String},
	OtherError {error: String}
}

#[derive(Serialize, Deserialize)]
struct IpInfoResponse {
	pub ip: String,
	pub hostname: Option<String>,
	pub city: String,
	pub region: String,
	pub country: String,
	pub loc: String,
	pub org: String,
	pub postal: String,
	pub timezone: String,
	pub readme: Option<String>,
}