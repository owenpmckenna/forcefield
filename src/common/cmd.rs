use crate::{Mode, MODE};
use std::process::Command;

pub fn exec(cmd: String) -> String {
    if MODE == Mode::Generator {
        println!("executing command... `{}`", cmd)
    }
    let out = Command::new("sh")
        .arg("-c")
        .arg(cmd)
        .output()
        .unwrap();
    let err = String::from_utf8_lossy(&out.stderr).to_string();
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    format!("{}{}", err, stdout)
}