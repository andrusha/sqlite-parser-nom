use std::fmt::{Display, Formatter};

#[derive(thiserror::Error, Debug)]
pub enum SQLiteError {
    #[error(transparent)]
    IOError(#[from] std::io::Error),

    #[error(transparent)]
    ParsingError(#[from] nom::error::Error<OwnedBytes>),

    #[error("unknown text encoding `{0}`")]
    UnknownTextEncodingError(u32),
}

/// Used so the error could outlive its input
#[derive(Debug)]
pub struct OwnedBytes(pub Vec<u8>);

impl From<Vec<u8>> for OwnedBytes {
    fn from(value: Vec<u8>) -> Self {
        OwnedBytes(value)
    }
}

impl Display for OwnedBytes {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.0.iter().fold(Ok(()), |result, byte| {
            result.and_then(|_| writeln!(f, "{:X} ", byte))
        })
    }
}
