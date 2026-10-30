use std::error::Error;
use std::fs;
use std::path::PathBuf;
use std::str::FromStr;
use crate::bvec;
use crate::generic_wireguard_endpoint::WireguardEndpoint;
use crate::handshaker::Peer;
use crate::state::BackendState;
use crate::ui::dialogue_box::DialogueBox;
use crate::ui::ui_main::{add_screen, replace_screen};
use crate::ui_utils::screen::{BasicRenderWidget, ButtonElement, Pane, Screen, TextInputElement};

pub struct ServerConnectScreen {
	toml_txt: String,
	desc_toml: String,
	file_txt: String,
	desc_file: String,
	screen: Option<Screen<Self>>,
}

impl ServerConnectScreen {
	fn read_file_and_add(_: &mut ButtonElement<Self>, s: &mut Self, state: &mut BackendState) -> Result<String, Box<dyn Error>> {
		let path = PathBuf::from_str(&s.file_txt)?;
		let text = fs::read(&path)?;
		let str = String::from_utf8(text)?;
		let wg = WireguardEndpoint::new(&str, &s.desc_file, state)?;
		let id = wg.id.clone();
		state.known_generators.push(Peer::Server(wg));
		Ok(id)
	}
	pub fn new() -> Self {
		let file_txt_box = TextInputElement::new("WG-Quick .config filename: ", |t: &mut Self, _| &mut t.file_txt, |t: &Self, _| &t.file_txt);
		let file_desc_box = TextInputElement::new("Server name: ", |t: &mut Self, _| &mut t.desc_file, |t, _| &t.desc_file);
		let install_btn = ButtonElement::new_("Install from File", true, |s, t: &mut Self, b| {
			Self::error_to_screen(Self::read_file_and_add(s, t, b))
		});
		let mut file_pane = Pane::new("Import Wireguard From File", bvec![file_txt_box, file_desc_box, install_btn]);
		file_pane.always_highlight = true;
		//let toml_txt_box = TextInputElement::new("WG-Quick .toml data: ", |t, _| &mut t.toml_txt, |t, _| &t.toml_txt);
		//let toml_pane = Pane::new("Import Wireguard From wg-quick Toml", bvec![toml_txt_box]);
		let screen = Screen::new(vec![file_pane/*, toml_pane*/]).unwrap();
		Self {
			toml_txt: "".to_string(),
			desc_file: "wireguard".to_string(),
			desc_toml: "wireguard".to_string(),
			file_txt: "./wg-quick.config".to_string(),
			screen: Some(screen),
		}
	}
	fn error_to_screen(err: Result<String, Box<dyn Error>>) {
		match err {
			Ok(_) => {replace_screen(DialogueBox::new("Success", "Server added successfully"))}
			Err(it) => {add_screen(DialogueBox::new("failure", &format!("Error occurred: {it}")))}
		}
	}
}
impl BasicRenderWidget<Self> for ServerConnectScreen {
	fn get_screen_option(&mut self) -> &mut Option<Screen<Self>> {
		&mut self.screen
	}
}