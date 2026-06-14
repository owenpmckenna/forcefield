use std::env;
use crate::citadel::main::citadel_main;
use crate::generator::main::generator_main;
use crate::Mode::{Citadel, Generator};

mod generator;
mod citadel;
mod common;

#[derive(Eq, PartialEq)]
pub enum Mode {
    Generator,
    Citadel
}
pub const MODE: Mode = Citadel;
fn main() {
    let args: Vec<String> = env::args().collect();
    println!("args: {:?}", args);
    if MODE == Generator {
        generator_main();
    } else {
        citadel_main();
    }
}
