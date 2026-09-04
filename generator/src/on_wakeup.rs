use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::sync::atomic::AtomicU16;
use std::sync::atomic::Ordering::SeqCst;
use std::time::Duration;
use icmp_socket::{IcmpSocket, IcmpSocket4, IcmpSocket6, Icmpv4Packet, Icmpv6Packet};
use icmp_socket::packet::WithEchoRequest;
use common::errors::FFError::ICMPPacketError;
use common::errors::FFResult;
use common::wireguard::{Route, Wireguard};
use crate::config::Config;

pub fn do_wakeup(x: &mut Config, x0: &mut HashMap<String, Vec<Route>>, x1: &mut Wireguard) {
    for i in &x1.peers {
        for i in &i.allowed_ips {
            if i.prefix_len() == i.max_prefix_len() {
                let _ = ping(i.addr());
            }
        }
    }
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