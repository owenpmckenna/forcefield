use crate::common::cmd::exec;
use crate::common::commands::{Command, Response};
use crate::common::errors::FFError::ICMPPacketError;
use crate::common::errors::{FFError, FFResult};
use crate::common::setup_handshake::{read_encrypted_data, read_packet, write_encrypted_data, write_packet, ConfigMessage};
use crate::common::wireguard::{get_default_route_v4, get_default_route_v6, get_pub_ipv6_addr, get_routes, Route, Wireguard, WireguardPeer};
use crate::generator::config::Config;
use crate::generator::init_config::InitialConfig;
use crate::generator::on_wakeup::do_wakeup;
use crate::generator::receive_connections::{prep_receive_connections, prep_receive_udp_connection};
use chacha20poly1305::aead::generic_array::GenericArray;
use chacha20poly1305::aead::{AeadMut, Payload};
use chacha20poly1305::consts::U24;
use chacha20poly1305::{Key, KeyInit, XChaCha20Poly1305};
use crossbeam_channel::select;
use icmp_socket::packet::WithEchoRequest;
use icmp_socket::{IcmpSocket, IcmpSocket4, IcmpSocket6, Icmpv4Packet, Icmpv6Packet};
use ipnet::IpNet;
use rsa::pkcs8::DecodePublicKey;
use rsa::{Pkcs1v15Encrypt, RsaPublicKey};
use std::collections::HashMap;
use std::error::Error;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, TcpListener, TcpStream, UdpSocket};
use std::process::exit;
use std::str::FromStr;
use std::sync::atomic::AtomicU16;
use std::sync::atomic::Ordering::SeqCst;
use std::sync::OnceLock;
use std::thread::sleep;
use std::time::Duration;
use rand::RngExt;

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
    loop {
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
    println!("read and decrypted config packet. Id: {}, ip4: {}, ip6: {}", out.server_id, out.server_ipv4, out.server_ipv6);
    Ok(out)
}
fn add_to_wireguard(config: &mut Config, wg: &mut Wireguard, (pub_key, (internal_ipv4, internal_ipv6), endpoint): &(String, (Ipv4Addr, Ipv6Addr), Option<SocketAddr>)) -> Vec<Route> {
    let (internal_ipv4, internal_ipv6) = (IpNet::V4((*internal_ipv4).into()), IpNet::V6((*internal_ipv6).into()));
    let wgp = WireguardPeer::new(pub_key.clone(), vec![internal_ipv4, internal_ipv6], *endpoint);
    let local_ipv4 = &config.server_ipv4;
    let local_ipv6 = &config.server_ipv6;
    let route_v4 = Route::new(internal_ipv4, None, Some("uwu0".into()), Some(local_ipv4.parse().unwrap()));
    let route_v6 = Route::new(internal_ipv6, None, Some("uwu0".into()), Some(local_ipv6.parse().unwrap()));
    wg.spawn_peer(&wgp);
    wg.peers.push(wgp);
    route_v4.add_self();
    route_v6.add_self();
    if endpoint.is_some() {
        let ipaddr = route_v4.addresses.hosts().next().unwrap();
        let _ = ping(ipaddr);//wakeup
    };
    vec![route_v4, route_v6]
}
static DEFAULT_DEVICE: OnceLock<String> = OnceLock::new();
fn run(mut config: Config) {
    let routes = get_routes();
    let mut default = None;
    let default_v4 = get_default_route_v4(&routes).cloned();
    let default_v6 = get_default_route_v6(&routes).cloned();
    if let Some(v6) = default_v6.clone() && v6.addresses.prefix_len() == 0 {
        default = Some(v6);
    }
    if let Some(v4) = default_v4.clone() && v4.addresses.prefix_len() == 0 {
        default = Some(v4);
    }
    let default = default.ok_or(FFError::NoRealInternetConnection).unwrap();
    let def_dev = default.device.unwrap();
    DEFAULT_DEVICE.get_or_init(|| def_dev.clone());
    exec(format!("sudo sysctl net.ipv6.conf.{}.proxy_ndp=1", def_dev));
    exec("sudo sysctl net.ipv6.conf.all.forwarding=1".to_string());
    exec(format!("sudo sysctl net.ipv6.conf.{}.accept_ra=2", def_dev));
    exec(format!("sudo iptables -t nat -A POSTROUTING -s 10.69.0.0/16 -o {} -j MASQUERADE", def_dev));
    //don't do this one, we handle it later using NAT instead of MASQ, I think
    //exec(format!("sudo ip6tables -t nat -A POSTROUTING -s fd80:51e8:5d8e::/48 -o {} -j MASQUERADE", def_dev));
    exec(format!("iptables -A FORWARD -i uwu0 -o {} -j ACCEPT", def_dev));
    exec(format!("ip6tables -A FORWARD -i uwu0 -o {} -j ACCEPT", def_dev));
    exec(format!("iptables -A FORWARD -i {} -o uwu0 -m conntrack --ctstate ESTABLISHED,RELATED -j ACCEPT", def_dev));
    exec(format!("ip6tables -A FORWARD -i {} -o uwu0 -m conntrack --ctstate ESTABLISHED,RELATED -j ACCEPT", def_dev));
    exec("iptables -A FORWARD -i uwu0 -o uwu0 -j ACCEPT".to_string());
    exec("ip6tables -A FORWARD -i uwu0 -o uwu0 -j ACCEPT".to_string());
    exec(format!("iptables -t nat -A POSTROUTING -s 10.69.0.1/32 -d 10.69.0.0/16 -o uwu0 -j SNAT --to-source {}", config.server_ipv4));
    //if it's from citadel and it's going to another generator, shove it back into the wireguard interface
    exec(format!("ip6tables -t nat -A POSTROUTING -s fd80:51e8:5d8e::1/128 -d fd80:51e8:5d8e::/48 -o uwu0 -j SNAT --to-source {}", config.server_ipv6));
    //if it's from any peer and it's going to *not* another generator, NAT it and send it to the main internet
    exec(format!("ip6tables -t nat -A POSTROUTING -s fd80:51e8:5d8e::/48 -o {} -j MASQUERADE", def_dev));
    let peers = vec![WireguardPeer::new(config.citadel_wg_pub.clone(), vec!["10.69.0.1/32".parse().unwrap(), "0.0.0.0/0".parse().unwrap(), "::/0".parse().unwrap(), "fd80:51e8:5d8e::1/128".parse().unwrap()], None)];
    let mut wg = Wireguard::new(config.port, config.gen_wg_priv.clone(), config.gen_wg_pub.clone(), "uwu0".to_string(), Ipv4Addr::from_str(&config.server_ipv4).unwrap(), Ipv6Addr::from_str(&config.server_ipv6).unwrap(), peers);
    let mut routes = HashMap::new();
    wg.spawn();
    for i in config.get_peers() {
        routes.insert(i.0.clone(), add_to_wireguard(&mut config, &mut wg, &i));
    }
    let mut cipher = XChaCha20Poly1305::new(config.get_config_key());
    let listener_data = prep_receive_connections(config.config_port);
    loop {
        let random = 120 + rand::rng().random_range(0..120);
        select! {
            recv(listener_data) -> msg => {
                if let Ok((stream, addr)) = msg {
                    handle_socket(addr.ip(), stream, &mut config, &mut cipher, &mut wg, &mut routes);
                }
            },
            default(Duration::from_secs(random)) => {
                do_wakeup(&mut config, &mut routes, &mut wg);
            },
        };
    }
}
fn handle_socket(addr: IpAddr, mut socket: TcpStream, config: &mut Config, cipher: &mut XChaCha20Poly1305, wg: &mut Wireguard, routes: &mut HashMap<String, Vec<Route>>) {
    println!("got config connection from {}", addr);
    let wp = write_packet(&mut socket, config.server_id.as_bytes());
    if wp.is_err() {
        return;
    }
    socket.set_read_timeout(Some(Duration::from_millis(1000))).unwrap();
    loop {
        let out = do_loop(config, socket, cipher, wg, routes);
        if let Err(out) = out {
            println!("errored out: {}", out);
            break;
        }
        socket = out.unwrap();
    }
}
fn do_loop(config: &mut Config, mut stream: TcpStream, cipher: &mut XChaCha20Poly1305, wg: &mut Wireguard, routes: &mut HashMap<String, Vec<Route>>) -> FFResult<TcpStream> {
    let cmd = read_encrypted_data(&mut stream, cipher)?;
    let cmd: Command = serde_json::from_slice(&cmd)?;
    stream.set_read_timeout(None).unwrap();//after getting the first packet with a timeout, we can wait however long we want
    match cmd {
        Command::Heartbeat(it) => {
            println!("got heartbeat! {}", it);
            let data = serde_json::to_vec(&Response::Heartbeat(it))?;
            write_encrypted_data(&mut stream, cipher, &data)?;
        },
        Command::GetIp => {
            println!("got getip command");
            let ip: Result<String, String> = minreq::get("https://ipinfo.io/json").send()
                .map(|it| String::from_utf8_lossy(it.as_bytes()).into())
                .map_err(|it| format!("error: {}", it));
            let data = serde_json::to_vec(&Response::GetIp(ip))?;
            write_encrypted_data(&mut stream, cipher, &data)?;
        },
        Command::GetRoutes => {
            println!("got getip command");
            let routes = get_routes();
            let data = serde_json::to_vec(&Response::Routes(routes))?;
            write_encrypted_data(&mut stream, cipher, &data)?;
        },
        Command::CreateWireguardPeer(it) => {
            let rt = add_to_wireguard(config, wg, &it);
            routes.insert(it.0.clone(), rt);
            config.add_peer(it);
            config.save();

            let routes = get_routes();
            let data = serde_json::to_vec(&Response::Routes(routes))?;
            write_encrypted_data(&mut stream, cipher, &data)?;
        },
        Command::RemoveWireguardPeer(it) => {
            let peer = wg.remove_peer(&it);
            routes.remove(&peer.public_key).unwrap().iter().for_each(|it|it.remove_self());

            let routes = get_routes();
            let data = serde_json::to_vec(&Response::Routes(routes))?;
            write_encrypted_data(&mut stream, cipher, &data)?;
        },
        Command::FireUDPWakeup(addr) => {
            let socket = UdpSocket::bind("0.0.0.0:0").unwrap();
            //socket.send_to(&[], addr).unwrap();
            println!("sending wakeup to... {}", addr);
        },
        Command::FireUDPShutdown(addr) => {
            let socket = UdpSocket::bind("0.0.0.0:0").unwrap();
            //socket.send_to(&[0], addr).unwrap();
            println!("sending shutdown to... {}", addr);
        },
        Command::GetIPv6Addr => {
            let ipv6 = get_pub_ipv6_addr(DEFAULT_DEVICE.get().unwrap());
            let data = serde_json::to_vec(&Response::Ipv6Addr(ipv6))?;
            write_encrypted_data(&mut stream, cipher, &data)?;
        }
        Command::Kill => {
            println!("shutdown");
            drop(stream);
            sleep(Duration::from_millis(1000));//allow stream closing to go through
            wg.kill();
            routes.iter_mut().for_each(|it| it.1.iter().for_each(|it| it.remove_self()));
            exit(0);
        }
    }
    Ok(stream)
}
static IDENTIFIER: AtomicU16 = AtomicU16::new(42);
pub fn ping(addr: IpAddr) -> FFResult<()> {
    let id = IDENTIFIER.fetch_add(1, SeqCst);
    match addr {
        IpAddr::V4(it) => {
            let mut socket = IcmpSocket4::try_from("0.0.0.0".parse::<Ipv4Addr>().unwrap())?;
            let packet4 = Icmpv4Packet::with_echo_request(id, 1, Vec::from("payload".as_bytes()))
                .map_err(|it| Box::new(ICMPPacketError(it)))?;
            socket.set_timeout(Some(Duration::from_millis(1250)));
            socket.send_to(it, packet4)?;
        }
        IpAddr::V6(it) => {
            let mut socket = IcmpSocket6::try_from("::0".parse::<Ipv6Addr>().unwrap())?;
            let packet6 = Icmpv6Packet::with_echo_request(id, 1, Vec::from("payload".as_bytes()))
                .map_err(|it| Box::new(ICMPPacketError(it)))?;
            socket.set_timeout(Some(Duration::from_millis(1250)));
            socket.send_to(it, packet6)?;
        }
    }
    Ok(())
}