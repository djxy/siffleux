use crate::common::error::Error;

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
