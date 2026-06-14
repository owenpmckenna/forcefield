use crate::citadel::ui::main_options_screen::MainOptionsScreen;
use crate::common::errors::FFResult;
use crossterm::event;
use crossterm::event::{Event, KeyEvent};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use std::io;
use std::io::{Stdout, Write};
use std::time::Duration;
use tui::backend::{Backend, CrosstermBackend};
use tui::{Frame, Terminal};
use crate::citadel::state::BackendState;

pub enum KeyResult {
    Handled,
    Exited,
    Passup(KeyEvent),
    AddScreen(Box<dyn RenderWidget>),
    ReplaceScreen(Box<dyn RenderWidget>)
}
pub trait RenderWidget {
    fn render(&mut self, rect: &mut Frame<CrosstermBackend<Stdout>>);
    fn handle_input(&mut self, key_event: KeyEvent, state: &mut BackendState) -> KeyResult;
}
pub fn ui_main(state: &mut BackendState) -> FFResult<()> {
    enable_raw_mode()?;
    let stdout = io::stdout();
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;
    let mut running = true;
    let mut stack: Vec<Box<dyn RenderWidget>> = vec![];
    let mos = MainOptionsScreen::new();
    stack.push(Box::new(mos));
    while running {
        terminal.draw(|rect| {
            if let Some(last) = stack.last_mut() {
                last.render(rect);
            } else {running = false;}
        })?;
        if running && event::poll(Duration::from_millis(500)).expect("poll works") {
            if let Event::Key(key) = event::read().expect("can read events") {
                if let Some(last) = stack.last_mut() {
                    match last.handle_input(key, state) {
                        KeyResult::Handled => {}
                        KeyResult::Exited => {stack.pop();}
                        KeyResult::Passup(_) => {/*ignore for now haha*/},
                        KeyResult::AddScreen(it) => {stack.push(it);},
                        KeyResult::ReplaceScreen(it) => {stack.pop(); stack.push(it);}
                    }
                }
            }
        }
    }

    terminal.backend_mut().clear()?;
    terminal.backend_mut().write_all("\r\n".as_bytes())?;
    disable_raw_mode()?;
    Ok(())
}