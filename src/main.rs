use std::env;
use crate::generator::main::generator_main;

mod generator;
mod citadel;
mod common;

const MODE: &str = "generator";
fn main() {
    let args: Vec<String> = env::args().collect();
    println!("args: {:?}", args);
    if MODE == "generator" {
        generator_main();
    } else {

    }
}
