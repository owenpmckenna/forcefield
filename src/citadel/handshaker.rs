use crate::citadel::state::BackendState;
use crate::common::errors::{FFError, FFResult};
use crate::common::ip::Port;
use crate::common::setup_handshake::{read_packet, write_packet, ConfigMessage};
use chacha20poly1305::aead::{Aead, OsRng};
use chacha20poly1305::{AeadCore, Key, KeyInit, XChaCha20Poly1305};
use rsa::pkcs8::DecodePrivateKey;
use rsa::{Pkcs1v15Encrypt, RsaPrivateKey};
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, SocketAddr, TcpStream};
use std::sync::OnceLock;
use std::time::Duration;

static PRIV_KEY_TEXT: &str = include_str!("../../key/private_pkcs1.pem");
#[derive(Serialize, Deserialize, Clone)]
pub struct Generator {
    pub id: String,
    pub pub_ip: IpAddr,
    pub pub_port: Port,
    pub internal_ip: IpAddr,
    #[serde(skip, default)]
    config_key: OnceLock<Key>,
    pub config_key_bytes: Vec<u8>,
    pub wg_public_key: String,
    pub description: Option<String>
}
impl Generator {
    pub fn get_config_key(&self) -> Key {
        self.config_key.get_or_init(|| Key::clone_from_slice(&self.config_key_bytes)).clone()
    }
    pub fn get_cipher(&self) -> XChaCha20Poly1305 {
        let key: &Key = self.config_key.get_or_init(|| Key::clone_from_slice(&self.config_key_bytes));
        XChaCha20Poly1305::new(key)
    }
    pub fn connect_to_generator(ip: String, state: &mut BackendState) -> FFResult<Generator> {
        let our_private = RsaPrivateKey::from_pkcs8_pem(PRIV_KEY_TEXT)?;
        let ip_addr: SocketAddr = ip.parse()?;
        let mut conn = TcpStream::connect_timeout(
            &ip_addr,
            Duration::new(15, 0)
        )?;
        conn.set_read_timeout(Some(Duration::new(15, 0)))?;
        conn.set_write_timeout(Some(Duration::new(15, 0)))?;

        //they write their config key, then their public wg key
        let their_config_key = read_packet(&mut conn)?;
        let twgc = read_packet(&mut conn)?;
        let their_config_key = our_private.decrypt(Pkcs1v15Encrypt, &their_config_key)?;
        let twgc = our_private.decrypt(Pkcs1v15Encrypt, &twgc)?;

        //we turn their config key into a useful cipher struct, then use that to send configs.
        let config_key: Key = Key::clone_from_slice(&their_config_key);
        let config_cipher = XChaCha20Poly1305::new(&config_key);
        let nonce = XChaCha20Poly1305::generate_nonce(OsRng);

        let (ip, str) = state.next_id()?;
        let msg = ConfigMessage::new(str.clone(), ip.to_string(), ip_addr.port(), state.our_wg_pub.clone());
        let bytes = config_cipher.encrypt(&nonce, serde_json::to_string(&msg)?.as_bytes())
            .map_err(|it| Box::new(FFError::CipherError(it)))?;
        write_packet(&mut conn, &nonce.as_slice())?;
        write_packet(&mut conn, &bytes)?;

        Ok(Generator {
            id: str,
            pub_ip: ip_addr.ip(),
            pub_port: ip_addr.port(),
            internal_ip: IpAddr::V4(ip),
            config_key: OnceLock::new(),
            config_key_bytes: config_key.to_vec(),
            wg_public_key: String::from_utf8(twgc)?,
            description: None
        })
    }
}