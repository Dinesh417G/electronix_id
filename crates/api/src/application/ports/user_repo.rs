use async_trait::async_trait;

use crate::application::error::AppResult;
use crate::domain::ids::{OrgId, UserId};
use crate::domain::user::User;
use crate::domain::value_objects::Email;

#[async_trait]
pub trait UserRepository: Send + Sync {
    async fn create(&self, user: &User) -> AppResult<()>;
    /// Org-scoped lookup. A user from org A must never resolve org B's users.
    async fn find_by_id(&self, org: OrgId, id: UserId) -> AppResult<User>;
    /// Login path only: email is globally unique, so this resolves the org.
    async fn find_by_email(&self, email: &Email) -> AppResult<Option<User>>;
    /// Auth-internal only (refresh path): a refresh token maps to exactly one
    /// user, so loading that user — with its org — is safe and necessary.
    /// Never call this from request handlers; use the org-scoped `find_by_id`.
    async fn find_by_id_any(&self, id: UserId) -> AppResult<User>;
    async fn list(&self, org: OrgId, limit: i64, offset: i64) -> AppResult<(Vec<User>, i64)>;
    async fn update(&self, user: &User) -> AppResult<()>;
    async fn delete(&self, org: OrgId, id: UserId) -> AppResult<()>;
    async fn exists_by_email(&self, email: &Email) -> AppResult<bool>;
}
