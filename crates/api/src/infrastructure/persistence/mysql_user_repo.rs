//! MySQL adapter for [`UserRepository`]. Every request-facing lookup is
//! org-scoped; `find_by_email` / `find_by_id_any` are auth-internal only.

use std::str::FromStr;

use async_trait::async_trait;
use sqlx::MySqlPool;

use crate::application::error::{AppResult, ApplicationError};
use crate::application::ports::user_repo::UserRepository;
use crate::domain::ids::{OrgId, UserId};
use crate::domain::user::User;
use crate::domain::value_objects::{Email, Role};
use crate::infrastructure::persistence::{map_sqlx, parse_uuid, to_utc};

pub struct MySqlUserRepo {
    pool: MySqlPool,
}

impl MySqlUserRepo {
    pub fn new(pool: MySqlPool) -> Self {
        Self { pool }
    }
}

/// Shared row shape so each query maps the same way.
struct UserRow {
    id: String,
    organization_id: String,
    email: String,
    password_hash: String,
    full_name: String,
    role: String,
    is_active: bool,
    created_at: chrono::NaiveDateTime,
    updated_at: chrono::NaiveDateTime,
}

impl UserRow {
    fn into_domain(self) -> AppResult<User> {
        Ok(User {
            id: parse_uuid(&self.id)?,
            organization_id: parse_uuid(&self.organization_id)?,
            email: Email::parse(&self.email)?,
            password_hash: self.password_hash,
            full_name: self.full_name,
            role: Role::from_str(&self.role).map_err(ApplicationError::internal)?,
            is_active: self.is_active,
            created_at: to_utc(self.created_at),
            updated_at: to_utc(self.updated_at),
        })
    }
}

#[async_trait]
impl UserRepository for MySqlUserRepo {
    async fn create(&self, user: &User) -> AppResult<()> {
        sqlx::query!(
            r#"
            INSERT INTO users
              (id, organization_id, email, password_hash, full_name, role, is_active, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
            user.id.to_string(),
            user.organization_id.to_string(),
            user.email.as_str(),
            user.password_hash,
            user.full_name,
            user.role.as_str(),
            user.is_active,
            user.created_at.naive_utc(),
            user.updated_at.naive_utc(),
        )
        .execute(&self.pool)
        .await
        .map_err(|e| map_sqlx(e, "user"))?;
        Ok(())
    }

    async fn find_by_id(&self, org: OrgId, id: UserId) -> AppResult<User> {
        sqlx::query_as!(
            UserRow,
            r#"
            SELECT id, organization_id, email, password_hash, full_name, role,
                   is_active AS `is_active: bool`, created_at, updated_at
            FROM users WHERE id = ? AND organization_id = ?
            "#,
            id.to_string(),
            org.to_string(),
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| map_sqlx(e, "user"))?
        .into_domain()
    }

    async fn find_by_email(&self, email: &Email) -> AppResult<Option<User>> {
        let row = sqlx::query_as!(
            UserRow,
            r#"
            SELECT id, organization_id, email, password_hash, full_name, role,
                   is_active AS `is_active: bool`, created_at, updated_at
            FROM users WHERE email = ?
            "#,
            email.as_str(),
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| map_sqlx(e, "user"))?;
        row.map(UserRow::into_domain).transpose()
    }

    async fn find_by_id_any(&self, id: UserId) -> AppResult<User> {
        sqlx::query_as!(
            UserRow,
            r#"
            SELECT id, organization_id, email, password_hash, full_name, role,
                   is_active AS `is_active: bool`, created_at, updated_at
            FROM users WHERE id = ?
            "#,
            id.to_string(),
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| map_sqlx(e, "user"))?
        .into_domain()
    }

    async fn list(&self, org: OrgId, limit: i64, offset: i64) -> AppResult<(Vec<User>, i64)> {
        let rows = sqlx::query_as!(
            UserRow,
            r#"
            SELECT id, organization_id, email, password_hash, full_name, role,
                   is_active AS `is_active: bool`, created_at, updated_at
            FROM users WHERE organization_id = ?
            ORDER BY created_at ASC, id ASC
            LIMIT ? OFFSET ?
            "#,
            org.to_string(),
            limit,
            offset,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| map_sqlx(e, "user"))?;

        let total = sqlx::query!(
            r#"SELECT COUNT(*) AS `total!: i64` FROM users WHERE organization_id = ?"#,
            org.to_string(),
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| map_sqlx(e, "user"))?
        .total;

        let users = rows
            .into_iter()
            .map(UserRow::into_domain)
            .collect::<AppResult<Vec<_>>>()?;
        Ok((users, total))
    }

    async fn update(&self, user: &User) -> AppResult<()> {
        sqlx::query!(
            r#"
            UPDATE users
            SET full_name = ?, role = ?, is_active = ?, password_hash = ?, updated_at = ?
            WHERE id = ? AND organization_id = ?
            "#,
            user.full_name,
            user.role.as_str(),
            user.is_active,
            user.password_hash,
            user.updated_at.naive_utc(),
            user.id.to_string(),
            user.organization_id.to_string(),
        )
        .execute(&self.pool)
        .await
        .map_err(|e| map_sqlx(e, "user"))?;
        Ok(())
    }

    async fn delete(&self, org: OrgId, id: UserId) -> AppResult<()> {
        let res = sqlx::query!(
            r#"DELETE FROM users WHERE id = ? AND organization_id = ?"#,
            id.to_string(),
            org.to_string(),
        )
        .execute(&self.pool)
        .await
        .map_err(|e| map_sqlx(e, "user"))?;
        if res.rows_affected() == 0 {
            return Err(ApplicationError::not_found("user"));
        }
        Ok(())
    }

    async fn exists_by_email(&self, email: &Email) -> AppResult<bool> {
        let row = sqlx::query!(
            r#"SELECT COUNT(*) AS `count!: i64` FROM users WHERE email = ?"#,
            email.as_str(),
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| map_sqlx(e, "user"))?;
        Ok(row.count > 0)
    }
}
