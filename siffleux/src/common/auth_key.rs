use std::str::FromStr;

use crate::common::error::Error;

const MAX_LENGTH: usize = 255;

#[derive(serde::Deserialize)]
#[serde(try_from = "String")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthKey(String);

impl AuthKey {
    pub fn from_bytes(bytes: &[u8]) -> Result<AuthKey, Error> {
        let auth_key_str = std::str::from_utf8(bytes).map_err(|_| Error::InvalidAuthKey {
            reason: "Invalid auth key UTF8 bytes.".to_string(),
        })?;

        Ok(AuthKey::new(auth_key_str)?)
    }

    pub fn new(value: &str) -> Result<Self, Error> {
        if value.is_empty() {
            return Err(Error::InvalidAuthKey {
                reason: format!("Auth key can't be empty."),
            });
        }
        if value.len() > MAX_LENGTH {
            return Err(Error::InvalidAuthKey {
                reason: format!("Auth key max length is 255 bytes."),
            });
        }

        Ok(Self(value.to_string()))
    }

    pub fn to_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<&str> for AuthKey {
    type Error = Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<String> for AuthKey {
    type Error = Error;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(&value)
    }
}

impl FromStr for AuthKey {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}
