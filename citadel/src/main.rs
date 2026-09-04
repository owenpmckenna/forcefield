#![feature(ip)]
#![feature(fn_traits)]
#![feature(trait_alias)]

pub mod state;
pub mod handshaker;
pub mod ui;
mod control_connection;
pub mod ui_utils;

use crate::state::BackendState;
use crate::ui::ui_main::ui_main;

pub fn main() {
    let mut state = BackendState::get();
    ui_main(&mut state).unwrap();
    if let Some(mut wg) = state.current_wg_setup {
        wg.down();
    }
}