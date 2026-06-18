//! Token port: issues/verifies short-lived JWT access tokens and mints/hashes
//! opaque refresh tokens. Sync — no IO, just crypto.

use crate::application::error::AppResult;
use crate::domain::ids::{OrgId, UserId};
use crate::domain::value_objects::Role;

/// Decoded access-token claims.
#[derive(Debug, Clone)]
pub struct AccessClaims {
    pub sub: UserId,
    pub org: OrgId,
    pub role: Role,
    pub iat: i64,
    pub exp: i64,
}

pub trait TokenService: Send + Sync {
    fn issue_access(&self, user_id: UserId, org: OrgId, role: Role) -> AppResult<String>;
    fn verify_access(&self, token: &str) -> AppResult<AccessClaims>;
    /// 32 random bytes, hex-encoded. Returned to the client; never stored raw.
    fn generate_refresh_token(&self) -> String;
    /// SHA-256 hex of a raw refresh token — this is what we persist/look up.
    fn hash_refresh_token(&self, raw: &str) -> String;
}
