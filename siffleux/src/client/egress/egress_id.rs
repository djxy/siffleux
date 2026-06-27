use std::{
    fmt::{Display, Formatter},
    str::FromStr,
};

use crate::Error;

const EGRESS_ID_MAX_LENGTH: usize = 255;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EgressId(String);

impl EgressId {
    pub fn from_bytes(bytes: &[u8]) -> Result<EgressId, Error> {
        let auth_key_str = std::str::from_utf8(bytes).map_err(|_| Error::InvalidEgressId {
            reason: "Invalid egress ID UTF8 bytes.".to_string(),
        })?;

        Ok(EgressId::new(auth_key_str)?)
    }

    pub fn new(value: &str) -> Result<Self, Error> {
        if value.is_empty() || value.len() > EGRESS_ID_MAX_LENGTH {
            return Err(Error::InvalidEgressId {
                reason: format!("Egress ID has to be between 1 and 255 UTF8 bytes."),
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

impl TryFrom<&str> for EgressId {
    type Error = Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<String> for EgressId {
    type Error = Error;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(&value)
    }
}

impl FromStr for EgressId {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

impl Display for EgressId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Ok(self.0.fmt(f)?)
    }
}
