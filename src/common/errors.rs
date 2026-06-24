use std::fmt::{Display, Formatter};
use chacha20poly1305::aead;
use icmp_socket::packet::IcmpPacketBuildError;

#[derive(Debug)]
pub enum FFError {
    IP4Deserial,
    OutOfIds,
    CipherError(aead::Error),
    NoGeneratorFoundError(String),
    WrongResponseType,
    WrongHeartbeat,
    BadGetIp(String),
    GenShutdownWrong(String),
    ICMPPacketError(IcmpPacketBuildError)
}

impl Display for FFError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl std::error::Error for FFError {}
pub type FFResult<T> = core::result::Result<T, Box<dyn std::error::Error>>;