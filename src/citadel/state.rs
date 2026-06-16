use crate::citadel::handshaker::Generator;
use crate::common::cmd::exec;
use crate::common::errors::{FFError, FFResult};
use crate::common::wireguard::{generate_wireguard_keys, get_default_route, Route, Wireguard, WireguardPeer, WireguardState};
use ipnet::{IpNet, PrefixLenError};
use openport::pick_random_unused_port;
use rand::distr::Alphanumeric;
use rand::RngExt;
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr};
use std::str::FromStr;
use std::fs;

#[derive(Serialize, Deserialize, Clone)]
pub struct BackendState {
    pub our_wg_pub: String,
    pub our_wg_priv: String,
    pub known_generators: Vec<Generator>,
    #[serde(skip, default)]
    pub current_wg_setup: Option<WireguardState>,
    pub current_wg_ids: Vec<String>
}
static FILE: &str = "conf.conf";
impl BackendState {
    pub fn get() -> Self {
        match fs::read_to_string(FILE) {
            Ok(it) => {
                serde_json::from_str(&it).unwrap()
            }
            Err(_) => {
                let (wg_private, wg_public) = generate_wireguard_keys();
                let data = Self {
                    our_wg_pub: wg_public,
                    our_wg_priv: wg_private,
                    known_generators: vec![],
                    current_wg_setup: None,
                    current_wg_ids: vec![]
                };
                data.save();
                data
            }
        }
    }
    pub fn save(&self) {
        let str = serde_json::to_string(self).unwrap();
        fs::write(FILE, str).unwrap();
    }
    pub fn delete() {
        fs::remove_file(FILE).unwrap();
    }

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
    fn is_everything(addresses: &IpNet) -> bool {
        addresses.prefix_len() == 0
    }
    fn is_everything_opt(addresses: &Option<IpNet>) -> bool {
        if let Some(it) = addresses {
            Self::is_everything(it)
        } else {true}
    }
    ///list is a list, in order, of which servers to use. it's indexes of known_generators
    pub fn create_wg_setup(&mut self, list: Vec<usize>, addresses: String) -> FFResult<()> {
        let addresses = IpNet::from_str(&addresses)?;
        if let Some(setup) = self.current_wg_setup.take() {
            setup.down();
        }
        self.current_wg_ids = vec![];
        if list.is_empty() {
            return Ok(());
        }

        let mut routes_to_add = vec![];
        let generators: Vec<&Generator> = list.iter().map(|it| &self.known_generators[*it]).collect();
        let mut peers: Vec<WireguardPeer> = list.iter().map(|i| &self.known_generators[*i])
            .enumerate()
            .map(|(id, it)| {
                let mut allowed_ip = vec![];
                //if last, tell wireguard that's the one we actually want to use. just route is not enough
                if id == list.len() - 1 {allowed_ip.push("0.0.0.0/0".parse().unwrap())}
                WireguardPeer::new(
                    it.wg_public_key.clone(),
                    allowed_ip,
                    Some((it.pub_ip, it.pub_port))
                )
            })
            .collect();
        for i in 0..(list.len()-1) {
            //well, this won't work if there's no endpoint...
            let endpoint = peers[i+1].endpoint.unwrap().0;
            let internal_ip = self.known_generators[list[i]].internal_ip;
            peers[i].allowed_ips.push(format!("{}/32", endpoint).parse()?);
            peers[i].allowed_ips.push(format!("{}/32", internal_ip).parse()?);
        }
        let wireguard = Wireguard::new(pick_random_unused_port().unwrap(), self.our_wg_priv.clone(), self.our_wg_pub.clone(), "uwu0".to_string(), IpAddr::from_str("10.69.0.1").unwrap(), peers);
        let peers = &wireguard.peers;
        let default = get_default_route();
        if Self::is_everything(&addresses) {
            default.remove_self();
        }
        //add first route, route to first generator over *normal network*
        routes_to_add.push(Route::new(
            simple_peer_to_cidr(peers.first().unwrap())?,
            default.via,
            default.device.clone(),
            default.src
        ));
        for id in 1..list.len() {
            //create each route, routing only the endpoint for the new wg device via the last one
            routes_to_add.push(Route::new(
                simple_peer_to_cidr(&peers[id])?,
                Some(generators[id - 1].internal_ip),//via: grab the last device's internal ip
                Some("uwu0".to_string()),
                Some("10.69.0.1".parse()?)
            ));
        }
        let l_id = list.len() - 1;
        //wireguard now knows how to reach each wireguard server, now add the default route to go to the last one.
        routes_to_add.push(Route::new(
            Some(addresses),
            Some(generators[l_id].internal_ip),
            Some("uwu0".to_string()),
            Some("10.69.0.1".parse()?)
        ));
        wireguard.spawn();
        for i in &routes_to_add {
            i.add_self();
        }
        let ipv4s = peers.iter().filter(|it| it.endpoint.is_some())
            .filter(|it| it.endpoint.unwrap().0.is_ipv4()).count();
        let ipv6s = peers.iter().filter(|it| it.endpoint.is_some())
            .filter(|it| it.endpoint.unwrap().0.is_ipv6()).count();
        exec(format!("ip link set uwu0 mtu {}", 1500 - 60 * ipv4s - 80 * ipv6s));
        self.current_wg_setup = Some(WireguardState::new(routes_to_add, vec![wireguard], default));
        self.current_wg_ids = list.into_iter().map(|it| self.known_generators[it].id.clone()).collect();
        Ok(())
    }
}
fn simple_peer_to_cidr(peer: &WireguardPeer) -> Result<Option<IpNet>, PrefixLenError> {
    IpNet::new(peer.endpoint.unwrap().0, 32)
        .map(|it| Some(it))
}