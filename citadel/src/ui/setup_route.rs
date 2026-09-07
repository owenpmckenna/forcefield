use crate::bvec;
use crate::handshaker::Endpoint;
use crate::state::BackendState;
use crate::ui::dialogue_box::DialogueBox;
use crate::ui::ui_main::{KeyResult, RenderWidget, add_screen, get_from_queue, replace_screen};
use crate::ui_utils::screen::{ButtonElement, Element, GetTextFnMut, GetTextOptions, Pane, Screen, TextInputElement, TextView};
use common::cmd::exec;
use common::ip::{get_routable_address, Port};
use common::wireguard::{Route, get_routes};
use crossterm::event::KeyEvent;
use std::cmp::PartialEq;
use std::io::Stdout;
use std::net::{Ipv4Addr, Ipv6Addr};
use std::ops::Index;
use tui::Frame;
use tui::backend::CrosstermBackend;
use tui::layout::{Constraint, Direction, Layout};
use tui::style::{Color, Style};

pub struct RouteSetupScreen {
	current_selected: Vec<(usize, Endpoint, String, Option<u16>)>,
	cached_gen_text: Vec<(String, Style)>,
	target_address: String,
	routes: Vec<Route>,
	pub_ip: Option<(Option<Ipv6Addr>, Option<Ipv4Addr>)>,
	screen: Option<Screen<Self>>,
	element_selected: usize
}
impl RouteSetupScreen {
    fn cached_gen_style(&self, index: usize) -> Style {
        self.cached_gen_text[index].1
    }
    pub fn available_gen_button(index: usize, state: &BackendState) -> Box<dyn Element<Self>> {
        let mut btn = ButtonElement::new("", Box::new(move |it: &Self, _| it.cached_gen_style(index)), false, move |btn:_, us: _, state: _| {
            us.alternate_selected(state, btn.alternate_selected);
            //rerun the text gen
			for i in 0..state.known_generators.len() {
				us.selected_text(state, i);
			}
		});
		btn.set_allow_selection(move |us: _, state: _| {
			us.get_id_allowed(state, index)
		});
		btn.other_text = Some(GetTextFnMut::new(move |us: &Self, _| {
			vec![&us.cached_gen_text[index].0]
		}));
		let mut wg_names: Vec<_> = state.known_generators[index].ws_ports.iter().map(|it| it.to_string()).collect();
		wg_names.insert(0, "Bare".into());
		btn.alternate_selection = GetTextOptions::new(wg_names);
		Box::new(btn)
    }
	pub fn selected_gen_button(index: usize) -> Box<dyn Element<Self>> {
		Box::new(TextView::new(move |it: &Self, _| {
			&it.current_selected[index].2
		}, false))
	}
	fn selected_to_text(index: usize, it: &String, state: &BackendState) -> (usize, Endpoint, String, Option<Port>) {
		let end = &state.endpoints_used[index];
		let ws = if let Some(it) = end.1 {
			format!(" - over WS {}", it)
		} else {"".into()};
		(state.known_generators.iter().position(|g| g.id.eq(it)).unwrap(),
		 end.0.clone(),
		 format!("{} - {}{}", it, end.0, ws), end.1)
	}
    pub fn new(state: &mut BackendState) -> Self {
        let current_selected: Vec<(usize, Endpoint, String, Option<u16>)> = state.current_wg_ids.iter().enumerate().map(|(index, it)|
            Self::selected_to_text(index, it, state)).collect();
        let routes = get_routes();
        let pub_ip = if current_selected.is_empty() {Some(get_routable_address())} else {None};
		let generator_buttons: Vec<Box<dyn Element<Self>>> = (0..state.known_generators.len()).into_iter().map(|it| {
			Self::available_gen_button(it, state)
		}).collect();
		let mut gen_pane = Pane::new("Available Generators", generator_buttons);
		gen_pane.render_as_list = true;
		let mut list_pane = Pane::new("Selected Generators", (0..current_selected.len()).map(Self::selected_gen_button).collect());
		list_pane.skip_selection = true;
		let cidr_input: TextInputElement<Self> = TextInputElement::new("CIDR: ", |us: &mut Self, _| &mut us.target_address, |us: _, _| &us.target_address);
		let connect_btn = ButtonElement::new_("Connect", true, |_, us: &mut Self, state: _| {
			let cs = us.current_selected.iter().cloned().map(|(a, b, _, c)| (a, b, c)).collect();
			match state.create_wg_setup(cs, us.target_address.to_string()) {
				Ok(_) => {
					let out = exec("ip route".into());
					let output = format!("`ip route` responded with {} lines:\n{}", out.len(), out);
					replace_screen(DialogueBox::new("Wireguard Setup Successful", &output))
				}
				Err(it) => {
					let output = format!("Error occurred: {}", it);
					add_screen(DialogueBox::new("Wireguard Setup Failed", &output))
				}
			}
		});
		let settings_pane = Pane::new("Settings", bvec![cidr_input, connect_btn]);
        let screen = Some(Screen::new(vec![gen_pane, list_pane, settings_pane]).unwrap());
        let mut se = Self { current_selected, cached_gen_text: vec![], target_address: "0.0.0.0/0,::/0".to_string(), routes, pub_ip, screen, element_selected: 0 };
        for i in 0..state.known_generators.len() {
            se.selected_text(state, i);
        }
		for i in 0..state.known_generators.len() {
			if se.get_id_allowed(state, i) {
				se.element_selected = i;
				break
			}
		}
        se
    }
    fn current_selected_to_ids<'a>(&self, state: &'a BackendState) -> Vec<&'a String> {
        self.current_selected.iter().map(|it| &state.known_generators[it.0].id).collect()
    }
	fn get_id_allowed(&self, state: &BackendState, index: usize) -> bool {
		if let Some((id, _, _, _)) = self.current_selected.last() && *id == index {
			return true
		}
		if self.current_selected.iter().position(|it| it.0 == index).is_some() {
			return false
		}
		let last_id = self.current_selected.last().map(|it| state.known_generators[it.0].id.clone());
		//let available_routes = last_id.as_ref().map(|it| state.get_by_id(&it)).flatten()
		//    .map(|it| it.probable_routes.lock().ok()).flatten();
		!state.known_generators[index].find_best_endpoint(&self.routes, Err(last_id), self.useful_ip_info()).is_none()
	}
    fn get_allowed_ids(&self, state: &BackendState) -> Vec<usize> {
        let mut data = Vec::with_capacity(state.known_generators.len());
        for i in 0..state.known_generators.len() {
			if self.get_id_allowed(state, i) {
				data.push(i);
			}
        }
        data
    }
    fn useful_ip_info(&self) -> Option<(Option<Ipv6Addr>, Option<Ipv4Addr>)> {
        if let Some(pub_ip) = self.pub_ip && self.current_selected.is_empty() {
            Some(pub_ip)
        } else {None}
    }
    fn current(&self) -> usize {
        self.element_selected
    }
    fn alternate_selected(&mut self, state: &BackendState, ws_selected: usize) {
        if let Some(ind) = self.is_selected(self.current()) {
            self.current_selected.remove(ind);
        } else {
            let last_id = self.current_selected.last().map(|it| state.known_generators[it.0].id.clone());
			let ge = &state.known_generators[self.current()];
            let best = ge.find_best_endpoint(&self.routes, Err(last_id), self.useful_ip_info());
			let (ws, wst) = if ws_selected == 0 {
				(None, "".into())
			} else {let port = ge.ws_ports[ws_selected - 1]; (Some(port), format!(" - over WS {}", port))};
			let txt = format!("{} - {}{}", ge.id, best.as_ref().unwrap(), wst);
            self.current_selected.push((self.current(), best.unwrap().clone(), txt, ws));
        }
    }
    fn is_selected(&self, index: usize) -> Option<usize> {
        self.current_selected.iter().position(|(i, _, _, _)| *i == index)
    }
    fn is_selected_ge(&self, state: &BackendState, ge: &str) -> Option<usize> {
        self.current_selected.iter().position(|(it, _, _, _)| state.known_generators[*it].id == ge)
    }
    fn selected_style(&self, sel: bool, has_route: bool) -> Style {
        if sel {
            Style::default().fg(Color::Blue)
        } else if has_route {
            Style::default().fg(Color::White)
        } else {
            Style::default().fg(Color::DarkGray)
        }
    }
    fn selected_text(&mut self, state: &BackendState, index: usize) {
		//unselected + allowed: white, unselected + disallowed: gray, selected: blue
        let last_id = self.current_selected.last().map(|it| state.known_generators[it.0].id.clone());
        let pos = self.is_selected(index);
        let useful_ip_info = if let Some(it) = self.pub_ip && (pos.eq(&Some(0)) || pos.eq(&None)) {
            Some(it)
        } else {None};
        let best = state.known_generators[index].find_best_endpoint(&self.routes, Err(last_id), useful_ip_info);
        let style = self.selected_style(pos.is_some(), best.is_some());
        let it = &state.known_generators[index];
        let out = (it.get_generator_text(&best), style);
        if self.cached_gen_text.len() <= index {
            self.cached_gen_text.push(out)
        } else {
            self.cached_gen_text[index] = out;
        }
    }
}


impl RenderWidget for RouteSetupScreen {
    fn render(&mut self, rect: &mut Frame<CrosstermBackend<Stdout>>, state: &mut BackendState) {
        let size = rect.size();
        let v = Layout::default()
            .direction(Direction::Vertical)
            .constraints(
                [
                    Constraint::Percentage(50),
                    Constraint::Percentage(50),
                ]
                    .as_ref(),
            )
            .split(size);
        let h = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(
                [
                    Constraint::Percentage(50),
                    Constraint::Percentage(50),
                ]
                    .as_ref(),
            )
            .split(v[1]);


		let mut screen = self.screen.take().unwrap();
		self.element_selected = screen.panes[0].element_selected;
		let sel = self.current_selected.len();
		if sel > screen.panes[1].elements.len() {
			screen.panes[1].elements.push(Self::selected_gen_button(sel - 1))
		}
		if sel < screen.panes[1].elements.len() {
			screen.panes[1].elements.pop();
		}
		screen.render(rect, vec![v[0], h[0], h[1]], self, state);
		let _ = self.screen.insert(screen);
    }
    fn handle_input(&mut self, key_event: KeyEvent, state: &mut BackendState) -> KeyResult {
		let mut screen = self.screen.take().unwrap();
		screen.on_key(key_event, self, state);
		let _ = self.screen.insert(screen);

		get_from_queue().unwrap_or(KeyResult::Passup(key_event))
	}
}
