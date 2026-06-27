use quinn::crypto::rustls::NoInitialCipherSuite;
use quinn::{
    ClosedStream, ConnectError, ConnectionError, ReadError, ReadExactError, ReadToEndError,
    WriteError,
};
use std::string::FromUtf8Error;
use std::sync::PoisonError;
use thiserror::Error;

use crate::code::{CONNECTION_EOF, REJECTED_AUTH_KEY, REJECTED_INGRESS_ID};
use crate::{EgressId, IngressId};

#[derive(Error, Debug)]
pub enum Error {
    #[error("Frame not received on time: {0}")]
    FrameNotReceivedOnTime(String),

    #[error("Rejected ingress Id.")]
    RejectedIngressId,

    #[error("Rejected auth key.")]
    RejectedAuthKey,

    #[error("Unexpected frame received: {0}")]
    UnexpectedFrameReceived(String),

    #[error("Unknown error: {0}")]
    Unknown(Box<dyn std::error::Error + Send + Sync>),

    #[error("Auth frame not received")]
    AuthFrameNotReceived,

    #[error("Connection closed")]
    ConnectionClosed,

    #[error("Closed tunnel")]
    ClosedTunnel,

    #[error("Closed stream")]
    ClosedStream,

    #[error("Egress is already started")]
    EgressAlreadyStarted,

    #[error("Egress is not started")]
    EgressNotStarted,

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

    #[error("Invalid egress_id reason={reason}")]
    InvalidEgressId { reason: String },

    #[error("Invalid server_id reason={reason}")]
    InvalidServerId { reason: String },

    #[error("Invalid tunnel_name reason={reason}")]
    InvalidTunnelName { reason: String },

    #[error("Incompatible version expected={expected}, received={received}")]
    IncompatibleVersion { expected: u8, received: u8 },

    #[error("Server is already listening")]
    ServerAlreadyListening,

    #[error("Server is not listening")]
    ServerNotListening,

    #[error("Ingress ID {0} already assigned")]
    IngressIDAlreadyAssigned(IngressId),

    #[error("Egress ID {0} already assigned")]
    EgressIDAlreadyAssigned(EgressId),

    #[error("Unknown error: {0}")]
    TLS(Box<dyn std::error::Error + Send + Sync>),

    #[error("IO error: {0}")]
    IO(Box<dyn std::error::Error + Send + Sync>),

    #[error("Lock poisoned: {0}")]
    PoisonLock(String),
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
                c if c == CONNECTION_EOF => Error::ClosedTunnel,
                c if c == REJECTED_AUTH_KEY => Error::RejectedAuthKey,
                c if c == REJECTED_INGRESS_ID => Error::RejectedIngressId,
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
        Error::Unknown(value.into())
    }
}

impl From<std::io::Error> for Error {
    fn from(io_error: std::io::Error) -> Self {
        match io_error.downcast::<quinn::WriteError>() {
            Ok(write_err) => write_err.into(),
            Err(io_error) => match io_error.downcast::<quinn::ReadError>() {
                Ok(read_err) => read_err.into(),
                Err(original_io_err) => Error::IO(original_io_err.into()),
            },
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

impl From<uuid::Error> for Error {
    fn from(uuid_err: uuid::Error) -> Self {
        match uuid_err {
            _ => Error::Unknown(uuid_err.into()),
        }
    }
}
