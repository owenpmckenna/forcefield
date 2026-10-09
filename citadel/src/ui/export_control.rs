use crate::bvec;
use crate::generators::{get_generator, get_systemd_setup_script};
use crate::state::BackendState;
use crate::ui::dialogue_box::DialogueBox;
use crate::ui::ui_main::{add_screen, get_from_queue, replace_screen, replace_then_add_screen_before, KeyResult, RenderWidget, TERMINAL};
use crate::ui_utils::screen::{ButtonElement, Element, GetTextOptions, Pane, Screen, TextInputElement, TextView};
use common::errors::FFError::{BadFileName, UploadFailed};
use common::wireguard::get_routes;
use crossterm::event::KeyEvent;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use std::error::Error;
use std::fmt::format;
use std::fs;
use std::fs::File;
use std::io::{stdin, stdout, Read, Stdout, Write};
use std::path::PathBuf;
use std::process::Command;
use std::str::FromStr;
use tui::{Frame, Terminal};
use tui::backend::CrosstermBackend;
use tui::layout::{Constraint, Direction, Layout};
use crate::ui::connect_to_generator_screen::ConnectToGeneratorScreen;

pub struct ExportControl {
	screen: Option<Screen<Self>>,
	file_to_export: String,
	ssh_server: String,
	ssh_port: String,
	ssh_user: String,
	ssh_location: String,
	arch: usize
}
impl ExportControl {
	fn save(arch: usize, file_to_export: &str, state: &BackendState) -> Result<PathBuf, Box<dyn Error>> {
		let bytes = get_generator(state, if arch == 0 {"x86"} else {"aarch64"});
		let file = PathBuf::from_str(file_to_export)?;
		Ok(fs::write(file.clone(), bytes).map(|_| file)?)
	}
	pub(crate) fn new() -> ExportControl {
		let list: Vec<Box<dyn Element<Self>>> = bvec![
			TextView::new(|_, _| "x86 (linux)", true),
			TextView::new(|_, _| "aarch64 (linux)", true),
		];
		let mut arch_pane = Pane::new("Architecture", list);
		arch_pane.render_as_list = true;
		arch_pane.always_highlight = true;
		let export_pane_list: Vec<Box<dyn Element<Self>>> = bvec![
			TextInputElement::new("Export Location: ", |us: &mut Self, _| &mut us.file_to_export, |us, _| &us.file_to_export),
			ButtonElement::new_("Save", true, |s, t: &mut Self, state| {
				match Self::save(t.arch, &t.file_to_export, state) {
					Ok(buf) => {
						replace_screen(DialogueBox::new("Saved Generator", &format!("File written to: {}", buf.display())));
					}
					Err(err) => {
						add_screen(DialogueBox::new("Failed to save generator", &err.to_string()));
					}
				};
			})
		];
		let mut export_file_pane = Pane::new("Export to File", export_pane_list);
		export_file_pane.render_as_list = true;
		export_file_pane.last_button = true;

		let mut btn = ButtonElement::new_("Upload", true, |a, b: &mut Self, c| {
			match b.upload_to_server(c, a.alternate_selected == 1) {
				Ok(it) => {
					let db = DialogueBox::new("Uploaded Generator", "No errors found.");
					if let Some(it) = it {
						replace_then_add_screen_before(it, db)
					} else {replace_screen(db)}
				}
				Err(err) => {
					add_screen(DialogueBox::new("Failed to upload generator", &err.to_string()));
				}
			};
			let mut lock = TERMINAL.lock().unwrap();
			let term = lock.as_mut().unwrap();
			Self::bring_up_terminal(term).unwrap();
		});
		btn.alternate_selection = GetTextOptions::new(vec!["Upload".to_string(), "Upload + Install w/ Systemd".to_string()]);
		let ssh_list: Vec<Box<dyn Element<Self>>> = bvec![
			TextInputElement::new("Server: ", |us: &mut Self, _| &mut us.ssh_server, |us:_, _| &us.ssh_server),
			TextInputElement::new("Port: ", |us: &mut Self, _| &mut us.ssh_port, |us:_, _| &us.ssh_port),
			TextInputElement::new("User: ", |us: &mut Self, _| &mut us.ssh_user, |us:_, _| &us.ssh_user),
			TextInputElement::new("Location: ", |us: &mut Self, _| &mut us.ssh_location, |us:_, _| &us.ssh_location),
			btn
		];
		let mut ssh_pane = Pane::new("Upload Via SSH", ssh_list);
		ssh_pane.render_as_list = true;
		let screen = Some(Screen::new(vec![arch_pane, export_file_pane, ssh_pane]).unwrap());
		Self {
			screen,
			file_to_export: "./forcefield_51820".into(),
			ssh_server: Self::get_def_ip().unwrap_or("localhost".into()),
			ssh_port: "22".into(),
			ssh_user: "user".into(),
			ssh_location: "~/forcefield_51820".into(),
			arch: 0
		}
	}
	fn get_def_ip() -> Option<String> {
		let routes = get_routes();
		let def = routes.iter().find(|it| it.addresses.prefix_len() == 0)?.clone().device?;
		let normal = routes.iter().find(|it|
			it.device.eq(&Some(def.to_string())) && it.addresses.prefix_len() != 0 && it.addresses.prefix_len() != it.addresses.max_prefix_len()
		)?;
		Some(normal.addresses.addr().to_string())
	}
	fn parse_out_port(name: &str) -> Result<u16, Box<dyn Error>> {
		let err = BadFileName("no port in file name (must be named xyz_port eg. ff_5502)".into());
		let part = if let Some(it) = name.split_once("_") {
			it.1
		} else {return Err(err.into())};
		Ok(part.parse().map_err(|_| err)?)
	}
	fn upload_to_server(&self, state: &mut BackendState, upload_script: bool) -> Result<Option<Box<dyn RenderWidget>>, Box<dyn Error>> {
		let mut lock = TERMINAL.lock()?;
		let term = lock.as_mut().unwrap();
		term.show_cursor()?;
		disable_raw_mode()?;
		term.set_cursor(0, 0)?;
		term.clear().expect("could not clear");

		let ssh_location = PathBuf::from_str(&self.ssh_location)?;
		let parts = ssh_location.iter().collect::<Vec<_>>();
		let mut folder = PathBuf::new();
		for i in 0..parts.len() - 1 {
			folder = folder.join(parts[i]);
		}
		let name = parts.last()
			.ok_or(BadFileName(format!("no file name found in: {}", self.ssh_location)))?
			.to_str().unwrap();
		let port: u16 = Self::parse_out_port(name)?;
		let tmp_file_name = format!("/tmp/{}", name);
		Self::save(self.arch, &tmp_file_name, state)?;
		let tmp_sh = PathBuf::from_str("/tmp/tmp.sh")?;
		let _ = fs::remove_file(&tmp_sh);
		let mut tmp_sh_file = File::create(&tmp_sh)?;
		tmp_sh_file.write_all(
			&get_systemd_setup_script(name, &self.ssh_location).ok_or(BadFileName("no file name".into()))?.as_bytes()
		).map_err(|it| UploadFailed(format!("couldn't write systemd tmp script: {}", it)))?;
		let (user, home) = match std::env::var("SUDO_USER") {
			Ok(it) => {(it, std::env::var("SUDO_HOME")?)}
			Err(_) => {(std::env::var("USER")?, std::env::var("PWD")?)}
		};
		let mut cmd = Command::new("sudo");
		cmd.args(["-u", &user, "--", "scp"])
			.args(["-o", "ConnectTimeout=10", "-o", "ConnectionAttempts=1"])
			.args(&["-P", &self.ssh_port]);
		if upload_script {
			cmd.arg(&tmp_sh.display().to_string());
		}
		cmd.arg(&tmp_file_name)
			.arg(&format!("{}@{}:{}/", self.ssh_user, self.ssh_server, folder.display()))
			.current_dir(PathBuf::from_str(&home)?);

		let mut cmd = cmd.spawn()?;
		let output = cmd.wait()?;
		fs::remove_file(PathBuf::from_str(&tmp_file_name)?)?;
		fs::remove_file(tmp_sh)?;

		println!("\npress enter to continue...");
		stdin().read_line(&mut String::new())?;
		let script = format!("ssh -p {} {}@{} 'chmod +x {}/tmp.sh ; {}/tmp.sh'", self.ssh_port, self.ssh_user, self.ssh_server, folder.display(), folder.display());
		let mut installing_systemd = false;
		let upload_script_good = if upload_script {
			println!("run \"{}\" to install the systemd unit. Do you us to run it right now? [y/N]\n", script);

			let mut input = String::new();
			stdin().read_line(&mut input)?;
			if input.trim().to_ascii_lowercase().eq("y") {
				let mut cmd = Command::new("sudo")
					.args(["-u", &user, "--", "ssh", "-p", &self.ssh_port])
					.args(["-tt", "-o", "ConnectTimeout=10", "-o", "ConnectionAttempts=1"])
					.arg(&format!("{}@{}", self.ssh_user, self.ssh_server))
					.arg(format!("chmod +x {}/tmp.sh ; {}/tmp.sh", folder.display(), folder.display()))
					.current_dir(PathBuf::from_str(&home)?)
					.spawn()?;
				let status = cmd.wait()?;
				println!("\npress enter to continue...");
				stdin().read_line(&mut String::new())?;
				installing_systemd = true;
				status.success()
			} else {true}
		} else {true};

		if output.success() && upload_script_good {
			if installing_systemd {
				println!("We just tried to launch forcefield via systemd. Would you like to connect to it? [y/N]");
				let mut input = String::new();
				stdin().read_line(&mut input)?;
				if input.trim().to_ascii_lowercase().eq("y") {
					let mut tmp = ConnectToGeneratorScreen::new();
					tmp.entered_ip = format!("{}:{}", self.ssh_server, port);
					return Ok(Some(Box::new(tmp)))
				}
			}
			Ok(None)
		} else {
			Err(Box::new(UploadFailed(
				format!("error code: {}", output.code().map(|it| it.to_string()).unwrap_or("killed by signal".into()))
			)))
		}
	}
	fn bring_up_terminal(term: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<(), Box<dyn Error>> {
		enable_raw_mode()?;
		term.hide_cursor()?;
		term.clear()?;
		term.flush()?;
		Ok(())
	}
}
impl RenderWidget for ExportControl {
	fn render(&mut self, rect: &mut Frame<CrosstermBackend<Stdout>>, state: &mut BackendState) {
		let vl = Layout::default()
			.direction(Direction::Horizontal)
			.constraints([Constraint::Min(20), Constraint::Percentage(40), Constraint::Percentage(40)])
			.split(rect.size());
		let mut screen = self.screen.take().unwrap();
		screen.render(rect, vl, self, state);
		let _ = self.screen.insert(screen);
	}

	fn handle_input(&mut self, key_event: KeyEvent, state: &mut BackendState) -> KeyResult {
		let mut screen = self.screen.take().unwrap();
		screen.on_key(key_event, self, state);
		self.arch = screen.panes[0].element_selected;
		let _ = self.screen.insert(screen);

		get_from_queue().unwrap_or(KeyResult::Passup(key_event))
	}
}