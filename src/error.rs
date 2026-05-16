use crate::message::code::WRONG_AUTH_KEY;
use quinn::crypto::rustls::NoInitialCipherSuite;
use quinn::{ConnectError, ConnectionError, ReadError, ReadExactError, ReadToEndError, WriteError};
use std::string::FromUtf8Error;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Auth key rejected.")]
    AuthKeyRejected,

    #[error("Invalid auth_key reason={reason}")]
    InvalidAuthKey { reason: String },

    #[error("Invalid ingress_id={value}, reason={reason}")]
    InvalidIngressId { value: String, reason: String },

    #[error("Invalid tunnel_name={value}, reason={reason}")]
    InvalidTunnelName { value: String, reason: String },

    #[error("Incompatible version expected={expected}, received={received}")]
    IncompatibleVersion { expected: u8, received: u8 },

    #[error("Invalid data: {0}")]
    InvalidData(Box<dyn std::error::Error + Send + Sync>),

    #[error("Unknown error: {0}")]
    Unknown(Box<dyn std::error::Error + Send + Sync>),
}

impl From<ReadExactError> for Error {
    fn from(read_exact_error: ReadExactError) -> Self {
        match read_exact_error {
            ReadExactError::ReadError(read_error) => read_error.into(),
            _ => Error::Unknown(read_exact_error.into()),
        }
    }
}

impl From<ReadError> for Error {
    fn from(read_error: ReadError) -> Self {
        match read_error {
            ReadError::ConnectionLost(connection_error) => connection_error.into(),
            _ => Error::Unknown(read_error.into()),
        }
    }
}

impl From<ReadToEndError> for Error {
    fn from(read_to_end_error: ReadToEndError) -> Self {
        match read_to_end_error {
            ReadToEndError::Read(read_error) => read_error.into(),
            _ => Error::Unknown(read_to_end_error.into()),
        }
    }
}

impl From<WriteError> for Error {
    fn from(write_error: WriteError) -> Self {
        match write_error {
            WriteError::ConnectionLost(connection_error) => connection_error.into(),
            _ => Error::Unknown(write_error.into()),
        }
    }
}

impl From<ConnectError> for Error {
    fn from(connect_error: ConnectError) -> Self {
        match connect_error {
            _ => Error::Unknown(connect_error.into()),
        }
    }
}

impl From<NoInitialCipherSuite> for Error {
    fn from(no_initial_cipher_suite: NoInitialCipherSuite) -> Self {
        match no_initial_cipher_suite {
            _ => Error::Unknown(no_initial_cipher_suite.into()),
        }
    }
}

impl From<ConnectionError> for Error {
    fn from(connection_error: ConnectionError) -> Self {
        match connection_error {
            ConnectionError::ApplicationClosed(application_close)
                if application_close.error_code == WRONG_AUTH_KEY.code =>
            {
                Error::AuthKeyRejected
            }
            _ => Error::Unknown(connection_error.into()),
        }
    }
}

impl From<uuid::Error> for Error {
    fn from(value: uuid::Error) -> Self {
        Error::InvalidData(value.into())
    }
}

impl From<FromUtf8Error> for Error {
    fn from(value: FromUtf8Error) -> Self {
        Error::InvalidData(value.into())
    }
}

impl From<std::io::Error> for Error {
    fn from(io_error: std::io::Error) -> Self {
        match io_error {
            _ => Error::Unknown(io_error.into()),
        }
    }
}

impl From<rustls::Error> for Error {
    fn from(rustls_error: rustls::Error) -> Self {
        match rustls_error {
            _ => Error::Unknown(rustls_error.into()),
        }
    }
}
