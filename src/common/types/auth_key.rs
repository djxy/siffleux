use std::str::FromStr;

use crate::common::error::Error;
use argon2::{
    Argon2, PasswordHash, PasswordVerifier,
    password_hash::{PasswordHasher, SaltString},
};

#[derive(Debug, Clone)]
pub struct AuthKey(String);

impl AuthKey {
    pub fn new(value: &str) -> Result<Self, Error> {
        if value.is_empty() {
            return Err(Error::InvalidAuthKey {
                reason: format!("Auth key can't be empty."),
            });
        }

        let mut buf = [0u8; 16];

        getrandom::fill(&mut buf).unwrap();

        let salt = SaltString::encode_b64(&mut buf).unwrap();

        Ok(Self(
            Argon2::default()
                .hash_password(value.as_bytes(), &salt)
                .unwrap()
                .to_string(),
        ))
    }

    pub fn verify(&self, password: &str) -> bool {
        let password_hash = PasswordHash::new(&self.0).unwrap();

        Argon2::default()
            .verify_password(password.as_bytes(), &password_hash)
            .is_ok()
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
