use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use serde::{Deserialize, Serialize};
use crate::common::wireguard::{EndpointAddr, Route};

#[derive(Serialize, Deserialize)]
pub enum Command {
    Heartbeat(usize),
    GetRoutes,
    GetIp,
    CreateWireguardPeer((String, (Ipv4Addr, Ipv6Addr), EndpointAddr)),
    RemoveWireguardPeer(String),
    FireUDPWakeup(SocketAddr),
    FireUDPShutdown(SocketAddr),
    GetIPv6Addr,
    RunCommand(String),
    Kill,
}
#[derive(Serialize, Deserialize)]
pub enum Response {
    Heartbeat(usize),
    Routes(Vec<Route>),
    GetIp(Result<String, String>),
    CommandResponse(String),
    Ipv6Addr(Option<Ipv6Addr>)
}