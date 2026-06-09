use crate::IngressId;
use crate::codes::{AUTH_KEY_REJECTED, CLOSED, INGRESS_ID_REJECTED};
use quinn::crypto::rustls::NoInitialCipherSuite;
use quinn::{
    ClosedStream, ConnectError, ConnectionError, ReadError, ReadExactError, ReadToEndError,
    WriteError,
};
use std::string::FromUtf8Error;
use std::sync::PoisonError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("First frame received not auth")]
    FirstFrameReceivedNotAuth,

    #[error("Auth frame not received")]
    AuthFrameNotReceived,

    #[error("Auth key rejected.")]
    AuthKeyRejected,

    #[error("Closed tunnel")]
    ClosedTunnel,

    #[error("Closed stream")]
    ClosedStream,

    #[error("Egress is already listening")]
    EgressAlreadyListening,

    #[error("Egress is not listening")]
    EgressNotListening,

    #[error("Ingress Id rejected.")]
    IngressIdRejected,

    #[error("Ingress has no tunnel connected")]
    IngressNoTunnelConnected,

    #[error("Ingress is already listening")]
    IngressAlreadyListening,

    #[error("Ingress is not listening")]
    IngressNotListening,

    #[error("Invalid auth_key reason={reason}")]
    InvalidAuthKey { reason: String },

    #[error("Invalid ingress_id reason={reason}")]
    InvalidIngressId { reason: String },

    #[error("Invalid tunnel_name reason={reason}")]
    InvalidTunnelName { reason: String },

    #[error("Incompatible version expected={expected}, received={received}")]
    IncompatibleVersion { expected: u8, received: u8 },

    #[error("Invalid data: {0}")]
    InvalidData(Box<dyn std::error::Error + Send + Sync>),

    #[error("Server is already listening")]
    ServerAlreadyListening,

    #[error("Server is not listening")]
    ServerNotListening,

    #[error("Ingress ID {0} already assigned")]
    IngressIDAlreadyAssigned(IngressId),

    #[error("Unknown error: {0}")]
    TLS(Box<dyn std::error::Error + Send + Sync>),

    #[error("IO error: {0}")]
    IO(Box<dyn std::error::Error + Send + Sync>),

    #[error("Lock poisoned: {0}")]
    PoisonLock(String),

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

impl From<rustls::Error> for Error {
    fn from(error: rustls::Error) -> Self {
        Error::TLS(error.into())
    }
}

impl From<ClosedStream> for Error {
    fn from(closed_stream: ClosedStream) -> Self {
        match closed_stream {
            _ => Error::ClosedStream,
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

const CERTIFICATE_HASH_MISMATCH_CODE: u64 = 296;

impl From<ConnectionError> for Error {
    fn from(connection_error: ConnectionError) -> Self {
        match connection_error {
            ConnectionError::ApplicationClosed(ac) => match ac.error_code {
                c if c == CLOSED.code => Error::ClosedTunnel,
                c if c == AUTH_KEY_REJECTED.code => Error::AuthKeyRejected,
                c if c == INGRESS_ID_REJECTED.code => Error::IngressIdRejected,
                _ => Error::Unknown(ConnectionError::ApplicationClosed(ac).into()),
            },
            ConnectionError::TransportError(te) => match u64::from(te.code) {
                c if c == CERTIFICATE_HASH_MISMATCH_CODE => Error::TLS(te.into()),
                _ => Error::Unknown(te.into()),
            },
            _ => Error::Unknown(connection_error.into()),
        }
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
            _ => Error::IO(io_error.into()),
        }
    }
}

impl<T> From<PoisonError<T>> for Error {
    fn from(poison_error: PoisonError<T>) -> Self {
        match poison_error {
            _ => Error::PoisonLock(poison_error.to_string()),
        }
    }
}
