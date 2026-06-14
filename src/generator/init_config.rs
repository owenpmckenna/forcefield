use crate::common::wireguard::generate_wireguard_keys;
use regex::Regex;
use rsa::pkcs1::LineEnding;
use rsa::pkcs8::{DecodePrivateKey, DecodePublicKey, EncodePrivateKey, EncodePublicKey};
use rsa::{RsaPrivateKey, RsaPublicKey};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
use std::{env, fs};

#[derive(Serialize, Deserialize)]
pub struct InitialConfig {
    pub conf_pub_key_int: String,
    pub conf_priv_key_int: String,
    #[serde(skip, default)]
    pub conf_pub_key: OnceLock<RsaPublicKey>,
    #[serde(skip, default)]
    pub conf_priv_key: OnceLock<RsaPrivateKey>,
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
                let pr = Regex::new("[0-9]+$").unwrap();
                let file = &env::args().collect::<Vec<_>>()[0];
                let port: u16 = pr.find(file).unwrap().as_str().parse().unwrap();
                let data = Self::init(port);
                Self::save(&data);
                data
            }
        }
    }
    fn save(conf: &InitialConfig) {
        let str = serde_json::to_string(conf).unwrap();
        fs::write(FILE, str).unwrap();
    }
    pub fn delete() {
        fs::remove_file(FILE).unwrap();
    }
    pub fn get_pub_key(&self) -> &RsaPublicKey {
        self.conf_pub_key.get_or_init(|| RsaPublicKey::from_public_key_pem(&self.conf_pub_key_int).unwrap())
    }
    pub fn get_priv_key(&self) -> &RsaPrivateKey {
        self.conf_priv_key.get_or_init(|| RsaPrivateKey::from_pkcs8_pem(&self.conf_priv_key_int).unwrap())
    }
    fn init(port: u16) -> InitialConfig {
        let privk = RsaPrivateKey::new(&mut rand::rng(), 4096).unwrap();
        let pubk = privk.as_public_key().clone();
        let (wg_private, wg_public) = generate_wireguard_keys();
        InitialConfig {
            conf_pub_key_int: pubk.to_public_key_pem(LineEnding::CRLF).unwrap(),
            conf_priv_key_int: privk.to_pkcs8_pem(LineEnding::CRLF).unwrap().to_string(),
            conf_pub_key: OnceLock::from(pubk),
            conf_priv_key: OnceLock::from(privk),
            port,
            wg_private,
            wg_public
        }
    }
}