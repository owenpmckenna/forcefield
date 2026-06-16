use std::fs;
use crate::common::cmd::exec;
use crate::common::ip::Port;//Port is type alias for u16
use ipnet::IpNet;
use std::net::IpAddr;
use std::str::FromStr;

//remember to use `ip route get 8.8.8.8` for test and `ip rule`
#[derive(Clone)]
pub struct WireguardState {
    pub routes: Vec<Route>,
    pub wg_interfaces: Vec<Wireguard>,
    pub old_default_route: Route,
}

impl WireguardState {
    pub(crate) fn down(&self) {
        for route in &self.routes {
            route.remove_self();
        }
        for wg in &self.wg_interfaces {
            wg.kill();
        }
        let reset_default = Self::is_everything_opt(&self.routes.last().unwrap().addresses);
        //if the last route is for everything, then we deleted the default route last time
        if reset_default {
            self.old_default_route.add_self();
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
    pub fn new(routes: Vec<Route>, wg_interfaces: Vec<Wireguard>, old_default_route: Route) -> WireguardState {
        WireguardState {routes, wg_interfaces, old_default_route}
    }
}
#[derive(Clone)]
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
    pub local_ip: IpAddr,
    pub peers: Vec<WireguardPeer>
}
impl Wireguard {
    pub fn new(listen_port: Port, priv_key: String, pub_key: String, name: String, local_ip: IpAddr, peers: Vec<WireguardPeer>) -> Self {
        Self { listen_port, priv_key, pub_key, name, local_ip, peers }
    }
    pub fn spawn(&self) {
        exec(format!("ip link add dev {} type wireguard", self.name));
        exec(format!("ip address add dev {} {}/16", self.name, self.local_ip));
        exec(format!("wg set {} listen-port {}", self.name, self.listen_port));
        fs::write("TMPKEY", &self.priv_key).unwrap();
        exec(format!("wg set {} private-key TMPKEY", self.name));
        fs::remove_file("TMPKEY").unwrap();
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
#[derive(Clone)]
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

use base64::{engine::general_purpose::STANDARD, Engine};
use regex_lite::Regex;
use x25519_dalek::PublicKey;
use x25519_dalek::StaticSecret;

pub fn generate_wireguard_keys() -> (String, String) {
    let private_key = StaticSecret::random_from_rng(&mut rand::rng());
    let public_key = PublicKey::from(&private_key);

    let private_b64 = STANDARD.encode(private_key.to_bytes());
    let public_b64 = STANDARD.encode(public_key.to_bytes());

    (private_b64, public_b64)
}
pub fn get_default_route() -> Route {
    let out = exec("ip route".into());
    let routes = out.split("\n");
    let default_route = routes.collect::<Vec<_>>().first().unwrap().trim().to_string();
    //default via 192.168.0.1 dev wlan0 proto dhcp src 192.168.0.31 metric 1024
    let via_regex = Regex::new(r"via ([^ ]+)").unwrap();
    let dev_regex = Regex::new(r"dev ([^ ]+)").unwrap();
    let src_regex = Regex::new(r"src ([^ ]+)").unwrap();
    let via = via_regex.captures(&default_route)
        .map(|it| IpAddr::from_str(&it[1]).unwrap());
    let dev = dev_regex.captures(&default_route)
        .map(|it| it[1].to_string());
    let src = src_regex.captures(&default_route)
        .map(|it| IpAddr::from_str(&it[1]).unwrap());
    Route::new(None, via, dev, src)
}
