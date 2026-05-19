use crate::common::error::Error;
use std::fmt::{Display, Formatter};

const AUTH_KEY_MAX_LENGTH: usize = 255;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthKey(String);

impl AuthKey {
    pub fn new(value: String) -> Result<Self, Error> {
        if value.len() > AUTH_KEY_MAX_LENGTH {
            return Err(Error::InvalidAuthKey {
                reason: format!("Auth key too long. Max length: {AUTH_KEY_MAX_LENGTH}"),
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

impl TryFrom<&str> for AuthKey {
    type Error = Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value.to_string())
    }
}

impl TryFrom<String> for AuthKey {
    type Error = Error;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

const INGRESS_ID_MAX_LENGTH: usize = 255;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IngressId(String);

impl IngressId {
    pub fn new(value: String) -> Result<Self, Error> {
        if value.len() > INGRESS_ID_MAX_LENGTH {
            return Err(Error::InvalidIngressId {
                value,
                reason: format!("Ingress ID too long. Max length: {INGRESS_ID_MAX_LENGTH}"),
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

impl TryFrom<&str> for IngressId {
    type Error = Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value.to_string())
    }
}

impl TryFrom<String> for IngressId {
    type Error = Error;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl Display for IngressId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Ok(self.0.fmt(f)?)
    }
}

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

#[derive(PartialEq, Eq, Hash, Copy, Clone, Debug)]
pub struct TunnelId(u64);

impl TunnelId {
    pub fn from_bytes(bytes: [u8; 8]) -> Self {
        TunnelId(u64::from_be_bytes(bytes))
    }

    pub fn new(id: u64) -> Self {
        Self(id)
    }

    pub fn to_bytes(&self) -> [u8; 8] {
        self.0.to_be_bytes()
    }
}

impl Display for TunnelId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Ok(self.0.fmt(f)?)
    }
}
