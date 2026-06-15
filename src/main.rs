use std::ascii::AsciiExt;
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
const fn get_mode() -> Mode {
    let str = include_str!("../feature_flag");
    if str.eq_ignore_ascii_case("citadel\n") {
        Citadel
    } else if str.eq_ignore_ascii_case("generator\n") {
        Generator
    } else {
        panic!("feature_flag wrong!")
    }
}
pub const MODE: Mode = get_mode();
fn main() {
    let args: Vec<String> = env::args().collect();
    println!("args: {:?}", args);
    if MODE == Generator {
        generator_main();
    } else {
        citadel_main();
    }
}
