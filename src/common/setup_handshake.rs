use crate::common::errors::FFResult;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::io::{Read, Write};
use std::net::TcpStream;
use chacha20poly1305::aead::{Aead, OsRng};
use chacha20poly1305::{AeadCore, XChaCha20Poly1305, XNonce};
use crate::common::errors::FFError::CipherError;

#[derive(Serialize, Deserialize)]
pub struct ConfigMessage {
    pub server_id: String,
    pub server_ip: String,
    pub port: u16,
    pub config_port: u16,
    pub citadel_wg_pub: String,
}
impl ConfigMessage {
    pub fn new(server_id: String, server_ip: String, port: u16, citadel_wg_pub: String) -> ConfigMessage {
        ConfigMessage {
            server_id,
            server_ip,
            port,
            config_port: port + 1,
            citadel_wg_pub,
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
    stream.write_all(&(data.len() as u64).to_le_bytes())?;
    stream.write_all(data)?;
    Ok(())
}
pub fn write_encrypted_data(stream: &mut TcpStream, cipher: &XChaCha20Poly1305, data: &[u8]) -> FFResult<()> {
    let nonce = XChaCha20Poly1305::generate_nonce(OsRng);
    let ciphertext = cipher.encrypt(&nonce, data)
        .map_err(|it| CipherError(it))?;
    write_packet(stream, &nonce)?;
    write_packet(stream, &ciphertext)?;
    Ok(())
}
pub fn read_encrypted_data(stream: &mut TcpStream, cipher: &XChaCha20Poly1305) -> FFResult<Vec<u8>> {
    let nonce_data = read_packet(stream)?;
    let nonce = XNonce::clone_from_slice(&nonce_data);
    let ciphertext = read_packet(stream)?;
    Ok(cipher.decrypt(&nonce, ciphertext.as_slice())
        .map_err(|it| CipherError(it))?)
}
