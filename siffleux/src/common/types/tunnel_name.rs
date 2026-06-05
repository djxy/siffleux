use crate::common::error::Error;
use std::{
    fmt::{Display, Formatter},
    str::FromStr,
};

const TUNNEL_NAME_MAX_LENGTH: usize = 255;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TunnelName(String);

impl TunnelName {
    fn new(value: String) -> Result<Self, Error> {
        if value.len() > TUNNEL_NAME_MAX_LENGTH {
            return Err(Error::InvalidIngressId {
                value,
                reason: format!("Tunnel name too long. Max length: {TUNNEL_NAME_MAX_LENGTH}"),
            });
        }

        Ok(Self(value))
    }

    pub fn value(&self) -> &str {
        &self.0
    }

    pub fn len(&self) -> u8 {
        self.0.len() as u8
    }
}

impl TryFrom<&str> for TunnelName {
    type Error = Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value.to_string())
    }
}

impl TryFrom<String> for TunnelName {
    type Error = Error;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
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
