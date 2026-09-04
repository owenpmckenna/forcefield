use crate::bvec;
use crate::handshaker::Generator;
use crate::state::BackendState;
use crate::ui::dialogue_box::DialogueBox;
use crate::ui::ui_main::{KeyResult, RenderWidget, add_screen, get_from_queue, replace_screen};
use crate::ui_utils::screen::{ButtonElement, Element, Pane, Screen, TextInputElement};
use common::ip::IpQuery;
use crossterm::event::KeyEvent;
use std::io::Stdout;
use tui::Frame;
use tui::backend::CrosstermBackend;

pub struct ConnectToGeneratorScreen {
    entered_ip: String,
    screen: Option<Screen<Self>>,
}
impl ConnectToGeneratorScreen {
    fn connect(&self, state: &mut BackendState) {
        let ip = self.entered_ip.clone();
        match Generator::connect_to_generator(ip.clone(), state) {
            Ok(mut it) => {
                let ip = IpQuery::query(&ip);
                let desc = match &ip {
                    Ok(it) => {it.to_normal_name()}
                    Err(it) => {format!("Error fetching ip: {}", it)}
                };
                let info = format!("Connected to `{}` as {} successfully! Internal IP `{}`. Location: `{}`\nEndpoint: {}", self.entered_ip, it.id, it.internal_ip_v4, desc, it.endpoints[0]);
                it.description = if let Ok(it) = ip {desc} else {"".to_string()};
                state.known_generators.push(it);
                state.save();
                replace_screen(DialogueBox::new("Connection success", &info));
            }
            Err(it) => {
                let error = format!("failed connecting to server `{}`. Error: {}", self.entered_ip, it);
                add_screen(DialogueBox::new("Connection failed", &error))
            }
        }

    }
    pub fn new() -> ConnectToGeneratorScreen {
        let buttons: Vec<Box<dyn Element<Self>>> = bvec![
            TextInputElement::new("", |it: &mut Self,_b:_| &mut it.entered_ip, |it,_b:_| &it.entered_ip),
            ButtonElement::new_("Connect", true, |_, ctg: &mut Self, z| ctg.connect(z))
        ];
        let mut pane = Pane::new("Connect To Generator", buttons);
        pane.enter_redirectors.push((0, 1));
        pane.allow_updown = false;
        let screen: Screen<Self> = Screen::new(vec![pane]).unwrap();
        ConnectToGeneratorScreen { entered_ip: "".to_string(), screen: Some(screen) }
    }
}

impl RenderWidget for ConnectToGeneratorScreen {
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

        get_from_queue().unwrap_or(KeyResult::Passup(key_event))
    }
}