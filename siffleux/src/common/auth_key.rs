use std::str::FromStr;

use crate::common::error::Error;
use argon2::{
    Argon2, PasswordHash, PasswordVerifier,
    password_hash::{PasswordHasher, SaltString},
};

const MAX_LENGTH: usize = 255;

#[derive(Debug, Clone)]
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

    pub fn hash(&self) -> HashedAuthKey {
        HashedAuthKey::new(self)
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

#[derive(Debug, Clone)]
pub struct HashedAuthKey(String);

impl HashedAuthKey {
    fn new(auth_key: &AuthKey) -> Self {
        let mut buf = [0u8; 16];

        getrandom::fill(&mut buf).unwrap();

        let salt = SaltString::encode_b64(&mut buf).unwrap();

        Self(
            Argon2::default()
                .hash_password(auth_key.to_str().as_bytes(), &salt)
                .unwrap()
                .to_string(),
        )
    }

    /// Verify if the auth key matches the hashed auth key
    pub fn verify(&self, auth_key: &AuthKey) -> bool {
        let password_hash = PasswordHash::new(&self.0).unwrap();

        Argon2::default()
            .verify_password(auth_key.to_str().as_bytes(), &password_hash)
            .is_ok()
    }
}
