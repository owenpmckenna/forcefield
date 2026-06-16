use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub enum Command {
    Heartbeat(usize),
    Kill,
}
#[derive(Serialize, Deserialize)]
pub enum Response {
    Heartbeat(usize)
}