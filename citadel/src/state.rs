use crate::control_connection::ControlConnection;
use crate::handshaker::{Endpoint, Generator, Peer, PeerData};
use common::cmd::exec;
use common::errors::{FFError, FFResult};
use common::wireguard::{generate_wireguard_keys, get_default_route_v4, get_default_route_v6, get_routes, Route, Wireguard, WireguardPeer, WireguardState, EndpointAddr};
use ipnet::{AddrParseError, IpNet, Ipv6Net};
use openport::pick_random_unused_port;
use rand::distr::Alphanumeric;
use rand::{rng, RngExt};
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::str::FromStr;
use std::{fs};
use std::io::SeekFrom::End;
use crossbeam_channel::{Receiver, Sender};
use rsa::pkcs1::LineEnding;
use rsa::pkcs8::{DecodePrivateKey, EncodePrivateKey};
use rsa::RsaPrivateKey;
use common::ip::Port;
use crate::ui::ui_main::KeyResult;

#[derive(Serialize, Deserialize)]
pub struct BackendState {
    pub our_wg_pub: String,
    pub our_wg_priv: String,
    pub known_generators: Vec<Peer>,
    #[serde(skip, default)]
    pub current_wg_setup: Option<WireguardState>,
    pub current_wg_ids: Vec<String>,
    ///has the same length as "current_wg_ids", each Endpoint represents how we reach the corresponding generator
    pub endpoints_used: Vec<(Endpoint, Option<Port>)>,
    #[serde(skip, default)]
    pub channels: Option<(Sender<KeyResult>, Receiver<KeyResult>)>,
    pub rsa_priv: String
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
                let rsa_priv = RsaPrivateKey::new(&mut rng(), 4096).unwrap().to_pkcs8_pem(LineEnding::CRLF).unwrap().to_string();
                let data = Self {
                    our_wg_pub: wg_public,
                    our_wg_priv: wg_private,
                    known_generators: vec![],
                    current_wg_setup: None,
                    current_wg_ids: vec![],
                    endpoints_used: vec![],
                    channels: None,
                    rsa_priv
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

    pub fn get_rsa_priv(&self) -> RsaPrivateKey {
        RsaPrivateKey::from_pkcs8_pem(&self.rsa_priv).unwrap()
    }
    pub fn next_id(&self) -> FFResult<(Ipv4Addr, Ipv6Addr, String)> {
        let ipv6_range: Ipv6Net = "fd80:51e8:5d8e::/48".parse().unwrap();
        //skips ::0 and ::1
        let ipv6 = ipv6_range.hosts().skip(self.known_generators.len() + 2).next().unwrap();
        let offset = (self.known_generators.len() + 2) as u32;
        let id = (0..8).map(|_| rand::rng().sample(Alphanumeric) as char).collect();
        let mut ip: Ipv4Addr = "10.69.0.0".parse()?;
        if offset >= 0b11111110 {
            Err(FFError::OutOfIds.into())
        } else {
            let mut bits = ip.octets();
            bits[3] += offset as u8;
            ip = Ipv4Addr::from_octets(bits);
            Ok((ip, ipv6, id))
        }
    }
    fn is_everything(addresses: &[IpNet]) -> bool {
        for address in addresses {
            if address.prefix_len() == 0 {
                return true
            }
        }
        false
    }
    /*fn is_everything_opt(addresses: &Option<IpNet>) -> bool {
        if let Some(it) = addresses {
            Self::is_everything(it)
        } else {true}
    }*/
    fn force_get_used_ip(&self, id: &str) -> Option<SocketAddr> {
        let peer = self.get_by_id(id)?;
        match peer {
            Peer::Ge(it) => {Some((it.internal_ip_v4, it.config_port).into())}
            Peer::Server(_) => {None}
        }
    }
    fn send_wakeups(&mut self) -> FFResult<()> {
        for (index, id) in self.current_wg_ids.iter().enumerate() {
            self.send_wakeup(index, id);
        }
        Ok(())
    }
    fn send_wakeup(&self, index: usize, id: &String) -> Option<()> {
        let peer = self.get_by_id(id).unwrap();
        if let Peer::Ge(ge) = peer {
            let next_id = self.current_wg_ids.get(index + 1)?;
            let ip = self.force_get_used_ip(next_id)?;
            let mut conn = ControlConnection::connect((ge.internal_ip_v4, ge.config_port).into(), self).ok()?;
            conn.send_wakeup_to(ip).ok()?;
            conn.send_heartbeat().ok()?;
            Some(())
        } else {None}
    }
    fn send_shutdowns(&mut self) -> bool {
        let mut errored_out = false;
        for (index, id) in self.current_wg_ids.iter().enumerate().rev() {
            if let Some(b) = self.send_shutdown(index, id) {
                errored_out |= b;
            }
        }
        errored_out
    }
    fn send_shutdown(&self, index: usize, id: &String) -> Option<bool> {
        let peer = self.get_by_id(id).unwrap();
        if let Peer::Ge(ge) = peer {
            let next_id = self.current_wg_ids.get(index + 1)?;
            let next_ip = self.force_get_used_ip(next_id)?;
            let conn = ControlConnection::connect((ge.internal_ip_v4, ge.config_port).into(), self);
            if let Ok(mut conn) = conn {
                let _ = conn.send_shutdown_to(next_ip);
            } else {
                return Some(true);
            }
        }
        Some(false)
    }
    fn peer_to_wg_peer(list: &Vec<(usize, Endpoint, Option<u16>)>, id: usize, (it, endpoint, ws): (&Peer, &Endpoint, Option<u16>)) -> Result<WireguardPeer, Wireguard> {
        let mut allowed_ip = vec![];
        //if last, tell wireguard that's the one we actually want to use. just route is not enough
        if id == list.len() - 1 {
            allowed_ip.push("0.0.0.0/0".parse().unwrap());
            allowed_ip.push("::/0".parse().unwrap());
        }
        let end = match &endpoint {
            Endpoint::PublicEndpoint(it) => {it}
            Endpoint::ViaPeer(_) => {//don't need to know this peer's id, it'll be the last one we did
                let it = it.as_generator().unwrap();
                &SocketAddr::new(it.internal_ip_v4.into(), it.wg_port)
            },
            Endpoint::FromPeer(it, _) => {it}
        };
        let ea = if let Some(ws) = ws {
            EndpointAddr::ActiveWstunnel((end.ip(), ws).into(), end.port())
        } else {EndpointAddr::Active(*end)};
        match it {
            Peer::Ge(it) => {
                Ok(
                    WireguardPeer::new(
                        it.wg_public_key.clone(),
                        allowed_ip,
                        ea
                    )
                )
            }
            Peer::Server(server) => {
                let peer = WireguardPeer::new_psk(server.peer_wg_public_key.clone(), server.peer_wg_preshared_key.clone(), allowed_ip, ea);
                let wg = Wireguard::new(pick_random_unused_port().unwrap(), server.wg_private_key.clone(),
                                        server.wg_public_key.clone(), format!("uwu_{}", server.id),
                                        server.interface_ipv4.unwrap(), server.interface_ipv6.unwrap(), vec![peer]);
                Err(wg)
            }
        }
    }
    fn create_new_route(wg_peers: &Vec<IpNet>, peers: &Vec<&Peer>, id: usize) -> Route {
        let end = /*simple_peer_to_cidr(*/&wg_peers[id]/*).unwrap()*/;
        Self::create_new_route_(peers[id - 1], *end, None)
    }
    fn create_new_route_(from_generator: &Peer, to_cidr: IpNet, metric: Option<u32>) -> Route {
        match from_generator {
            Peer::Ge(ge) => {
                if let IpNet::V6(_) = to_cidr {
                    Route::new_full(
                        to_cidr,
                        Some(IpAddr::V6(ge.internal_ip_v6)), //via: grab the last device's internal ip
                        Some("uwu0".to_string()),
                        Some("fd80:51e8:5d8e::1".parse().unwrap()),
                        None,
                        metric
                    )
                } else {
                    Route::new_full(
                        to_cidr,
                        Some(IpAddr::V4(ge.internal_ip_v4)), //via: grab the last device's internal ip
                        Some("uwu0".to_string()),
                        Some("10.69.0.1".parse().unwrap()),
                        None, 
                        metric
                    )
                }
            },
            Peer::Server(wge) => {
                if let IpNet::V6(_) = to_cidr {
                    Route::new_full(
                        to_cidr,
                        None, //we don't know this
                        Some(format!("uwu_{}", wge.id)),
                        Some(wge.interface_ipv6.unwrap().into()),
                        None,
                        metric
                    )
                } else {
                    Route::new_full(
                        to_cidr,
                        None, //we don't know this
                        Some(format!("uwu_{}", wge.id)),
                        Some(wge.interface_ipv4.unwrap().into()),
                        None,
                        metric
                    )
                }
            }
        }
    }
    ///list is a list, in order, of which servers to use. it's indexes of (known_generators, connection mode)
    pub fn create_wg_setup(&mut self, list: Vec<(usize, Endpoint, Option<u16>)>, addresses: String) -> FFResult<()> {
        self.send_shutdowns();
        if let Some(mut setup) = self.current_wg_setup.take() {
            setup.down();
        }
        self.current_wg_ids = vec![];
        self.endpoints_used = vec![];
        if list.is_empty() {
            return Ok(());
        }
        
        let mut addr_len = 0;
		let addresses: Vec<IpNet> = addresses.split(",").map(IpNet::from_str).filter_map(|it| {addr_len += 1; it.ok()}).collect();
        if addr_len != addresses.len() && addresses.len() != 0 {
            return Err(FFError::BadCIDR.into())
        }

        let mut routes_to_add = vec![];
        let generators: Vec<&Peer> = list.iter().map(|it| &self.known_generators[it.0]).collect();
        let mut peer_addresses: Vec<IpNet> = vec![];
        let mut main_peers: Vec<Result<WireguardPeer, Wireguard>> = list.iter().map(|i| (&self.known_generators[i.0], &i.1, i.2))
            .enumerate()
            .map(|(id, (it, end, ws))| {
                Self::peer_to_wg_peer(&list, id, (it, end, ws))
            })
            .collect();
        for i in 0..list.len()-1 {
            //well, this won't work if there's no endpoint...
            let addr = wg_to_addr(&main_peers[i+1]);
            let (internal_ipv4, internal_ipv6) = match &self.known_generators[list[i].0] {
                Peer::Ge(it) => {(it.internal_ip_v4, it.internal_ip_v6)}
                Peer::Server(it) => {(it.interface_ipv4.unwrap(), it.interface_ipv6.unwrap())}
            };
            let peer = match &mut main_peers[i] {
                Ok(it) => {it}
                Err(it) => {&mut it.peers[0]}
            };
            peer_addresses.push(addr.into());
            peer.allowed_ips.push(addr.into());
            peer.allowed_ips.push(IpNet::V4(internal_ipv4.into()));
            peer.allowed_ips.push(IpNet::V6(internal_ipv6.into()));
        }
        if let Some(last_peer) = main_peers.get(list.len().saturating_sub(1)) {
            peer_addresses.push(wg_to_addr(&last_peer).into());
        }
        let first_addr = match &main_peers[0] {
            Ok(it) => {it.endpoint.addr().unwrap()}
            Err(it) => {it.peers[0].endpoint.addr().unwrap()}
        }.ip();
        let mut wireguards: Vec<Wireguard> = vec![];
        let mut peers = vec![];
        main_peers.into_iter().for_each(|it| match it {
            Ok(it) => {peers.push(it)}
            Err(it) => {wireguards.push(it) }
        });
        let mut wireguard = Wireguard::new(pick_random_unused_port().unwrap(), self.our_wg_priv.clone(),
                                           self.our_wg_pub.clone(), "uwu0".to_string(), Ipv4Addr::from_str("10.69.0.1").unwrap(),
                                           Ipv6Addr::from_str("fd80:51e8:5d8e::1").unwrap(), peers);
        let peers = &wireguard.peers;
        let routes = get_routes();
        let mut default = None;
        let default_v4 = get_default_route_v4(&routes).cloned();
        let default_v6 = get_default_route_v6(&routes).cloned();
        if let Some(v6) = default_v6.clone() && Self::is_everything(&[v6.addresses]) {
            v6.remove_self();
            default = Some(v6);
        }
        if let Some(v4) = default_v4.clone() && Self::is_everything(&[v4.addresses]) {
            v4.remove_self();
            default = Some(v4);
        }
        let default = default.ok_or(FFError::NoRealInternetConnection)?;
        //add first route, route to first generator over *normal network*
        routes_to_add.push(Route::new(
            //this has to be unwrapped because we need the first peer to have a routable endpoint
            first_addr.into(),
            default.via,
            default.device.clone(),
            default.src
        ));
        for id in 1..list.len() {
            //create each route, routing only the endpoint for the new wg device via the last one
            routes_to_add.push(Self::create_new_route(&peer_addresses, &generators, id));
        }
        let l_id = list.len() - 1;
        //wireguard now knows how to reach each wireguard server, now add the default route to go to the last one.
        if addresses.is_empty() {
            if let Some(def) = default_v4.as_ref() {
                routes_to_add.push(def.clone());
            }
            if let Some(def) = default_v6.as_ref() {
                routes_to_add.push(def.clone());
            }
        }
        for address in addresses {
            routes_to_add.push(Self::create_new_route_(&generators[l_id], address, Some(50)));
        }
        let ipv4s = peers.iter().filter_map(|it| it.endpoint.addr())
            .filter(|it| it.ip().is_ipv4()).count();
        let ipv6s = peers.iter().filter_map(|it| it.endpoint.addr())
            .filter(|it| it.ip().is_ipv6()).count();
        wireguard.spawn();
        for i in &mut wireguards {
            i.spawn();
        }
        wireguards.insert(0, wireguard);
        for i in &routes_to_add {
            i.add_self();
        }
        let mtu = Self::get_mtu_of_def_interface(&default).unwrap_or(1500) - 20;
        let websockets = list.iter().filter(|(_,_,ws)| ws.is_some()).count();
        exec(format!("ip link set uwu0 mtu {}", mtu - 60 * ipv4s - 80 * ipv6s - 80 * websockets));
        self.current_wg_setup = Some(WireguardState::new(routes_to_add, wireguards, default_v4, default_v6));
        self.current_wg_ids = list.iter().map(|(it, _, _)| self.known_generators[*it].get_id().clone()).collect();
        self.endpoints_used = list.into_iter().map(|(_, it, ws)| (it, ws)).collect();
        self.send_wakeups()?;
        Ok(())
    }
    fn get_mtu_of_def_interface(dr: &Route) -> Option<usize> {
        let dev = dr.device.as_ref()?;
        let mtu_file = fs::read_to_string(format!("/sys/class/net/{}/mtu", dev)).ok()?;
        mtu_file.trim().parse().ok()
    }
    pub fn get_index_by_id(&self, id: &str) -> Option<usize> {
        self.known_generators.iter().enumerate()
            .find(|it| it.1.get_id().eq(&id))
            .map(|it| it.0)
    }
    pub fn get_next_gen(&self, id: &str) -> Option<String> {
        let nid = self.current_wg_ids.iter().position(|it| it.eq(id))?;
        self.current_wg_ids.get(nid + 1).map(Clone::clone)
    }
    pub fn get_by_id(&self, id: &str) -> Option<&Peer> {
        for i in 0..self.known_generators.len() {
            if self.known_generators[i].get_id().eq(&id) {
                return self.known_generators.get(i);
            }
        }
        None
    }
    pub fn get_by_id_mut(&mut self, id: &str) -> Option<&mut Peer> {
        for i in 0..self.known_generators.len() {
            if self.known_generators[i].get_id().eq(&id) {
                return self.known_generators.get_mut(i);
            }
        }
        None
    }
    //list is a list, in order, of which servers to use. it's indexes of (known_generators, connection mode)
    /*pub fn create_wg_setup_2(&mut self, list: Vec<(usize, Endpoint)>, addresses: String) -> FFResult<()> {
        let addresses = IpNet::from_str(&addresses)?;
        if let Some(setup) = self.current_wg_setup.take() {
            setup.down();
        }
        self.current_wg_ids = vec![];
        self.endpoints_used = vec![];
        if list.is_empty() {
            return Ok(());
        }
        //create our wireguard setup
        let our_wg = Wireguard::new(pick_random_unused_port().unwrap(), self.our_wg_priv.clone(), self.our_wg_pub.clone(), "uwu0".into(), "10.69.0.1".parse()?, vec![]);
        let num_hops = list.len();
        for (index, (generator, end)) in list.iter().enumerate() {
            let generator = &self.known_generators[generator];

            let (peer_wg_ip, via_last) = match end {
                Endpoint::PublicEndpoint(addr) => {(addr, true)}
                Endpoint::ViaPeer(peer) => {}
                Endpoint::FromPeer(addr, _) => {(addr, true)}
            };
        }

        Ok(())
    }*/
}
fn simple_peer_to_cidr(peer: &WireguardPeer) -> Option<IpNet> {
    peer.endpoint.addr().map(|it| it.ip().into())
}
fn wg_to_addr(point: &Result<WireguardPeer, Wireguard>) -> IpAddr {
    let addr = match &point {
        Ok(it) => {it.endpoint.addr().unwrap().ip()}
        Err(it) => {it.peers[0].endpoint.addr().unwrap().ip()}
    };
    addr
}