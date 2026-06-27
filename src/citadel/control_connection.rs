use crate::citadel::state::BackendState;
use crate::common::commands::{Command, Response};
use crate::common::errors::FFError::{BadGetIp, GenShutdownWrong, NoGeneratorFoundError, WrongHeartbeat, WrongResponseType};
use crate::common::errors::{FFError, FFResult};
use crate::common::setup_handshake::{read_encrypted_data, read_packet, write_encrypted_data};
use chacha20poly1305::XChaCha20Poly1305;
use rand::Rng;
use std::io::{ErrorKind, Read};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, TcpStream};
use std::time::Duration;
use crate::common::wireguard::Route;

pub struct ControlConnection {
    pub server_id: String,
    stream: TcpStream,
    cipher: XChaCha20Poly1305
}
impl ControlConnection {
    pub fn connect(addr: SocketAddr, state: &BackendState) -> FFResult<ControlConnection> {
        let mut stream = TcpStream::connect_timeout(&addr, Duration::new(5, 0))?;
        stream.set_read_timeout(Some(Duration::new(5, 0)))?;
        stream.set_write_timeout(Some(Duration::new(5, 0)))?;

        let bytes = read_packet(&mut stream)?;
        let id = String::from_utf8(bytes)?;
        let ge = state.known_generators.iter().find(|it| it.id.eq(&id));
        if let Some(ge) = ge {
            let mut conn = ControlConnection {
                server_id: id,
                stream,
                cipher: ge.get_cipher(),
            };
            conn.send_heartbeat()?;
            Ok(conn)
        } else {Err(Box::new(NoGeneratorFoundError(id)))}
    }
    pub fn send_heartbeat(&mut self) -> FFResult<()> {
        let hb_num = rand::rng().next_u64() as usize;
        let hb0 = Command::Heartbeat(hb_num);
        let hb = serde_json::to_string(&hb0)?;
        self.write_encrypted_data(hb.as_bytes())?;
        let resp = self.read_encrypted_data()?;
        let hb_resp: Response = serde_json::from_str(&String::from_utf8(resp)?)?;
        match hb_resp {
            Response::Heartbeat(it) => {
                if it == hb_num {
                    Ok(())
                } else {
                    Err(Box::new(WrongHeartbeat))
                }
            }
            _ => {
                Err(Box::new(WrongResponseType))
            }
        }
    }
    pub fn send_get_ip(&mut self) -> FFResult<String> {
        let ip0 = Command::GetIp;
        let ip = serde_json::to_string(&ip0)?;
        self.write_encrypted_data(ip.as_bytes())?;
        let resp = self.read_encrypted_data()?;
        let hb_resp: Response = serde_json::from_str(&String::from_utf8(resp)?)?;
        match hb_resp {
            Response::GetIp(it) => {
                match it {
                    Ok(it) => {Ok(it)}
                    Err(it) => {Err(Box::new(BadGetIp(it)))}
                }
            }
            _ => {
                Err(Box::new(WrongResponseType))
            }
        }
    }

    pub fn send_get_routes(&mut self) -> FFResult<Vec<Route>> {
        let r0 = Command::GetRoutes;
        self.write_read_routes(r0)
    }
    pub fn send_kill(&mut self) -> FFResult<()> {
        let k0 = Command::Kill;
        let k = serde_json::to_string(&k0)?;
        self.write_encrypted_data(k.as_bytes())?;
        let mut buf = [0u8; 1];
        match self.stream.read(&mut buf) {
            Ok(it) => {Err(Box::from(GenShutdownWrong("Read Data".into())))}
            Err(it) => {
                match it.kind() {
                    ErrorKind::TimedOut => {Err(Box::from(GenShutdownWrong("Timed Out".into())))}
                    _ => {Ok(())}
                }
            }
        }
    }
    pub fn order_create_wg(&mut self, peer_wg_pub: &str, peer_internal_ipv4: Ipv4Addr, peer_internal_ipv6: Ipv6Addr, endpoint: Option<SocketAddr>) -> FFResult<Vec<Route>> {
        let cwp0 = Command::CreateWireguardPeer((peer_wg_pub.into(), (peer_internal_ipv4, peer_internal_ipv6), endpoint));
        self.write_read_routes(cwp0)
    }
    pub fn order_delete_wg(&mut self, peer_wg_pub: &str) -> FFResult<Vec<Route>> {
        let cwp0 = Command::RemoveWireguardPeer(peer_wg_pub.into());
        self.write_read_routes(cwp0)
    }
    fn write_read_routes(&mut self, cmd: Command) -> FFResult<Vec<Route>> {
        let cwp = serde_json::to_string(&cmd)?;
        self.write_encrypted_data(cwp.as_bytes())?;
        let resp = self.read_encrypted_data()?;
        let hb_resp: Response = serde_json::from_str(&String::from_utf8(resp)?)?;
        match hb_resp {
            Response::Routes(it) => {
                Ok(it)
            }
            _ => {
                Err(Box::new(WrongResponseType))
            }
        }
    }
    pub fn send_wakeup_to(&mut self, address: SocketAddr) -> FFResult<()> {
        let cmd = Command::FireUDPWakeup(address);
        let cwp = serde_json::to_string(&cmd)?;
        self.write_encrypted_data(cwp.as_bytes())?;
        Ok(())
    }
    pub fn send_get_ipv6(&mut self) -> FFResult<Option<Ipv6Addr>> {
        let ip0 = Command::GetIPv6Addr;
        let ip = serde_json::to_string(&ip0)?;
        self.write_encrypted_data(ip.as_bytes())?;
        let resp = self.read_encrypted_data()?;
        let hb_resp: Response = serde_json::from_str(&String::from_utf8(resp)?)?;
        match hb_resp {
            Response::Ipv6Addr(it) => {
                Ok(it)
            }
            _ => {
                Err(Box::new(WrongResponseType))
            }
        }
    }
    pub fn send_shutdown_to(&mut self, address: SocketAddr) -> FFResult<()> {
        let cmd = Command::FireUDPShutdown(address);
        let cwp = serde_json::to_string(&cmd)?;
        self.write_encrypted_data(cwp.as_bytes())?;
        Ok(())
    }
    fn write_encrypted_data(&mut self, data: &[u8]) -> FFResult<()> {
        write_encrypted_data(&mut self.stream, &self.cipher, data)
    }
    fn read_encrypted_data(&mut self) -> FFResult<Vec<u8>> {
        read_encrypted_data(&mut self.stream, &self.cipher)
    }
}