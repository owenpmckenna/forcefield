use std::net::{IpAddr, Ipv4Addr};
use ipnet::IpNet;
use crate::common::cmd::exec;
use crate::common::ip::Port;

//remember to use `ip route get 8.8.8.8` for test and `ip rule`
#[derive(Default)]
pub struct WireguardState {
    pub routes: Vec<Route>,
    pub wg_interfaces: Vec<Wireguard>
}
pub struct Route {
    pub addresses: Option<IpNet>,
    pub via: Option<IpAddr>,
    pub device: Option<String>,
    pub src: Option<IpAddr>
}
impl Route {
    pub fn new(addresses: Option<IpNet>, via: Option<IpAddr>, device: Option<String>, src: Option<IpAddr>) -> Self {
        Self { addresses, via, device, src }
    }
    fn move_self(&self, cmd: &str) {
        let addr = match &self.addresses {
            None => {"default"}
            Some(it) => {&format!("{}", it)}
        };
        let via = empty(&self.via, |it| format!(" via {}", it));
        let dev = empty(&self.device, |it| format!(" dev {}", it));
        let src = empty(&self.src, |it| format!(" src {}", it));
        exec(format!("ip route {} {}{}{}{}", cmd, addr, via, dev, src));
    }
    pub fn add_self(&self) {
        self.move_self("add")
    }
    pub fn remove_self(&self) {
        self.move_self("del")
    }
}
impl Drop for Route {
    fn drop(&mut self) {
        self.remove_self()
    }
}
fn empty<T, F>(me: &Option<T>, u: F) -> String where F: FnOnce(&T) -> String {
    if let Some(it) = me {
        u(it)
    } else {"".to_string()}
}
pub struct Wireguard {
    pub listen_port: Port,
    pub priv_key: String,
    pub pub_key: String,
    ///ex: "wg0"
    pub name: String,
    pub peers: Vec<WireguardPeer>
}
impl Wireguard {
    pub fn new(listen_port: Port, priv_key: String, pub_key: String, name: String, peers: Vec<WireguardPeer>) -> Self {
        Self { listen_port, priv_key, pub_key, name, peers }
    }
    pub fn spawn(&self) {
        exec(format!("ip link add dev {} type wireguard", self.name));
        exec(format!("wg set {} listen-port {} private-key {}", self.name, self.listen_port, self.priv_key));
        for peer in &self.peers {
            let addrs = peer.allowed_ips.iter()
                .map(IpNet::to_string)
                .collect::<Vec<String>>()
                .join(",");
            let endpoint = empty(&peer.endpoint, |(a, p)| format!(" endpoint {}:{}", a, p));
            exec(format!("wg set {} peer {} allowed-ips {}{}", self.name, peer.public_key, addrs, endpoint));
        }
        exec(format!("ip link set up dev {}", self.name));
    }
    pub fn kill(&self) {
        exec(format!("ip link set down dev {}", self.name));
        exec(format!("ip link delete {}", self.name));
    }
    pub fn first_peer(&self) -> &WireguardPeer {
        &self.peers[0]
    }
}
impl Drop for Wireguard {
    fn drop(&mut self) {
        self.kill();
    }
}
pub struct WireguardPeer {
    pub public_key: String,
    pub allowed_ips: Vec<IpNet>,
    pub endpoint: Option<(IpAddr, Port)>
}
impl WireguardPeer {
    pub fn new(public_key: String, allowed_ips: Vec<IpNet>, endpoint: Option<(IpAddr, Port)>) -> Self {
        Self { public_key, allowed_ips, endpoint }
    }
}