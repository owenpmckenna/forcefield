use crate::common::wireguard::generate_wireguard_keys;
use chacha20poly1305::aead::OsRng;
use chacha20poly1305::{Key, KeyInit, XChaCha20Poly1305};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
use std::{env, fs};

#[derive(Serialize, Deserialize)]
pub struct InitialConfig {
    pub config_key_bytes: Vec<u8>,
    #[serde(skip, default)]
    pub config_key: OnceLock<Key>,
    pub port: u16,
    pub wg_public: String,
    pub wg_private: String
}
static FILE: &str = "initconf.conf";
impl InitialConfig {
    pub fn get() -> InitialConfig {
        match fs::read_to_string(FILE) {
            Ok(it) => {
                serde_json::from_str(&it).unwrap()
            }
            Err(_) => {
                let file = &env::args().collect::<Vec<_>>()[0];
                let txt = file.split("_").last().unwrap();
                let port: u16 = match txt.parse() {
                    Ok(it) => {it},
                    Err(it) => {
                        panic!("error parsing {}: {}! Filename must be like forcefield_8080", txt, it)
                    }
                };
                let data = Self::init(port);
                Self::save(&data);
                data
            }
        }
    }
    fn save(conf: &InitialConfig) {
        let str = serde_json::to_string(conf).unwrap();
        let path = std::env::current_dir().unwrap().join(FILE);
        println!("writing config to file: {}", path.display());
        fs::write(FILE, str).unwrap();
    }
    pub fn delete() {
        fs::remove_file(FILE).unwrap();
    }
    pub fn get_key(&self) -> &Key {
        self.config_key.get_or_init(|| Key::clone_from_slice(&self.config_key_bytes))
    }
    fn init(port: u16) -> InitialConfig {
        let (wg_private, wg_public) = generate_wireguard_keys();
        let key: Key  = XChaCha20Poly1305::generate_key(OsRng);
        InitialConfig {
            config_key_bytes: key.to_vec(),
            config_key: OnceLock::new(),
            port,
            wg_private,
            wg_public
        }
    }
}