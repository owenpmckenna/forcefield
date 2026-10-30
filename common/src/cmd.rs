use std::process::Command;

pub fn exec(cmd: String) -> (i32, String) {
    let out = Command::new("sh")
        .arg("-c")
        .arg(cmd)
        .output()
        .unwrap();
    let err = String::from_utf8_lossy(&out.stderr).to_string();
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    (out.status.code().unwrap_or(-776) , format!("{}{}", err, stdout))
}