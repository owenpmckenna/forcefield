use crate::state::BackendState;
use crate::ui::main_options_screen::MainOptionsScreen;
use common::errors::FFResult;
use crossbeam_channel::unbounded;
use crossterm::event;
use crossterm::event::{Event, KeyCode, KeyEvent};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use std::collections::VecDeque;
use std::io;
use std::io::{Stdout, Write};
use std::sync::Mutex;
use std::time::Duration;
use tui::backend::{Backend, CrosstermBackend};
use tui::{Frame, Terminal};

pub enum KeyResult {
    Handled,
    Exited,
    Passup(KeyEvent),
    AddScreen(Box<dyn RenderWidget>),
    ReplaceScreen(Box<dyn RenderWidget>)
}
pub trait RenderWidget {
    fn render(&mut self, rect: &mut Frame<CrosstermBackend<Stdout>>, state: &mut BackendState);
    fn handle_input(&mut self, key_event: KeyEvent, state: &mut BackendState) -> KeyResult;
}
pub fn ui_main(state: &mut BackendState) -> FFResult<()> {
    state.channels = Some(unbounded());
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
                last.render(rect, state);
            } else {running = false;}
        })?;
        if running && event::poll(Duration::from_millis(500)).expect("poll works") {
            if let Event::Key(key) = event::read().expect("can read events") {
                if key.code == KeyCode::Esc {
                    stack.pop();
                    continue;
                }
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
    state.save();

    terminal.backend_mut().clear()?;
    terminal.backend_mut().write_all("\r\n".as_bytes())?;
    disable_raw_mode()?;
    Ok(())
}
thread_local! {
    static QUEUE: Mutex<VecDeque<KeyResult>> = Mutex::new(VecDeque::new());
}
fn put_in_queue(kr: KeyResult) {
    QUEUE.with(|q| {
        q.lock().unwrap().push_back(kr);
    });
}
pub fn get_from_queue() -> Option<KeyResult> {
    QUEUE.with(|q| {
        q.lock().unwrap().pop_front()
    })
}
//add, replace, exit
pub fn exit_screen() {
    put_in_queue(KeyResult::Exited)
}
pub fn add_screen<T>(rw: T) where T: RenderWidget + 'static {
    put_in_queue(KeyResult::AddScreen(Box::new(rw)))
}
pub fn replace_screen<T>(rw: T) where T: RenderWidget + 'static {
    put_in_queue(KeyResult::ReplaceScreen(Box::new(rw)))
}
