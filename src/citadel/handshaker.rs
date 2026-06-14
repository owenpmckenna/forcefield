use crate::citadel::state::BackendState;
use crate::common::errors::FFResult;
use crate::common::setup_handshake::{read_packet, write_packet, ConfigMessage};
use chacha20poly1305::{Key, KeyInit};
use rsa::pkcs1::DecodeRsaPrivateKey;
use rsa::pkcs8::{DecodePrivateKey, DecodePublicKey};
use rsa::{Pkcs1v15Encrypt, RsaPrivateKey, RsaPublicKey};
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, SocketAddr, TcpStream};
use std::sync::OnceLock;
use std::time::Duration;
use crate::common::ip::Port;

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
    pub fn connect_to_generator(ip: String, state: &mut BackendState) -> FFResult<Generator> {
        let our_private = RsaPrivateKey::from_pkcs8_pem(PRIV_KEY_TEXT)?;
        let ip_addr: SocketAddr = ip.parse()?;
        let mut conn = TcpStream::connect_timeout(
            &ip_addr,
            Duration::new(15, 0)
        )?;
        conn.set_read_timeout(Some(Duration::new(15, 0)))?;
        conn.set_write_timeout(Some(Duration::new(15, 0)))?;

        //they write their public key, then their public wg key
        let their_public_key = read_packet(&mut conn)?;
        let twgc = read_packet(&mut conn)?;
        let their_public_key = our_private.decrypt(Pkcs1v15Encrypt, &their_public_key)?;
        let twgc = our_private.decrypt(Pkcs1v15Encrypt, &twgc)?;

        //we turn their public key into a useful cipher struct, then use that to send configs.
        let gen_public_key = RsaPublicKey::from_public_key_pem(&String::from_utf8(their_public_key)?)?;

        let (ip, str) = state.next_id()?;
        let msg = ConfigMessage::new(str.clone(), ip.to_string(), ip_addr.port(), state.our_wg_pub.clone());
        let key_copy = msg.config_key_bytes.clone();
        let bytes = gen_public_key.encrypt(&mut rand::rng(), Pkcs1v15Encrypt, serde_json::to_string(&msg)?.as_bytes())?;
        write_packet(&mut conn, &bytes)?;

        Ok(Generator {
            id: str,
            pub_ip: ip_addr.ip(),
            pub_port: ip_addr.port(),
            internal_ip: IpAddr::V4(ip),
            config_key: OnceLock::new(),
            config_key_bytes: key_copy,
            wg_public_key: String::from_utf8(twgc)?,
            description: None
        })
    }
}