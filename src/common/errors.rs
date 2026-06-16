use std::fmt::{Display, Formatter};
use chacha20poly1305::aead;

#[derive(Debug)]
pub enum FFError {
    IP4Deserial,
    OutOfIds,
    CipherError(aead::Error),
    NoGeneratorFoundError(String),
    WrongResponseType,
    WrongHeartbeat
}

impl Display for FFError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl std::error::Error for FFError {}
pub type FFResult<T> = core::result::Result<T, Box<dyn std::error::Error>>;