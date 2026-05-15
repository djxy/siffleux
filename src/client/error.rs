use quinn::{ConnectionError, ReadError, ReadExactError};
use std::error::Error;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum TunnelError {
    #[error("Wrong auth key.")]
    WrongAuthKey,

    #[error("Wrong auth key.")]
    Closed,

    #[error("Unknown error.")]
    Unknown,
}

impl Into<TunnelError> for ReadExactError {
    fn into(self) -> TunnelError {
        match self {
            ReadExactError::FinishedEarly(_) => TunnelError::Closed,
            ReadExactError::ReadError(e) => e.into(),
        }
    }
}

impl Into<TunnelError> for ReadError {
    fn into(self) -> TunnelError {
        match self {
            ReadError::ConnectionLost(e) => e.into(),
            _ => TunnelError::Unknown,
        }
    }
}

impl Into<TunnelError> for ConnectionError {
    fn into(self) -> TunnelError {
        match self {
            ConnectionError::ConnectionClosed(e) => match e.error_code {
                (e) => match e {
                     => TunnelError::Closed,
                    _ => TunnelError::Closed,
                },
                _ => TunnelError::Unknown,
            },
            _ => TunnelError::Unknown,
        }
    }
}
