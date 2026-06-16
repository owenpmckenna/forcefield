#![feature(fn_traits)]

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
    #[cfg(generator)]
    return Mode::Generator;
    #[cfg(citadel)]
    return Mode::Citadel;
    Citadel
}
pub const MODE: Mode = get_mode();
fn main() {
    if MODE == Generator {
        generator_main();
    } else {
        citadel_main();
    }
}
