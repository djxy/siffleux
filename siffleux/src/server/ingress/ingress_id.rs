use std::{
    fmt::{Display, Formatter},
    str::FromStr,
};

use crate::Error;

const INGRESS_ID_MAX_LENGTH: usize = 255;

#[derive(serde::Deserialize)]
#[serde(try_from = "String")]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IngressId(String);

impl IngressId {
    pub fn from_bytes(bytes: &[u8]) -> Result<IngressId, Error> {
        let auth_key_str = std::str::from_utf8(bytes).map_err(|_| Error::InvalidIngressId {
            reason: "Invalid ingress ID UTF8 bytes.".to_string(),
        })?;

        Ok(IngressId::new(auth_key_str)?)
    }

    pub fn new(value: &str) -> Result<Self, Error> {
        if value.is_empty() || value.len() > INGRESS_ID_MAX_LENGTH {
            return Err(Error::InvalidIngressId {
                reason: format!("Ingress ID has to be between 1 and 255 UTF8 bytes."),
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

impl TryFrom<&str> for IngressId {
    type Error = Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<String> for IngressId {
    type Error = Error;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(&value)
    }
}

impl FromStr for IngressId {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

impl Display for IngressId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Ok(self.0.fmt(f)?)
    }
}
