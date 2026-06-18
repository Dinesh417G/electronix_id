//! MySQL adapter for [`RefreshTokenRepository`]. Only the SHA-256 hash of a
//! refresh token is ever stored; rotation revokes by setting `revoked_at`.

use async_trait::async_trait;
use chrono::Utc;
use sqlx::MySqlPool;

use crate::application::error::AppResult;
use crate::application::ports::refresh_token_repo::{RefreshTokenRecord, RefreshTokenRepository};
use crate::domain::ids::RefreshTokenId;
use crate::infrastructure::persistence::{map_sqlx, parse_uuid, to_utc};

pub struct MySqlRefreshTokenRepo {
    pool: MySqlPool,
}

impl MySqlRefreshTokenRepo {
    pub fn new(pool: MySqlPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl RefreshTokenRepository for MySqlRefreshTokenRepo {
    async fn create(&self, rec: &RefreshTokenRecord) -> AppResult<()> {
        sqlx::query!(
            r#"
            INSERT INTO refresh_tokens
              (id, user_id, token_hash, expires_at, revoked_at, created_at)
            VALUES (?, ?, ?, ?, ?, ?)
            "#,
            rec.id.to_string(),
            rec.user_id.to_string(),
            rec.token_hash,
            rec.expires_at.naive_utc(),
            rec.revoked_at.map(|t| t.naive_utc()),
            rec.created_at.naive_utc(),
        )
        .execute(&self.pool)
        .await
        .map_err(|e| map_sqlx(e, "refresh_token"))?;
        Ok(())
    }

    async fn find_by_hash(&self, token_hash: &str) -> AppResult<Option<RefreshTokenRecord>> {
        let row = sqlx::query!(
            r#"
            SELECT id, user_id, token_hash, expires_at, revoked_at, created_at
            FROM refresh_tokens WHERE token_hash = ?
            "#,
            token_hash,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| map_sqlx(e, "refresh_token"))?;

        row.map(|row| {
            Ok(RefreshTokenRecord {
                id: parse_uuid(&row.id)?,
                user_id: parse_uuid(&row.user_id)?,
                token_hash: row.token_hash,
                expires_at: to_utc(row.expires_at),
                revoked_at: row.revoked_at.map(to_utc),
                created_at: to_utc(row.created_at),
            })
        })
        .transpose()
    }

    async fn revoke(&self, id: RefreshTokenId) -> AppResult<()> {
        sqlx::query!(
            r#"UPDATE refresh_tokens SET revoked_at = ? WHERE id = ? AND revoked_at IS NULL"#,
            Utc::now().naive_utc(),
            id.to_string(),
        )
        .execute(&self.pool)
        .await
        .map_err(|e| map_sqlx(e, "refresh_token"))?;
        Ok(())
    }
}
