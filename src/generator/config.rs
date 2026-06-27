use crate::common::setup_handshake::ConfigMessage;
use crate::common::wireguard::Wireguard;
use crate::generator::init_config::InitialConfig;
use chacha20poly1305::Key;
use serde::{Deserialize, Serialize};
use std::fs;
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::OnceLock;

#[derive(Deserialize, Serialize)]
pub struct Config {
    ///8 char id string
    pub server_id: String,
    ///in 10.69.0.0/16, addr of local wireguard
    pub server_ipv4: String,
    pub server_ipv6: String,
    #[serde(skip, default)]
    config_key: OnceLock<Key>,
    pub config_key_bytes: Vec<u8>,
    pub port: u16,
    pub config_port: u16,
    pub gen_wg_pub: String,
    pub gen_wg_priv: String,
    pub citadel_wg_pub: String,
    ///the Option<String> is actually an Option<SocketAddr>
    pub peers: Vec<(String, (Ipv4Addr, Ipv6Addr), Option<String>)>
}
static FILE: &str = "conf.conf";
impl Config {
    pub fn get_config_key(&self) -> &Key {
        self.config_key.get_or_init(|| *Key::from_slice(&self.config_key_bytes))
    }
    pub fn new(initial_config: InitialConfig, cfg_msg: ConfigMessage) -> Config {
        Config {
            server_id: cfg_msg.server_id,
            server_ipv4: cfg_msg.server_ipv4,
            server_ipv6: cfg_msg.server_ipv6,
            config_key: OnceLock::new(),
            config_key_bytes: initial_config.config_key_bytes,
            port: cfg_msg.port,
            config_port: cfg_msg.config_port,
            gen_wg_pub: initial_config.wg_public,
            gen_wg_priv: initial_config.wg_private,
            citadel_wg_pub: cfg_msg.citadel_wg_pub,
            peers: vec![]
        }
    }
    pub fn get() -> Option<Self> {
        match fs::read_to_string(FILE) {
            Ok(it) => {
                Some(serde_json::from_str(&it).unwrap())
            }
            Err(_) => {
                None
            }
        }
    }
    pub fn save(&self) {
        let str = serde_json::to_string(self).unwrap();
        fs::write(FILE, str).unwrap();
    }
    fn delete() {
        fs::remove_file(FILE).unwrap();
    }
    pub fn get_peers(&self) -> Vec<(String, (Ipv4Addr, Ipv6Addr), Option<SocketAddr>)> {
        self.peers.iter().map(|it| 
            (it.0.clone(), (it.1.0.clone(), it.1.1.clone()), it.2.as_ref().map(|it| it.parse().unwrap()))
        ).collect()
    }
    pub fn add_peer(&mut self, peer: (String, (Ipv4Addr, Ipv6Addr), Option<SocketAddr>)) {
        self.peers.push((peer.0, (peer.1.0, peer.1.1), peer.2.map(|it| it.to_string())))
    }
    pub fn get_citadel_peer_index(&self, wg: &Wireguard) -> usize {
        wg.peers.iter().position(|it| it.public_key.eq(&self.citadel_wg_pub)).unwrap()
    }
}