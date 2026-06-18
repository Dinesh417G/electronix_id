//! NOTE: Not in the §4 port list. Refresh-token rows must be persisted (with
//! rotation and revocation), and that is DB work, so it goes through a repository
//! rather than leaking into the JWT `token_service`. (Deviation reported in summary.)

use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::application::error::AppResult;
use crate::domain::ids::{RefreshTokenId, UserId};

/// A stored refresh token. Only the SHA-256 hash of the raw token is kept.
#[derive(Debug, Clone)]
pub struct RefreshTokenRecord {
    pub id: RefreshTokenId,
    pub user_id: UserId,
    pub token_hash: String,
    pub expires_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

impl RefreshTokenRecord {
    pub fn is_usable(&self, now: DateTime<Utc>) -> bool {
        self.revoked_at.is_none() && self.expires_at > now
    }
}

#[async_trait]
pub trait RefreshTokenRepository: Send + Sync {
    async fn create(&self, rec: &RefreshTokenRecord) -> AppResult<()>;
    async fn find_by_hash(&self, token_hash: &str) -> AppResult<Option<RefreshTokenRecord>>;
    async fn revoke(&self, id: RefreshTokenId) -> AppResult<()>;
}
