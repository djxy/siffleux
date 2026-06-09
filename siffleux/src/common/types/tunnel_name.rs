use crate::common::error::Error;
use std::{
    fmt::{Display, Formatter},
    str::FromStr,
};

const TUNNEL_NAME_MAX_LENGTH: usize = 255;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TunnelName(String);

impl TunnelName {
    pub fn from_bytes(bytes: &[u8]) -> Result<TunnelName, Error> {
        let auth_key_str = std::str::from_utf8(bytes).map_err(|_| Error::InvalidTunnelName {
            reason: "Invalid tunnel name UTF8 bytes.".to_string(),
        })?;

        Ok(TunnelName::new(auth_key_str)?)
    }

    fn new(value: &str) -> Result<Self, Error> {
        if value.len() > TUNNEL_NAME_MAX_LENGTH {
            return Err(Error::InvalidIngressId {
                reason: format!("Tunnel name too long. Max length: {TUNNEL_NAME_MAX_LENGTH}"),
            });
        }

        Ok(Self(value.to_string()))
    }

    pub fn to_str(&self) -> &str {
        &self.0
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }
}

impl TryFrom<&str> for TunnelName {
    type Error = Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<String> for TunnelName {
    type Error = Error;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(&value)
    }
}

impl Display for TunnelName {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Ok(self.0.fmt(f)?)
    }
}

impl FromStr for TunnelName {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        TunnelName::try_from(s)
    }
}
