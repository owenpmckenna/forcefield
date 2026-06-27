use std::fmt::{Display, Formatter};
use crate::citadel::state::BackendState;
use crate::common::errors::{FFError, FFResult};
use crate::common::ip::Port;
use crate::common::setup_handshake::{read_packet, write_packet, ConfigMessage};
use chacha20poly1305::aead::{Aead, OsRng};
use chacha20poly1305::{AeadCore, Key, KeyInit, XChaCha20Poly1305};
use rsa::pkcs8::DecodePrivateKey;
use rsa::{Pkcs1v15Encrypt, RsaPrivateKey};
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, TcpStream};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;
use tui::text::Spans;
use crate::citadel::handshaker::Endpoint::{FromPeer, PublicEndpoint, ViaPeer};
use crate::common::wireguard::{has_route_for_ip_no_lookup, Route};

static PRIV_KEY_TEXT: &str = include_str!("../../key/private_pkcs1.pem");
#[derive(Serialize, Deserialize, Clone, Eq, PartialEq, Debug)]
pub enum Endpoint {
    PublicEndpoint(SocketAddr),
    ///this means that the generator in question (hehe) has connected to the peer with the id in the string
    ///from behind NAT or something. It can be reached by its internal ip from the network of the peer
    ViaPeer(String),
    ///this means the generator can be connected to by the ip, but only when the last hop is from the specified
    ///endpoint. If it's None, then it can be reached by that IP from us, potentially
    FromPeer(SocketAddr, Option<String>)
}
impl Endpoint {
    pub fn from_initial_ip(ip: SocketAddr, lg: Option<&Generator>) -> Self {
        match ip.ip().is_global() {
            true => {PublicEndpoint(ip)}
            false => {FromPeer(ip, lg.map(|it| it.id.clone()))}
        }
    }
}

impl Display for Endpoint {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            PublicEndpoint(ip) => {write!(f, "{}", ip.ip())}
            ViaPeer(it) => {write!(f, "via {}", it)}
            FromPeer(ip, id) => {write!(f, "{} via {}", ip, id.clone().unwrap_or("local network".into()))}
        }
    }
}
#[derive(Serialize, Deserialize, Clone)]
pub struct Generator {
    pub id: String,
    pub endpoints: Vec<Endpoint>,
    pub wg_port: Port,
    pub config_port: Port,
    pub internal_ip_v4: Ipv4Addr,
    pub internal_ip_v6: Ipv6Addr,
    #[serde(skip, default)]
    config_key: OnceLock<Key>,
    pub config_key_bytes: Vec<u8>,
    pub wg_public_key: String,
    pub description: Option<String>,
    #[serde(skip, default)]
    pub probable_routes: Arc<Mutex<Vec<Route>>>
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

        let (ipv4, ipv6, str) = state.next_id()?;
        let msg = ConfigMessage::new(str.clone(), ipv4.to_string(), ipv6.to_string(), ip_addr.port(), state.our_wg_pub.clone());
        let bytes = config_cipher.encrypt(&nonce, serde_json::to_string(&msg)?.as_bytes())
            .map_err(|it| Box::new(FFError::CipherError(it)))?;
        write_packet(&mut conn, &nonce.as_slice())?;
        write_packet(&mut conn, &bytes)?;

        let lg = if let Some(id) = state.current_wg_ids.last() {
            state.get_by_id(id)
        } else {None};
        Ok(Generator {
            id: str,
            wg_port: ip_addr.port(),
            config_port: ip_addr.port() + 1,
            internal_ip_v4: ipv4,
            internal_ip_v6: ipv6,
            endpoints: vec![Endpoint::from_initial_ip(ip_addr, lg)],
            config_key: OnceLock::new(),
            config_key_bytes: config_key.to_vec(),
            wg_public_key: String::from_utf8(twgc)?,
            description: None,
            probable_routes: Arc::new(Mutex::new(vec![]))
        })
    }
    ///yeah so... pass in Err(id) if you don't want just a route, but a logical next route in a vpn chain
    pub fn find_best_endpoint(&self, routes: &Vec<Route>, connected: Result<&Vec<String>, Option<String>>, pub_ip: Option<(Option<Ipv6Addr>, Option<Ipv4Addr>)>) -> Option<&Endpoint> {
        match connected {
            //we are connected in a chain, and want to find the best way to talk to this generator
            Ok(connected) => {
                self.find_best_endpoint_force_lc(routes, connected, connected.last().cloned(), pub_ip)
            }
            //We want to know the best way to this generator, from the end of a vpn chain
            Err(Some(it)) => {
                self.find_best_endpoint_force_lc(routes, &vec![it.clone()], Some(it), pub_ip)
            }
            //we want to know the best way to this generator, from the beginning of a vpn chain
            Err(None) => {
                self.find_best_endpoint_force_lc(routes, &vec![], None, pub_ip)
            }
        }
    }
    fn find_best_endpoint_force_lc(&self, routes: &Vec<Route>, connected: &Vec<String>, last_connected: Option<String>, pub_ip: Option<(Option<Ipv6Addr>, Option<Ipv4Addr>)>) -> Option<&Endpoint> {
        //prefer routes direct from peer, they're fastest and simplest
        if let Some(ep) = self.endpoints.iter()
            .find_map(|it| if let FromPeer(ip, id) = it && id.eq(&last_connected) {
                if last_connected == None {//if we haven't done any connections yet, check if we're on the right network
                    if has_route_for_ip_no_lookup(ip.ip(), routes) {
                        Some(it)
                    } else {None}
                } else {
                    Some(it)
                }
            } else {None}) {
            return Some(ep);
        }

        for connected in connected {
            //ok, so if there's a like, mid level generator in the route with a peer connection to
            //our target, use it, it'll be faster than the public endpoint
            if let Some(end) =  self.endpoints.iter().find_map(|it|
                if let ViaPeer(id) = it && id.eq(connected) {Some(it)} else { None }
            ) {
                return Some(end);
            }
        }

        if let Some((ep, _)) = self.endpoints.iter()
            .filter_map(|it| if let PublicEndpoint(ad) = &it {Some((it, ad))} else {None})
            .find(|it| it.1.is_ipv4()){
            if let Some(pub_ip) = pub_ip {
                if pub_ip.1.is_some() {
                    return Some(ep)
                }
            } else {return Some(ep)}
        }
        if let Some((ep, _)) = self.endpoints.iter()
            .filter_map(|it| {if let PublicEndpoint(ad) = &it {Some((it, ad))} else {None}})
            .find(|it| {it.1.is_ipv6()}) {
            if let Some(pub_ip) = pub_ip {
                if pub_ip.0.is_some() {
                    return Some(ep)
                }
            } else {return Some(ep)}
        }
        None
    }
    pub fn get_generator_text(&self, endpoint: &Option<&Endpoint>) -> String {
        let desc = match &self.description {
            None => {""}
            Some(it) => {&format!(" - {}", it)}
        };
        let conn = if let Some(end) = endpoint {
            format!(": {}", end)
        } else {"".into()};
        format!("{}{}{}", self.id, conn, desc)
    }
    pub fn get_best_endpoint(&self) {

    }
}