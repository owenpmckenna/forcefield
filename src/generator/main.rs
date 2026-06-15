use std::error::Error;
use std::io::{Read, Write};
use std::net::{IpAddr, TcpListener, TcpStream};
use std::str::FromStr;
use std::time::Duration;
use chacha20poly1305::aead::{AeadMut, OsRng, Nonce, Payload};
use chacha20poly1305::{AeadCore, ChaChaPoly1305, Key, KeyInit, XChaCha20Poly1305, XNonce};
use chacha20poly1305::aead::generic_array::{ArrayLength, GenericArray};
use chacha20poly1305::consts::U24;
use ipnet::IpNet;
use rsa::pkcs8::DecodePublicKey;
use rsa::{Pkcs1v15Encrypt, RsaPrivateKey, RsaPublicKey};
use crate::common::cmd::exec;
use crate::common::commands::Command;
use crate::common::errors::FFError;
use crate::common::setup_handshake::{read_packet, write_packet, ConfigMessage};
use crate::common::wireguard::{get_default_route, Wireguard, WireguardPeer};
use crate::generator::config::Config;
use crate::generator::init_config::InitialConfig;

static PUB_KEY_TEXT: &str = include_str!("../../key/public.pem");
pub fn generator_main() {
    if let Some(conf) = Config::get()  {
        run(conf); return;
    }
    let config = InitialConfig::get();
    let listener = TcpListener::bind(format!("0.0.0.0:{}", config.port)).unwrap();
    let running = true;
    let citadel_key = RsaPublicKey::from_public_key_pem(PUB_KEY_TEXT).unwrap();
    println!("encrypting keys... lengths {} and {}", config.config_key_bytes.len(), config.wg_public.as_bytes().len());
    let our_wg_key = citadel_key.encrypt(&mut rand::rng(), Pkcs1v15Encrypt,
                                      config.wg_public.as_bytes()).unwrap();
    let our_key = citadel_key.encrypt(&mut rand::rng(), Pkcs1v15Encrypt,
                                      &config.config_key_bytes).unwrap();
    println!("waiting for initial connection...");
    while running {
        match attempt_run(&listener, &our_key, &our_wg_key, config.get_key()) {
            Ok(it) => {
                drop(listener);
                let config = Config::new(config, it);
                InitialConfig::delete();
                config.save();
                run(config);
                return;
            }
            Err(it) => {
                println!("got error waiting for settings: {}", it);
            }
        }
    }
}
fn attempt_run(listener: &TcpListener, our_key: &Vec<u8>, our_wg_key: &Vec<u8>, key: &Key) -> Result<ConfigMessage, Box<dyn Error>> {
    let (mut stream, addr) = listener.accept()?;
    stream.set_read_timeout(Some(Duration::new(15, 0)))?;
    println!("got connection from addr {}", addr);
    //write the u64 size, then our public key, encrypted with the citadel public key
    write_packet(&mut stream, our_key)?;
    write_packet(&mut stream, our_wg_key)?;
    println!("wrote packets");

    let nonce_read_buf = read_packet(&mut stream)?;
    let read_buf = read_packet(&mut stream)?;
    let mut cipher = XChaCha20Poly1305::new(key);
    let nonce: GenericArray<u8, U24> = GenericArray::clone_from_slice(nonce_read_buf.as_slice()); // 192-bits; unique per message
    let read_buf: Vec<u8> = cipher.decrypt(&nonce, Payload::from(read_buf.as_slice()))
        .map_err(|it| Box::new(FFError::CipherError(it)))?;
    let out: ConfigMessage = serde_json::from_slice(&read_buf)?;
    println!("read and decrypted config packet. Id: {}, ip: {}", out.server_id, out.server_ip);
    Ok(out)
}
fn run(config: Config) {
    exec(format!("iptables -t nat -A POSTROUTING -o {} -j MASQUERADE", get_default_route().device.unwrap()));
    let peers = vec![WireguardPeer::new(config.citadel_wg_pub.clone(), vec![/*"10.68.0.0/16".parse().unwrap(),*/ "0.0.0.0/0".parse().unwrap()], None)];
    let wg = Wireguard::new(config.port, config.gen_wg_priv.clone(), config.gen_wg_pub.clone(), "uwu0".to_string(), IpAddr::from_str(&config.server_ip).unwrap(), peers);
    wg.spawn();
    let mut cipher = XChaCha20Poly1305::new(config.get_config_key());
    let listener = TcpListener::bind(format!("{}:{}", config.server_ip, config.port + 1)).unwrap();
    loop {
        //these are trusted, I think
        let (mut socket, addr) = listener.accept().unwrap();
        loop {
            let packet = if let Ok(it) = read_packet(&mut socket) {it} else {break};
            let nonce = &packet[0..24];
            let text = &packet[24..];
            let decrypted = cipher.decrypt(XNonce::from_slice(nonce), text);
            let decrypted = if let Ok(it) = decrypted {it} else {break};
            let cmd: Command = serde_json::from_slice(&decrypted).unwrap();
            match cmd {
                Command::Kill => {
                    wg.kill();
                    return;
                }
            }
        }
    }
}