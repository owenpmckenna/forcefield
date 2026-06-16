use crate::citadel::state::BackendState;
use crate::common::commands::{Command, Response};
use crate::common::errors::FFError::{NoGeneratorFoundError, WrongHeartbeat, WrongResponseType};
use crate::common::errors::FFResult;
use crate::common::setup_handshake::{read_encrypted_data, read_packet, write_encrypted_data};
use chacha20poly1305::XChaCha20Poly1305;
use rand::Rng;
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

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
    fn write_encrypted_data(&mut self, data: &[u8]) -> FFResult<()> {
        write_encrypted_data(&mut self.stream, &self.cipher, data)
    }
    fn read_encrypted_data(&mut self) -> FFResult<Vec<u8>> {
        read_encrypted_data(&mut self.stream, &self.cipher)
    }
}