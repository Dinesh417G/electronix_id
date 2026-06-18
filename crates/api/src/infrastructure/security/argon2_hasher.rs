//! Argon2id password hashing adapter.

use argon2::Argon2;
use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::{PasswordHash, PasswordHasher as _, PasswordVerifier, SaltString};

use crate::application::error::{AppResult, ApplicationError};
use crate::application::ports::password_hasher::PasswordHasher;

#[derive(Default)]
pub struct Argon2Hasher {
    argon2: Argon2<'static>,
}

impl PasswordHasher for Argon2Hasher {
    fn hash(&self, password: &str) -> AppResult<String> {
        let salt = SaltString::generate(&mut OsRng);
        let hash = self
            .argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(ApplicationError::internal)?;
        Ok(hash.to_string())
    }

    fn verify(&self, password: &str, hash: &str) -> AppResult<bool> {
        let parsed = PasswordHash::new(hash).map_err(ApplicationError::internal)?;
        Ok(self
            .argon2
            .verify_password(password.as_bytes(), &parsed)
            .is_ok())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_then_verify_roundtrip() {
        let h = Argon2Hasher::default();
        let hash = h.hash("correct horse").unwrap();
        assert!(hash.starts_with("$argon2"));
        assert!(h.verify("correct horse", &hash).unwrap());
        assert!(!h.verify("wrong", &hash).unwrap());
    }
}
