use std::{env, fs};
use crate::citadel::handshaker::Generator;
use crate::common::errors::{FFError, FFResult};
use crate::common::wireguard::{generate_wireguard_keys, Route, Wireguard, WireguardPeer, WireguardState};
use ipnet::{IpNet, PrefixLenError};
use openport::pick_random_unused_port;
use rand::distr::Alphanumeric;
use rand::RngExt;
use std::net::Ipv4Addr;
use regex::Regex;
use serde::{Deserialize, Serialize};
use crate::common::cmd::exec;
use crate::generator::init_config::InitialConfig;

#[derive(Serialize, Deserialize, Clone)]
pub struct BackendState {
    pub our_wg_pub: String,
    pub our_wg_priv: String,
    pub known_generators: Vec<Generator>,
    #[serde(skip, default)]
    pub current_wg_setup: Option<WireguardState>,
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
        //add first route, route to first generator over *normal network*
        routes_to_add.push(Route::new(
            simple_wireguard_to_cidr(&wireguards[0])?,
            Some(generators[0].internal_ip),
            Some("wlan0".into()),
            //Some("10.69.0.1".parse()?)//lets try it with no src addr
            None
        ));
        for id in 1..list.len() {
            //create each route, routing only the endpoint for the new wg device via the last one
            routes_to_add.push(Route::new(
                simple_wireguard_to_cidr(&wireguards[id])?,
                Some(generators[id - 1].internal_ip),//via: grab the last device's internal ip
                Some(wireguards[id - 1].name.clone()),
                //Some("10.69.0.1".parse()?)
                None
            ));
        }
        let l_id = list.len() - 1;
        //wireguard now knows how to reach each wireguard server, now add the default route to go to the last one.
        routes_to_add.push(Route::new(
            None,
            Some(generators[l_id].internal_ip),
            Some(wireguards[l_id].name.clone()),
            //Some("10.69.0.1".parse()?)
            None
        ));
        drop(self.current_wg_setup.take());
        for i in &wireguards {
            i.spawn();
        }
        for i in &routes_to_add {
            i.add_self();
        }
        self.current_wg_setup = Some(WireguardState::new(routes_to_add, wireguards));
        Ok(())
    }
}
fn simple_wireguard_to_cidr(addr: &Wireguard) -> Result<Option<IpNet>, PrefixLenError> {
    IpNet::new(addr.peers[0].endpoint.unwrap().0, 32)
        .map(|it| Some(it))
}