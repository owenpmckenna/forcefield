use crate::bvec;
use crate::citadel::state::BackendState;
use crate::citadel::ui::connect_to_generator_screen::ConnectToGeneratorScreen;
use crate::citadel::ui::control_connection_screen::ControlConnectionScreen;
use crate::citadel::ui::dialogue_box::DialogueBox;
use crate::citadel::ui::setup_route::RouteSetupScreen;
use crate::citadel::ui::ui_main::{KeyResult, RenderWidget, add_screen, exit_screen, get_from_queue};
use crate::citadel::ui_utils::screen::{ButtonElement, Element, Pane, Screen};
use crossterm::event::KeyEvent;
use std::io::Stdout;
use tui::Frame;
use tui::backend::CrosstermBackend;

pub struct MainOptionsScreen {
    screen: Option<Screen<MainOptionsScreen>>,
}
impl MainOptionsScreen {
    pub fn new() -> MainOptionsScreen {
        let buttons: Vec<Box<dyn Element<Self>>> = bvec![
            ButtonElement::new_("Connect To Generator", false, move |_, _, _| {
                add_screen(ConnectToGeneratorScreen::new())
            }),
            ButtonElement::new_("Control Generators", false, move |_, _, state| {
                if state.known_generators.is_empty() {
                    add_screen(DialogueBox::new("Error", "No Generators Available"))
                } else {
                    add_screen(ControlConnectionScreen::new())
                }
            }),
            ButtonElement::new_("Set Wireguard Path", false, move |_, _, state| {
                if state.known_generators.is_empty() {
                    add_screen(DialogueBox::new("Error", "No Generators Available"))
                } else {
                    add_screen(RouteSetupScreen::new(state))
                }
            }),
            ButtonElement::new_("Exit", false, move |_, _, _| {
                exit_screen()
            }),
        ];
        let mut pane = Pane::new("Project FORCEFIELD", buttons);
        pane.render_as_list = true;
        let screen: Screen<Self> = Screen::new(vec![pane]).unwrap();
        MainOptionsScreen { screen: Some(screen) }
    }
}
impl RenderWidget for MainOptionsScreen {
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