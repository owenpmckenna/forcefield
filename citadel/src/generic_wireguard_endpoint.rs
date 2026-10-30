use crate::generic_wireguard_endpoint::ForeignAddress::Ip;
use crate::handshaker::Endpoint::{FromPeer, PublicEndpoint};
use crate::handshaker::{Endpoint, PeerData};
use crate::state::BackendState;
use common::errors::FFError::{ConfigParseError, NoPeerAddressError, NoPeersError};
use common::ip::Port;
use common::wireguard::{Route, get_pub_key_for_private};
use ipnet::IpNet;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::io::BufRead;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::str::FromStr;
use std::sync::Mutex;
use std::time::SystemTime;

#[derive(Deserialize, Serialize)]
pub struct WireguardEndpoint {
	pub id: String,
	pub desc: String,
	pub interface_ipv4: Option<Ipv4Addr>,
	pub interface_ipv6: Option<Ipv6Addr>,
	pub wg_private_key: String,
	pub wg_public_key: String,
	pub mtu_override: Option<u16>,

	pub peer_wg_public_key: String,
	pub peer_wg_preshared_key: Option<String>,
	pub peer_endpoint: ForeignAddress,
	#[serde(default)]
	cached_peer_endpoint: Mutex<Option<(SystemTime, SocketAddr)>>,
	#[serde(default)]
	effective_endpoint: Mutex<Option<Endpoint>>,

	supposed_internal_ips_: (Ipv4Addr, Ipv6Addr)
}
impl WireguardEndpoint {
	pub fn parse(string: &str) -> Result<Vec<(String, Vec<(String, String)>)>, Box<dyn Error>> {
		let mut known = vec![];
		let mut seeking = None;
		let mut data = vec![];
		let lines = string.lines();
		for i in lines {
			let re = Regex::new(r"\[([a-zA-Z0-9_\-]+)]")?;
			if let Some(re) = re.captures(i) {
				let str = re.get(1).unwrap().as_str().to_string();
				//println!("Found region {}", str);
				if let Some(it) = seeking.take() {
					known.push((it, data));
					data = vec![];
				}
				seeking = Some(str);
			} else {
				//print!("Found data? {} ---", data.len());
				if let Some((left, right)) = i.split_once("=") {
					data.push((left.trim().to_string(), right.trim().to_string()))
				}
			}
		}
		if let Some(seeking) = seeking {
			known.push((seeking, data));
		}
		Ok(known)
	}
	pub fn new(data: &str, desc: &str, state: &mut BackendState) -> Result<Self, Box<dyn Error>> {
		let data = Self::parse(data)?;
		//println!("{:?}", data);
		//panic!();
		let mut map = HashMap::new();
		for (name, d) in data {
			let mut mp = HashMap::new();
			for i in d {
				mp.insert(i.0, i.1);
			}
			map.insert(name, mp);
		}
		let ips: Vec<IpNet> = map["Interface"]["Address"].split(",")
			.filter_map(|i| i.trim().to_string().parse().ok()).collect::<Vec<_>>();
		let ipv4_addr = ips.iter().find_map(
			|it| if let IpAddr::V4(it) = it.addr() {Some(it)} else {None}
		);
		let ipv6_addr = ips.iter().find_map(
			|it| if let IpAddr::V6(it) = it.addr() {Some(it)} else {None}
		);
		if ipv4_addr.is_none() && ipv6_addr.is_none() {
			return Err(err("Need to have at least one ip."))
		}
		let privkey = &map["Interface"]["PrivateKey"];
		let pubkey = if let Ok(it) = get_pub_key_for_private(&privkey) {
			it
		} else {
			return Err(err("Invalid Private Key (couldn't conv to public key)"))
		};
		let mtu = map["Interface"].get("MTU").map(|it| it.parse().ok()).flatten();
		let peer = map.get("Peer").ok_or(NoPeersError)?;
		let peer_public = peer["PublicKey"].clone();
		let preshared = peer.get("PresharedKey").cloned();
		let peer_endpoint = ForeignAddress::from_str(&peer.get("Endpoint").ok_or(NoPeerAddressError)?, 51820)?;
		let ids = state.next_id()?;
		Ok(Self {
			id: ids.2,
			desc: desc.to_string(),
			interface_ipv4: ipv4_addr,
			interface_ipv6: ipv6_addr,
			wg_private_key: privkey.to_string(),
			wg_public_key: pubkey,
			mtu_override: mtu,
			peer_wg_public_key: peer_public,
			peer_wg_preshared_key: preshared,
			peer_endpoint,
			effective_endpoint: Default::default(),
			supposed_internal_ips_: (ids.0, ids.1),
			cached_peer_endpoint: Mutex::default()
		})
	}
}
fn err(str: &str) -> Box<dyn Error> {
	ConfigParseError(str.into()).into()
}
impl PeerData for WireguardEndpoint {
	fn get_id(&self) -> &String {
		&self.id
	}
	fn get_description(&self) -> &String {
		&self.desc
	}
	fn get_description_mut(&mut self) -> &mut String {
		&mut self.desc
	}

	fn get_generator_text(&self, endpoint: &Option<&Endpoint>) -> String {
		let desc = format!(" - {}", self.desc);
		let conn = if let Some(end) = endpoint {
			format!(": {}", end)
		} else {"".into()};
		format!("{}{}{}", self.id, conn, desc)
	}

	fn find_best_endpoint(&self, routes: &Vec<Route>, connected: Result<&Vec<String>, Option<String>>, pub_ip: Option<(Option<Ipv6Addr>, Option<Ipv4Addr>)>) -> Option<Endpoint> {
		let mut lock = self.cached_peer_endpoint.lock().unwrap();
		let cache = if let Some((time, addr)) = &(*lock) && time.elapsed().unwrap().as_millis() < 5 * 60 * 1000 {
			(*time, *addr)
		} else {
			(SystemTime::now(), self.peer_endpoint.resolve().ok()?)
		};
		let end = cache.1.clone();
		*lock = Some(cache);
		Some(if end.ip().is_global() {
			PublicEndpoint(end)
		} else {
			FromPeer(end, match connected {
				Ok(it) => {it.last().cloned()}
				Err(it) => {it}
			})
		})
	}
}
#[derive(Deserialize)]
struct WireguardToml {
	Interface: Interface,
	Peer: Peer
}
#[derive(Deserialize)]
struct Interface {
	Address: String,
	PrivateKey: String,
	MTU: Option<u16>,
	DNS: Option<String>
}
#[derive(Deserialize)]
struct Peer {
	PublicKey: String,
	PresharedKey: Option<String>,
	Endpoint: String
}

#[derive(Serialize, Deserialize)]
pub enum ForeignAddress {
	Ip(SocketAddr),
	DNS(String, Port)
}
impl ForeignAddress {
	pub fn from_str(s: &str, default_port: Port) -> Result<Self, Box<dyn Error>> {
		if let Ok(addr) = SocketAddr::parse_ascii(s.as_bytes()) {
			return Ok(Self::Ip(addr))
		}
		if let Ok(addr) = IpAddr::parse_ascii(s.as_bytes()) {
			return Ok(Self::Ip((addr, default_port).into()))
		}
		let regex = Regex::new(r"^([a-z0-9][a-z0-9\-]*\.)+[a-z]+(:[0-9]{1,5})?$")?;
		if !regex.is_match(s) {
			return Err(err("Could not find Ip address or hostname"));
		}
		let split = s.split(":").collect::<Vec<_>>();
		let host = split[0];
		let port = split.get(1).map(|it| it.parse().unwrap()).unwrap_or(default_port);
		Ok(Self::DNS(host.to_string(), port))
	}
	pub fn resolve(&self) -> Result<SocketAddr, Box<dyn Error>> {
		let (name, port) = match self {
			Ip(it) => {return Ok(*it);}
			ForeignAddress::DNS(name, port) => {(name, port)}
		};

		use tokio::runtime::Runtime;

		// Set up the standard tokio runtime (multithreaded by default).
		let runtime = Runtime::new().expect("Failed to create runtime");

		let resolver = {
			// To make this independent, if targeting macOS, BSD, Linux, or Windows, we can use the system's configuration:
			#[cfg(any(unix, windows))]
			{
				use hickory_resolver::{TokioResolver, net::runtime::TokioRuntimeProvider};

				// use the system resolver configuration
				TokioResolver::builder(TokioRuntimeProvider::default())
					.expect("failed to create resolver")
					.build()?
			}

			// For other operating systems, we can use one of the preconfigured definitions
			#[cfg(not(any(unix, windows)))]
			{
				// Directly reference the config types
				use hickory_resolver::{
					Resolver,
					config::{GOOGLE, ResolverConfig, ResolverOpts},
				};

				// Get a new resolver with the google nameservers as the upstream recursive resolvers
				Resolver::tokio(ResolverConfig::udp_and_tcp(), ResolverOpts::default())
			}
		};

		let future = resolver.lookup_ip(name);

		let ips = runtime
			.block_on(future)
			.expect("Failed completing lookup future")
			.iter()
			.collect::<Vec<_>>();

		// Drop the resolver, which means that the runtime will become idle.
		drop(resolver);
		drop(runtime);

		Ok((ips[0], *port).into())
	}
}
impl Display for ForeignAddress {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		match self {
			Ip(it) => {write!(f, "{it}")}
			ForeignAddress::DNS(it, port) => {write!(f, "{it}:{port}")}
		}
	}
}
