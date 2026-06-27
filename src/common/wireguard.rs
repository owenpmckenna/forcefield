use std::fmt::{Debug, Display, Formatter};
use std::fs;
use crate::common::cmd::exec;
use crate::common::ip::Port;//Port is type alias for u16
use ipnet::{IpNet, Ipv6Net};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::str::FromStr;

//remember to use `ip route get 8.8.8.8` for test and `ip rule`
#[derive(Clone)]
pub struct WireguardState {
    pub routes: Vec<Route>,
    pub wg_interfaces: Vec<Wireguard>,
    pub old_default_route_v4: Option<Route>,
    pub old_default_route_v6: Option<Route>,
}

impl WireguardState {
    pub(crate) fn down(&self) {
        for route in &self.routes {
            route.remove_self();
        }
        for wg in &self.wg_interfaces {
            wg.kill();
        }
        let reset_default = Self::is_everything(&self.routes.last().unwrap().addresses);
        //if the last route is for everything, then we deleted the default route last time
        if reset_default {
            if let Some(route) = &self.old_default_route_v4 {
                route.add_self()
            }
            if let Some(route) = &self.old_default_route_v6 {
                route.add_self()
            }
        }
    }
    fn is_everything(addresses: &IpNet) -> bool {
        addresses.prefix_len() == 0
    }
    fn is_everything_opt(addresses: &Option<IpNet>) -> bool {
        if let Some(it) = addresses {
            Self::is_everything(it)
        } else {true}
    }
}

impl WireguardState {
    pub fn new(routes: Vec<Route>, wg_interfaces: Vec<Wireguard>, old_default_route_v4: Option<Route>, old_default_route_v6: Option<Route>) -> WireguardState {
        WireguardState {routes, wg_interfaces, old_default_route_v4, old_default_route_v6}
    }
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Route {
    pub addresses: IpNet,
    pub via: Option<IpAddr>,
    pub device: Option<String>,
    pub src: Option<IpAddr>,
    metric: Option<u32>,
    proto: Option<String>
}
impl Display for Route {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let addr = match self.addresses.prefix_len() == 0 {
            true => {"default"}
            false => {&format!("{}", self.addresses)}
        };
        let via = empty(&self.via, |it| format!(" via {}", it));
        let dev = empty(&self.device, |it| format!(" dev {}", it));
        let proto = empty(&self.proto, |it| format!(" proto {}", it));
        let src = empty(&self.src, |it| format!(" src {}", it));
        let metric = empty(&self.proto, |it| format!(" metric {}", it));
        write!(f, "{}{}{}{}{}{}", addr, via, dev, proto, src, metric)
    }
}
impl Debug for Route { fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result { Display::fmt(&self, f) } }
impl Route {
    pub fn new(addresses: IpNet, via: Option<IpAddr>, device: Option<String>, src: Option<IpAddr>) -> Self {
        Self { addresses, via, device, src, metric: None, proto: None }
    }
    pub fn new_full(addresses: IpNet, via: Option<IpAddr>, device: Option<String>, src: Option<IpAddr>, proto: Option<String>, metric: Option<u32>) -> Self {
        Self { addresses, via, device, src, proto, metric}
    }
    fn move_self(&self, cmd: &str) {
        let q6 = if let IpNet::V6(it) = self.addresses {
            " -6"
        } else {""};
        let resp = exec(format!("ip{} route {} {}", q6, cmd, self));
        if resp.contains("RTNETLINK answers: File exists") && cmd.eq("add") {
            self.move_self("replace");
        }
    }
    pub fn add_self(&self) {
        self.move_self("add")
    }
    pub fn remove_self(&self) {
        self.move_self("del")
    }
}
fn empty<T, F>(me: &Option<T>, u: F) -> String where F: FnOnce(&T) -> String {
    if let Some(it) = me {
        u(it)
    } else {"".to_string()}
}
#[derive(Clone)]
pub struct Wireguard {
    pub listen_port: Port,
    pub priv_key: String,
    pub pub_key: String,
    ///ex: "wg0"
    pub name: String,
    pub local_ipv4: Ipv4Addr,
    pub local_ipv6: Ipv6Addr,
    pub peers: Vec<WireguardPeer>
}
impl Wireguard {
    pub fn new(listen_port: Port, priv_key: String, pub_key: String, name: String, local_ipv4: Ipv4Addr, local_ipv6: Ipv6Addr, peers: Vec<WireguardPeer>) -> Self {
        Self { listen_port, priv_key, pub_key, name, local_ipv4, local_ipv6, peers }
    }
    pub fn spawn(&self) {
        exec(format!("ip link add dev {} type wireguard", self.name));
        exec(format!("ip address add dev {} {}/16", self.name, self.local_ipv4));
        exec(format!("ip address add dev {} {}/48", self.name, self.local_ipv6));
        exec(format!("wg set {} listen-port {}", self.name, self.listen_port));
        fs::write("TMPKEY", &self.priv_key).unwrap();
        exec(format!("wg set {} private-key TMPKEY", self.name));
        fs::remove_file("TMPKEY").unwrap();
        for peer in &self.peers {
            self.spawn_peer(peer)
        }
        exec(format!("ip link set up dev {}", self.name));
    }
    pub fn spawn_peer(&self, peer: &WireguardPeer) {
        let addrs = peer.allowed_ips.iter()
            .map(IpNet::to_string)
            .collect::<Vec<String>>()
            .join(",");
        let endpoint = empty(&peer.endpoint, |a| format!(" endpoint {}:{}", a.ip(), a.port()));
        exec(format!("wg set {} peer {} allowed-ips {}{}", self.name, peer.public_key, addrs, endpoint));
    }
    pub fn spawn_peer_add(&mut self, peer: WireguardPeer, pos: usize) {
        let addrs = peer.allowed_ips.iter()
            .map(IpNet::to_string)
            .collect::<Vec<String>>()
            .join(",");
        let endpoint = empty(&peer.endpoint, |a| format!(" endpoint {}:{}", a.ip(), a.port()));
        exec(format!("wg set {} peer {} allowed-ips {}{}", self.name, peer.public_key, addrs, endpoint));
        self.peers.insert(pos, peer);
    }
    pub fn remove_peer(&mut self, wg_key: &str) -> WireguardPeer {
        let pos = self.peers.iter().position(|it| it.public_key.eq(wg_key))
            .expect("could not find peer???");
        exec(format!("wg set {} peer {} remove", self.name, wg_key));
        self.peers.remove(pos)
    }
    pub fn try_remove_allowed_ip_from_peer(&mut self, peer: usize, allowed_ip: IpNet) {
        let wgp: &WireguardPeer = &self.peers[peer];
        let id = wgp.allowed_ips.iter().position(|it| it.eq(&allowed_ip));
        if let Some(id) = id {
            self.remove_allowed_ip_from_peer(peer, id);
        }
    }
    pub fn remove_allowed_ip_from_peer(&mut self, peer_i: usize, allowed_ip: usize) {
        let mut peer = self.peers.remove(peer_i);
        exec(format!("wg set {} peer {} remove", self.name, peer.public_key));
        peer.allowed_ips.remove(allowed_ip);
        self.spawn_peer_add(peer, peer_i);
    }
    pub fn add_allowed_ip_to_peer(&mut self, peer_i: usize, allowed_ip: IpNet) {
        let mut peer = self.peers.remove(peer_i);
        exec(format!("wg set {} peer {} remove", self.name, peer.public_key));
        peer.allowed_ips.push(allowed_ip);
        self.spawn_peer_add(peer, peer_i);
    }
    pub fn kill(&self) {
        exec(format!("ip link set down dev {}", self.name));
        exec(format!("ip link delete {}", self.name));
    }
    pub fn first_peer(&self) -> &WireguardPeer {
        &self.peers[0]
    }
}
#[derive(Clone)]
pub struct WireguardPeer {
    pub public_key: String,
    pub allowed_ips: Vec<IpNet>,
    pub endpoint: Option<SocketAddr>
}
impl WireguardPeer {
    pub fn new(public_key: String, allowed_ips: Vec<IpNet>, endpoint: Option<SocketAddr>) -> Self {
        Self { public_key, allowed_ips, endpoint }
    }
}

use base64::{engine::general_purpose::STANDARD, Engine};
use crossterm::style::Stylize;
use regex_lite::Regex;
use serde::{Deserialize, Serialize};
use x25519_dalek::PublicKey;
use x25519_dalek::StaticSecret;

pub fn generate_wireguard_keys() -> (String, String) {
    let private_key = StaticSecret::random_from_rng(&mut rand::rng());
    let public_key = PublicKey::from(&private_key);

    let private_b64 = STANDARD.encode(private_key.to_bytes());
    let public_b64 = STANDARD.encode(public_key.to_bytes());

    (private_b64, public_b64)
}
pub fn get_routes() -> Vec<Route> {
    let v4s = get_routes_(true);
    let v6s = get_routes_(false);
    let mut all = Vec::with_capacity(v4s.len() + v6s.len());
    v4s.into_iter().for_each(|it| all.push(it));
    v6s.into_iter().for_each(|it| all.push(it));
    all
}
fn get_routes_(v4: bool) -> Vec<Route> {
    let out = if v4 {
        exec("ip route".into())
    } else {exec("ip -6 route".into())};
    out.split("\n").map(|it| it.trim())
        .filter(|it| !it.is_empty())
        .map(|it| {
            let via_regex = Regex::new(r"via ([^ ]+)").unwrap();
            let dev_regex = Regex::new(r"dev ([^ ]+)").unwrap();
            let pro_regex = Regex::new(r"proto ([^ ]+)").unwrap();
            let src_regex = Regex::new(r"src ([^ ]+)").unwrap();
            let met_regex = Regex::new(r"metric ([^ ]+)").unwrap();
            let via = via_regex.captures(it)
                .map(|it| IpAddr::from_str(&it[1]).unwrap());
            let dev = dev_regex.captures(it)
                .map(|it| it[1].to_string());
            let proto = pro_regex.captures(it)
                .map(|it| it[1].to_string());
            let src = src_regex.captures(it)
                .map(|it| IpAddr::from_str(&it[1]).unwrap());
            let metric = met_regex.captures(it)
                .map(|it| it[1].parse::<u32>().unwrap());
            //default via 192.168.0.1 dev wlan0 proto dhcp src 192.168.0.31 metric 1024
            let first = it.split_once(" ").unwrap().0;
            let addr: IpNet = if first.eq("default") {
                if v4 {
                    "0.0.0.0/0".parse().unwrap()
                } else {"::/0".parse().unwrap()}
            } else {
                //sometimes it's like: 192.168.0.1 via ... or 192.168.0.0/24 via ...
                if let Ok(it) = first.parse() {
                    it
                } else {match first.parse::<IpAddr>().unwrap() {
                    IpAddr::V4(it) => {IpNet::new_assert(it.into(), 32)}
                    IpAddr::V6(it) => {IpNet::new_assert(it.into(), 128)}
                }}
            };
            Route::new_full(addr, via, dev, src, proto, metric)
        })
        .collect::<Vec<Route>>()
}
pub fn get_default_route_v4(routes: &[Route]) -> Option<&Route> {
    routes.iter().find(|it|
        if let IpNet::V4(v4) = it.addresses {v4.prefix_len() == 0} else {false}
    )
}
pub fn get_default_route_v6(routes: &[Route]) -> Option<&Route> {
    routes.iter().find(|it|
        if let IpNet::V6(v6) = it.addresses {v6.prefix_len() == 0} else {false}
    )
}
///returns true if the device's route table contains a non-default route for this address
pub fn has_route_for_ip(ip: IpAddr) -> bool {
    has_route_for_ip_no_lookup(ip, &get_routes())
}
pub fn has_route_for_ip_no_lookup(ip: IpAddr, routes: &Vec<Route>) -> bool {
    //panic!("looking for ip: {}\nin routes: {:?}", ip, routes);
    routes.iter().find(|it| it.addresses.contains(&ip)).is_some()
}
/*2: ens3: <BROADCAST,MULTICAST,UP,LOWER_UP> mtu 9000 qdisc pfifo_fast state UP group default qlen 1000
    altname enp0s3
    inet6 2603:c020:4019:22d3:0:a81b:3e0:f35b/128 scope global dynamic noprefixroute
       valid_lft 82911sec preferred_lft 79311sec
    inet6 fe80::17ff:fe0f:e712/64 scope link
       valid_lft forever preferred_lft forever*/
pub fn get_pub_ipv6_addr(device: &str) -> Option<Ipv6Addr> {
    //inet6 2605:8600:5c0:b2ef:fa54:f6ff:fec6:3ebe/64 scope global
    let output = exec(format!("ip -6 addr show dev {}", device));
    let addr_regex = Regex::new(r"inet6 ([a-z0-9:/]+) scope global").unwrap();
    let addr = addr_regex.captures(&output)
        .map(|it| Ipv6Net::from_str(&it[1]).unwrap())?;
    if addr.prefix_len() == 128 {
        Some(addr.addr())
    } else {None}
}