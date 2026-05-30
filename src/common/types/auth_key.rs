use std::str::FromStr;

use crate::common::error::Error;

const AUTH_KEY_MAX_LENGTH: usize = 255;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthKey(String);

impl AuthKey {
    pub fn new(value: &str) -> Result<Self, Error> {
        if value.is_empty() || value.len() > AUTH_KEY_MAX_LENGTH {
            return Err(Error::InvalidAuthKey {
                reason: format!("Auth key has to be between 1 and 255 UTF8 bytes."),
            });
        }

        Ok(Self(value.to_string()))
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
