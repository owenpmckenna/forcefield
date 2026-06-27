use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use serde::{Deserialize, Serialize};
use crate::common::wireguard::Route;

#[derive(Serialize, Deserialize)]
pub enum Command {
    Heartbeat(usize),
    GetRoutes,
    GetIp,
    CreateWireguardPeer((String, (Ipv4Addr, Ipv6Addr), Option<SocketAddr>)),
    RemoveWireguardPeer(String),
    FireUDPWakeup(SocketAddr),
    FireUDPShutdown(SocketAddr),
    GetIPv6Addr,
    Kill,
}
#[derive(Serialize, Deserialize)]
pub enum Response {
    Heartbeat(usize),
    Routes(Vec<Route>),
    GetIp(Result<String, String>),
    Ipv6Addr(Option<Ipv6Addr>)
}