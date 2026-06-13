use std::fmt::{Display, Formatter};

#[derive(Debug)]
pub enum FFError {
    IP4Deserial,
    OutOfIds
}

impl Display for FFError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl std::error::Error for FFError {}
pub type FFResult<T> = core::result::Result<T, Box<dyn std::error::Error>>;