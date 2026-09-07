#![feature(ip)]
#![feature(fn_traits)]
#![feature(trait_alias)]

pub mod state;
pub mod handshaker;
pub mod ui;
mod control_connection;
pub mod ui_utils;

use common::wireguard::try_extract_wstunnel;
use crate::state::BackendState;
use crate::ui::ui_main::ui_main;

pub fn main() {
    let _ = try_extract_wstunnel();
    let mut state = BackendState::get();
    ui_main(&mut state).unwrap();
    let _ = state.create_wg_setup(vec![], "".into());
    state.save();
}