use std::borrow::Cow;
use std::process::Command;

pub fn exec(cmd: String) -> String {
    let cmd = Command::new("sh")
        .arg("-c")
        .arg(cmd)
        .spawn()
        .unwrap();
    let out = cmd.wait_with_output().unwrap();
    let err = String::from_utf8_lossy(&out.stderr);
    if !err.trim().is_empty() {
        println!("got output from cmd: {}", err)
    }
    String::from_utf8_lossy(&out.stdout).to_string()
}