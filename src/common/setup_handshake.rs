use std::error::Error;
use std::io::{Read, Write};
use std::net::TcpStream;
use chacha20poly1305::{KeyInit, XChaCha20Poly1305};
use chacha20poly1305::aead::OsRng;
use rand::rngs::StdRng;
use serde::{Deserialize, Serialize};
use crate::common::errors::FFResult;

#[derive(Serialize, Deserialize)]
pub struct ConfigMessage {
    pub server_id: String,
    pub server_ip: String,
    pub port: u16,
    pub citadel_wg_pub: String,
    pub config_key_bytes: Vec<u8>
}
impl ConfigMessage {
    pub fn new(server_id: String, server_ip: String, port: u16, citadel_wg_pub: String) -> ConfigMessage {
        let key = XChaCha20Poly1305::generate_key(OsRng);
        ConfigMessage {
            server_id,
            server_ip,
            port,
            citadel_wg_pub,
            config_key_bytes: key.to_vec()
        }
    }
}
pub fn read_packet(stream: &mut TcpStream) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut read_buf = [0u8; 64 / 8];
    stream.read_exact(&mut read_buf)?;
    let read_len = u64::from_le_bytes(read_buf);
    let mut read_buf = vec![0u8; read_len as usize];
    stream.read_exact(&mut read_buf)?;
    Ok(read_buf)
}
pub fn write_packet(stream: &mut TcpStream, data: &[u8]) -> FFResult<()> {
    stream.write(&(data.len() as u64).to_le_bytes())?;
    stream.write(&data)?;
    Ok(())
}