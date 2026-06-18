//! JWT (HS256) access tokens + opaque refresh-token minting/hashing.

use std::str::FromStr;

use chrono::Utc;
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::application::error::{AppResult, ApplicationError};
use crate::application::ports::token_service::{AccessClaims, TokenService};
use crate::domain::ids::{OrgId, UserId};
use crate::domain::value_objects::Role;

#[derive(Serialize, Deserialize)]
struct Claims {
    sub: String,
    org: String,
    role: String,
    iat: i64,
    exp: i64,
}

pub struct JwtTokenService {
    encoding: EncodingKey,
    decoding: DecodingKey,
    access_ttl_secs: i64,
}

impl JwtTokenService {
    pub fn new(secret: &[u8], access_ttl_secs: i64) -> Self {
        Self {
            encoding: EncodingKey::from_secret(secret),
            decoding: DecodingKey::from_secret(secret),
            access_ttl_secs,
        }
    }
}

impl TokenService for JwtTokenService {
    fn issue_access(&self, user_id: UserId, org: OrgId, role: Role) -> AppResult<String> {
        let now = Utc::now().timestamp();
        let claims = Claims {
            sub: user_id.to_string(),
            org: org.to_string(),
            role: role.as_str().to_string(),
            iat: now,
            exp: now + self.access_ttl_secs,
        };
        encode(&Header::new(Algorithm::HS256), &claims, &self.encoding)
            .map_err(ApplicationError::internal)
    }

    fn verify_access(&self, token: &str) -> AppResult<AccessClaims> {
        let data = decode::<Claims>(token, &self.decoding, &Validation::new(Algorithm::HS256))
            .map_err(|_| ApplicationError::Unauthorized("invalid or expired token".into()))?;
        let c = data.claims;
        let bad = || ApplicationError::Unauthorized("invalid token claims".into());
        Ok(AccessClaims {
            sub: UserId::from_str(&c.sub).map_err(|_| bad())?,
            org: OrgId::from_str(&c.org).map_err(|_| bad())?,
            role: Role::from_str(&c.role).map_err(|_| bad())?,
            iat: c.iat,
            exp: c.exp,
        })
    }

    fn generate_refresh_token(&self) -> String {
        let mut bytes = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut bytes);
        to_hex(&bytes)
    }

    fn hash_refresh_token(&self, raw: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(raw.as_bytes());
        to_hex(&hasher.finalize())
    }
}

fn to_hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn svc(ttl: i64) -> JwtTokenService {
        JwtTokenService::new(b"0123456789012345678901234567890123", ttl)
    }

    #[test]
    fn issue_then_verify_roundtrip() {
        let s = svc(900);
        let uid = UserId::new();
        let org = OrgId::new();
        let token = s.issue_access(uid, org, Role::Admin).unwrap();
        let claims = s.verify_access(&token).unwrap();
        assert_eq!(claims.sub, uid);
        assert_eq!(claims.org, org);
        assert_eq!(claims.role, Role::Admin);
    }

    #[test]
    fn tampered_token_is_rejected() {
        let s = svc(900);
        let token = s
            .issue_access(UserId::new(), OrgId::new(), Role::Viewer)
            .unwrap();
        let mut bad = token.clone();
        bad.push('x');
        assert!(s.verify_access(&bad).is_err());
    }

    #[test]
    fn expired_token_is_rejected() {
        let s = svc(-120); // expired beyond jsonwebtoken's 60s clock-skew leeway
        let token = s
            .issue_access(UserId::new(), OrgId::new(), Role::Owner)
            .unwrap();
        assert!(s.verify_access(&token).is_err());
    }

    #[test]
    fn refresh_token_hash_is_stable_and_64_hex() {
        let s = svc(900);
        let raw = s.generate_refresh_token();
        assert_eq!(raw.len(), 64);
        assert_eq!(s.hash_refresh_token(&raw), s.hash_refresh_token(&raw));
        assert_eq!(s.hash_refresh_token(&raw).len(), 64);
    }
}
