#![feature(ip)]
#![feature(fn_traits)]
#![feature(trait_alias)]

pub mod state;
pub mod handshaker;
pub mod ui;
mod control_connection;
pub mod ui_utils;
pub mod generators;

use std::io::{stdout, Write};
use clap::{Parser, Subcommand};
use common::wireguard::try_extract_wstunnel;
use generator::do_nothing;
use crate::generators::{get_generator, get_repl_pos};
use crate::state::BackendState;
use crate::ui::ui_main::ui_main;

#[derive(Debug, Parser)]
#[command(name = "forcefield")]
#[command(about = "Create wireguard configurations", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Debug, Subcommand)]
enum Commands {
    #[command(arg_required_else_help = false, alias = "export", display_name = "export")]
    ExportGenerator {
        /// the target eg. x86 or i386 or aarch64
        target: Option<String>,
    },
}

pub fn main() {
    let mut state = BackendState::get();
    let args = Cli::try_parse().unwrap_or_else(|error| error.exit());

    match args.command {
        None => {}
        Some(Commands::ExportGenerator {target}) => {
            let typ = target.unwrap_or("x86".into());
            //println!("{}", get_repl_pos());
            stdout().write_all(get_generator(&state, &typ)).expect("could not write");
            return;
        }
    }

    let _ = try_extract_wstunnel();
    ui_main(&mut state).unwrap();
    let _ = state.create_wg_setup(vec![], "".into());
    state.save();

    do_nothing()
}