//! Password hashing port. Sync — Argon2 is CPU-bound, not IO.

use crate::application::error::AppResult;

pub trait PasswordHasher: Send + Sync {
    /// Produce a PHC-format hash string for storage.
    fn hash(&self, password: &str) -> AppResult<String>;
    /// Constant-time-ish verification against a stored hash.
    fn verify(&self, password: &str, hash: &str) -> AppResult<bool>;
}
