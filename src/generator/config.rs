use crate::common::setup_handshake::ConfigMessage;
use crate::generator::init_config::InitialConfig;
use chacha20poly1305::Key;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
use std::fs;

#[derive(Deserialize, Serialize)]
pub struct Config {
    ///8 char id string
    pub server_id: String,
    ///in 10.69.0.0/16, addr of local wireguard
    pub server_ip: String,
    #[serde(skip, default)]
    config_key: OnceLock<Key>,
    pub config_key_bytes: Vec<u8>,
    pub port: u16,
    pub gen_wg_pub: String,
    pub gen_wg_priv: String,
    pub citadel_wg_pub: String,
}
static FILE: &str = "conf.conf";
impl Config {
    pub fn get_config_key(&self) -> &Key {
        self.config_key.get_or_init(|| *Key::from_slice(&self.config_key_bytes))
    }
    pub fn new(initial_config: InitialConfig, cfg_msg: ConfigMessage) -> Config {
        Config {
            server_id: cfg_msg.server_id,
            server_ip: cfg_msg.server_ip,
            config_key: OnceLock::new(),
            config_key_bytes: cfg_msg.config_key_bytes,
            port: cfg_msg.port,
            gen_wg_pub: initial_config.wg_public,
            gen_wg_priv: initial_config.wg_private,
            citadel_wg_pub: cfg_msg.citadel_wg_pub,
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
}