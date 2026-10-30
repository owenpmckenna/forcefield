use crate::bvec;
use crate::control_connection::ControlConnection;
use crate::handshaker::{Peer, PeerData};
use crate::state::BackendState;
use crate::ui::test_screen::TextState::{CompletedGood, InProgress, InProgressErr, WaitingServer};
use crate::ui::ui_main::{KeyResult, RenderWidget, exit_screen, get_from_queue};
use crate::ui_utils::screen;
use crate::ui_utils::screen::{ButtonElement, Element, Pane, Screen, TextView};
use chacha20poly1305::XChaCha20Poly1305;
use crossbeam_channel::{Receiver, Sender, unbounded};
use crossterm::event::KeyEvent;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::error::Error;
use std::io::Stdout;
use std::net::SocketAddr;
use std::thread;
use std::thread::sleep;
use std::time::{Duration, Instant};
use tui::Frame;
use tui::backend::CrosstermBackend;
use tui::style::{Color, Style};

pub struct TestScreen {
	stop_sender: Sender<()>,
	info_receiver: Receiver<Info>,
	screen: Option<Screen<Self>>,
}
enum TestInstruction {
	Generator(SocketAddr, String, XChaCha20Poly1305),
	Server(String)
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
			match ge {
				Peer::Ge(ge) => {TestInstruction::Generator((ge.internal_ip_v4, ge.config_port).into(), ge.get_id().clone(), ge.get_cipher())}
				Peer::Server(it) => {TestInstruction::Server(it.id.clone())}
			}
		}).collect();
		let _ = thread::spawn(move || {
			let _ = Self::thread(wgs, info_sender, stop_receiver);
		});
		let mut main_pane = Pane::new("Testing connections...", bvec![end_line]);
		main_pane.allow_updown = false;
		let screen = Some(Screen::new(vec![main_pane]).unwrap());
		Self { screen, stop_sender, info_receiver }
	}
	fn thread(endpoints: Vec<TestInstruction>, info_sender: Sender<Info>, stop_receiver: Receiver<()>) -> Result<(), Box<dyn std::error::Error>> {
		for point in endpoints {
			let (addr, id, cipher) = if let TestInstruction::Generator(addr, id, cipher) = point {
				(addr, id, cipher)
			} else {continue};
			info_sender.send(Info::StartingConnection {
				id: id.clone()
			})?;
			if stop_receiver.try_recv().is_ok() {
				return Ok(());
			}
			let mut x = None;
			let mut num = 0;
			while x.is_none() {
				sleep(Duration::from_millis(350));
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
	fn gen_txt_view(id: String, status: TextState, state: &BackendState) -> TextView<Self> {
		let ge = state.get_by_id(&id).unwrap();
		let desc = ge.get_description().clone();
		let mut view: TextView<Self> = TextView::new(false);
		view.add(status.desc().to_string());
		view.add(format!("{}: ({}) ", desc, id));
		if let InProgressErr(retry, err) = &status {
			view.add(format!("error: \"{}\" ", err));
			view.add(format!("retries: {} ", *retry));
		}
		if status.is_completed() {
			view.add_new_ref(Self::spinning_wheel());
		}

		view.add(" █");
		let color = match &status {
			InProgress => {Color::Yellow}
			InProgressErr(_, _) => {Color::Red}
			WaitingServer(_) => {Color::Blue}
			CompletedGood => {Color::Green}
		};
		view.add_override_style(Style::default().fg(color));
		view
	}
}

enum TextState {
	InProgress,
	InProgressErr(usize, String),
	WaitingServer(Receiver<bool>),
	CompletedGood
}
impl TextState {
	fn desc(&self) -> &str {
		match self {
			InProgress => {"Connecting to generator: "}
			InProgressErr(_, _) => {"Connecting to generator: "}
			WaitingServer(_) => {"Waiting to check server..."}
			CompletedGood => {"Connected to generator: "}
		}
	}
	fn is_completed(&self) -> bool {
		match self {
			InProgress => false,
			InProgressErr(_, _) => false,
			WaitingServer(_) => false,
			CompletedGood => true
		}
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

		let mut message_passers: HashMap<String, Sender<bool>> = HashMap::new();

		match info {
			Info::StartingConnection { id } => {
				self.add_line(Self::gen_txt_view(id, InProgress, state));
			}
			Info::SkippingServer { id } => {
				let (tx, rx) = unbounded();
				message_passers.insert(id.clone(), tx);
				self.add_line(Self::gen_txt_view(id, WaitingServer(rx), state));
			}
			Info::RetryingConnection { retry, reason, id } => {
				self.replace_last_line(Self::gen_txt_view(id, InProgressErr(retry, reason), state));
			}
			Info::SuccessfullyConnected {id} => {
				self.replace_last_line(Self::gen_txt_view(id, CompletedGood, state));
			}
			Info::NotifyServerDecision {good, id} => {

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
	SkippingServer {id: String},
	RetryingConnection {id: String, retry: usize, reason: String},
	SuccessfullyConnected {id: String},
	NotifyServerDecision {id: String, good: bool},
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