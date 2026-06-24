use crate::common::cmd::exec;
use crate::common::commands::{Command, Response};
use crate::common::errors::FFError::ICMPPacketError;
use crate::common::errors::{FFError, FFResult};
use crate::common::setup_handshake::{read_encrypted_data, read_packet, write_encrypted_data, write_packet, ConfigMessage};
use crate::common::wireguard::{get_default_route, get_routes, Route, Wireguard, WireguardPeer};
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
use std::thread::sleep;
use std::time::Duration;

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
    println!("read and decrypted config packet. Id: {}, ip: {}", out.server_id, out.server_ip);
    Ok(out)
}
fn add_to_wireguard(config: &mut Config, wg: &mut Wireguard, peer: &(String, IpAddr, Option<SocketAddr>)) -> Route {
    let wgp = WireguardPeer::new(peer.0.clone(), vec![peer.1.into()], peer.2);
    let local_ip = &config.server_ip;
    let route = Route::new(Some(peer.1.into()), None, Some("uwu0".into()), Some(local_ip.parse().unwrap()));
    wg.spawn_peer(&wgp);
    wg.peers.push(wgp);
    route.add_self();
    if let Some(mut net) = route.addresses.as_ref().map(|it| it.hosts()) && peer.2.is_some() {
        let ipaddr = net.next().unwrap();
        let _ = ping(ipaddr);//wakeup
    };
    route
}
fn run(mut config: Config) {
    let def_dev = get_default_route().device.unwrap();
    exec(format!("sudo iptables -t nat -A POSTROUTING -s 10.69.0.0/16 -o {} -j MASQUERADE", def_dev));
    exec(format!("iptables -A FORWARD -i uwu0 -o {} -j ACCEPT", def_dev));
    exec(format!("iptables -A FORWARD -i {} -o uwu0 -m conntrack --ctstate ESTABLISHED,RELATED -j ACCEPT", def_dev));
    let peers = vec![WireguardPeer::new(config.citadel_wg_pub.clone(), vec!["10.69.0.1/32".parse().unwrap()], None)];
    let mut wg = Wireguard::new(config.port, config.gen_wg_priv.clone(), config.gen_wg_pub.clone(), "uwu0".to_string(), IpAddr::from_str(&config.server_ip).unwrap(), peers);
    let mut routes = HashMap::new();
    wg.spawn();
    for i in config.get_peers() {
        routes.insert(i.0.clone(), add_to_wireguard(&mut config, &mut wg, &i));
    }
    let mut cipher = XChaCha20Poly1305::new(config.get_config_key());
    let listener_data = prep_receive_connections(config.config_port);
    let udp_listener_data = prep_receive_udp_connection(config.config_port);
    loop {
        select! {
            recv(listener_data) -> msg => {
                if let Ok((stream, addr)) = msg {
                    handle_socket(addr.ip(), stream, &mut config, &mut cipher, &mut wg, &mut routes);
                }
            },
            recv(udp_listener_data) -> msg => {
                if let Ok(it) = msg {if it.1.is_empty() {
                    handle_wakeup_packet(it.0.ip(), &mut wg);
                } else {handle_shutdown_time(&mut wg);}}
            },
            default(Duration::from_secs(300)) => {
                do_wakeup(&mut config, &mut routes, &mut wg);
                handle_shutdown_time(&mut wg);
            },
        };
    }
}
fn handle_socket(addr: IpAddr, mut socket: TcpStream, config: &mut Config, cipher: &mut XChaCha20Poly1305, wg: &mut Wireguard, routes: &mut HashMap<String, Route>) {
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
fn do_loop(config: &mut Config, mut stream: TcpStream, cipher: &mut XChaCha20Poly1305, wg: &mut Wireguard, routes: &mut HashMap<String, Route>) -> FFResult<TcpStream> {
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
            let data = serde_json::to_vec(&Response::GetIpResponse(ip))?;
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
            routes.remove(&peer.public_key).unwrap().remove_self();

            let routes = get_routes();
            let data = serde_json::to_vec(&Response::Routes(routes))?;
            write_encrypted_data(&mut stream, cipher, &data)?;
        },
        Command::FireUDPWakeup(addr) => {
            let socket = UdpSocket::bind("0.0.0.0:0").unwrap();
            socket.send_to(&[], addr).unwrap();
            println!("sending wakeup to... {}", addr);
        },
        Command::FireUDPShutdown(addr) => {
            let socket = UdpSocket::bind("0.0.0.0:0").unwrap();
            socket.send_to(&[0], addr).unwrap();
            println!("sending shutdown to... {}", addr);
        },
        Command::Kill => {
            println!("shutdown");
            drop(stream);
            sleep(Duration::from_millis(1000));//allow stream closing to go through
            wg.kill();
            routes.iter_mut().for_each(|it| it.1.remove_self());
            exit(0);
        }
    }
    Ok(stream)
}
fn handle_shutdown_time(wg: &mut Wireguard) {
    //if none with no peer, then the 10.69.0.1/32 route is on the default endpoint.
    if let Some(ind) = wg.peers.iter().position(|it| it.allowed_ips.is_empty()) {
        remove_peer_with_two(wg);
        wg.add_allowed_ip_to_peer(ind, "10.69.0.1/32".parse().unwrap());
    }
}
fn remove_peer_with_two(wg: &mut Wireguard) {
    for (index, peer) in &mut wg.peers.iter().enumerate() {
        if peer.allowed_ips.len() == 2 {
            let pos = peer.allowed_ips.iter().position(|it| it.eq(&("10.69.0.1/32".parse().unwrap())));
            wg.remove_allowed_ip_from_peer(index, pos.unwrap());
            return;
        }
    }
}
fn handle_wakeup_packet(src_ip: IpAddr, wg: &mut Wireguard) {
    let home: IpAddr = "10.69.0.1".parse().unwrap();
    let home_connecting = src_ip.eq(&home);
    let home_net = IpNet::new_assert(home, 32);
    //this is the id of the peer currently able to receive data from home
    let ind_w_home = wg.peers.iter().position(|it| it.allowed_ips.contains(&home_net)).unwrap();
    let peer_w_home: &mut WireguardPeer = &mut wg.peers[ind_w_home];
    if peer_w_home.allowed_ips.len() == 1 {//this means, we were getting a connection direct from the citadel
        if home_connecting {
            println!("wk state 0");
            return;//already good. Home is connecting and their address is in the right spot
        } else {
            println!("wk state 1");
            //Bad. Citadel isn't connecting directly and their address is in the spot for direct connections
            wg.remove_allowed_ip_from_peer(ind_w_home, 0);
        }
    } else {
        if peer_w_home.allowed_ips.contains(&IpNet::new_assert(src_ip, 32)) {
            println!("wk state 2");
            //already good. Citadel addr in right spot (along with the ip of our preceding peer)
            return;
        } else {
            println!("wk state 3");
            //Bad. Citadel is connecting via a new peer, their ip is associated with another peer though. remove it.
            let pos = peer_w_home.allowed_ips.iter().position(|it| it.eq(&home_net));
            wg.remove_allowed_ip_from_peer(ind_w_home, pos.unwrap());
        }
    }
    //at this point, citadel has been removed if it was in the wrong spot, or we returned. We now need to add it back.
    //we get the index of the peer which includes the src address of the packet signaling that future vpn stuff will come from that direction.
    let correct_id = wg.peers.iter().position(|it| it.allowed_ips.contains(&IpNet::new_assert(src_ip, 32))).unwrap();
    println!("adding correct ip to peer: id{}, peer {}", correct_id, wg.peers[correct_id].public_key);
    wg.add_allowed_ip_to_peer(correct_id, home_net);
}
static IDENTIFIER: AtomicU16 = AtomicU16::new(42);
fn ping(addr: IpAddr) -> FFResult<()> {
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