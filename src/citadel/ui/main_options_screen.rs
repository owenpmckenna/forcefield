use crate::citadel::ui::ui_main::{KeyResult, RenderWidget};
use crossterm::event::{KeyCode, KeyEvent};
use std::io::Stdout;
use rsa::Error::NprimesTooSmall;
use tui::backend::CrosstermBackend;
use tui::layout::{Constraint, Direction, Layout};
use tui::style::{Color, Style};
use tui::widgets::{Block, BorderType, Borders, List, ListItem, ListState};
use tui::Frame;
use crate::citadel::state::BackendState;
use crate::citadel::ui::connect_to_generator_screen::ConnectToGeneratorScreen;
use crate::citadel::ui::dialogue_box::DialogueBox;
use crate::citadel::ui::setup_route::RouteSetupScreen;

pub struct MainOptionsScreen {
    current: usize
}
#[derive(Copy, Clone, Debug)]
#[repr(usize)]
enum Options {
    ConnectToGenerator,
    SetRoute,
    Exit
}
impl MainOptionsScreen {
    pub fn new() -> MainOptionsScreen {
        MainOptionsScreen { current: 0 }
    }
}
impl RenderWidget for MainOptionsScreen {
    fn render(&mut self, rect: &mut Frame<CrosstermBackend<Stdout>>) {
        self.current += 4;
        self.current %= 4;

        let size = rect.size();
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(2)
            .constraints(
                [
                    Constraint::Min(3),
                ]
                    .as_ref(),
            )
            .split(size);
        let items = vec!["Connect To Generator", "Control Generators", "Set Wireguard Path", "Exit"];
        let picker = List::new(items.into_iter().map(|it| ListItem::new(it)).collect::<Vec<ListItem>>())
            .style(Style::default().fg(Color::LightCyan))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .style(Style::default().fg(Color::White))
                    .title("Project FORCEFIELD")
                    .border_type(BorderType::Plain),
            )
            .highlight_symbol("> ");

        let mut list_state = ListState::default();
        list_state.select(Some(self.current));

        rect.render_stateful_widget(picker, chunks[0], &mut list_state);
    }

    fn handle_input(&mut self, key_event: KeyEvent, state: &mut BackendState) -> KeyResult {
        match key_event.code {
            KeyCode::Down => {self.current += 1; KeyResult::Handled}
            KeyCode::Up => {self.current -= 1; KeyResult::Handled}
            KeyCode::Enter => {
                let screen: Option<Box<dyn RenderWidget>> = if self.current == 0 {
                    Some(Box::new(ConnectToGeneratorScreen::new()))
                } else if self.current == 2 && !state.known_generators.is_empty() {
                    Some(Box::new(RouteSetupScreen::new(state)))
                } else if self.current == 2 && state.known_generators.is_empty() {
                    Some(Box::new(DialogueBox::new("Error".into(), "No Generators Available".into())))
                } else if self.current == 3 {return KeyResult::Exited} else {None};
                if let Some(screen) = screen {
                    KeyResult::AddScreen(screen)
                } else {KeyResult::Passup(key_event)}
            }
            KeyCode::Esc => KeyResult::Exited,
            _ => KeyResult::Passup(key_event),
        }
    }
}