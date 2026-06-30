use std::{
    fmt::{Display, Formatter},
    str::FromStr,
};

use crate::Error;

const SERVER_ID_MAX_LENGTH: usize = 255;

#[derive(serde::Deserialize)]
#[serde(try_from = "String")]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ServerId(String);

impl ServerId {
    pub fn from_bytes(bytes: &[u8]) -> Result<ServerId, Error> {
        let utf8_str = std::str::from_utf8(bytes).map_err(|_| Error::InvalidServerId {
            reason: "Invalid server ID UTF8 bytes.".to_string(),
        })?;

        Ok(ServerId::new(utf8_str)?)
    }

    pub fn new(value: &str) -> Result<Self, Error> {
        if value.is_empty() || value.len() > SERVER_ID_MAX_LENGTH {
            return Err(Error::InvalidServerId {
                reason: format!("Server ID has to be between 1 and 255 UTF8 bytes."),
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

impl TryFrom<&str> for ServerId {
    type Error = Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<String> for ServerId {
    type Error = Error;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(&value)
    }
}

impl FromStr for ServerId {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

impl Display for ServerId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Ok(self.0.fmt(f)?)
    }
}
