use std::net::{IpAddr, Ipv4Addr};
use ipnet::{IpNet, Ipv4Net, PrefixLenError};
use openport::pick_random_unused_port;
use crate::citadel::handshaker::Generator;
use crate::common::errors::{FFError, FFResult};
use rand::distr::{Alphabetic, Alphanumeric, SampleString};
use rand::{rng, RngExt};
use crate::common::wireguard::{Route, Wireguard, WireguardPeer, WireguardState};

pub struct State {
    pub our_wg_pub: String,
    pub our_wg_priv: String,
    pub known_generators: Vec<Generator>,

    pub current_wg_setup: Option<WireguardState>,
}
impl State {
    pub fn next_id(&self) -> FFResult<(Ipv4Addr, String)> {
        let offset = (self.known_generators.len() + 2) as u32;
        let id = (0..8).map(|_| rand::rng().sample(Alphanumeric) as char).collect();
        let mut ip: Ipv4Addr = "10.69.0.0".parse()?;
        if offset >= 0b11111110 {
            Err(FFError::OutOfIds.into())
        } else {
            let mut bits = ip.octets();
            bits[3] += offset as u8;
            ip = Ipv4Addr::from_octets(bits);
            Ok((ip, id))
        }
    }
    ///list is a list, in order, of which servers to use. it's indexes of known_generators
    pub fn create_wg_setup(&mut self, list: Vec<usize>) -> FFResult<()> {
        let mut routes_to_add = vec![];
        let generators: Vec<&Generator> = list.iter().map(|it| &self.known_generators[*it]).collect();
        let wireguards: Vec<Wireguard> = list.iter().map(|i| &self.known_generators[*i])
            .enumerate()
            .map(|(i, it)| {
                let name = format!("wg{}", i + 1);
                let peer = WireguardPeer::new(
                    it.wg_public_key.clone(), 
                    vec!["10.69.0.0/16".parse().unwrap()], 
                    Some((it.pub_ip, it.pub_port))
                );
                Wireguard::new(pick_random_unused_port().unwrap(), self.our_wg_priv.clone(), self.our_wg_pub.clone(), name, vec![peer])
            })
            .collect();
        routes_to_add.push(Route::new(
            simple_wireguard_to_cidr(&wireguards[0])?,
            Some(generators[0].internal_ip),
            Some("wlan0".into()),
            Some("10.69.0.1".parse()?)
        ));
        return Ok(());
    }
}
fn simple_wireguard_to_cidr(addr: &Wireguard) -> Result<Option<IpNet>, PrefixLenError> {
    IpNet::new(addr.peers[0].endpoint.unwrap().0, 32)
        .map(|it| Some(it))
}