use std::net::{IpAddr, SocketAddr};
use serde::{Deserialize, Serialize};
use crate::common::wireguard::Route;

#[derive(Serialize, Deserialize)]
pub enum Command {
    Heartbeat(usize),
    GetRoutes,
    GetIp,
    CreateWireguardPeer((String, IpAddr, Option<SocketAddr>)),
    RemoveWireguardPeer(String),
    FireUDPWakeup(SocketAddr),
    FireUDPShutdown(SocketAddr),
    Kill,
}
#[derive(Serialize, Deserialize)]
pub enum Response {
    Heartbeat(usize),
    Routes(Vec<Route>),
    GetIpResponse(Result<String, String>)
}