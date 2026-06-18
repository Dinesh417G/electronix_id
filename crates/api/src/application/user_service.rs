//! User management use cases. Every method is org-scoped; writes require admin.

use std::sync::Arc;

use chrono::Utc;

use crate::application::auth_service::require_role;
use crate::application::error::{AppResult, ApplicationError};
use crate::application::ports::password_hasher::PasswordHasher;
use crate::application::ports::user_repo::UserRepository;
use crate::domain::ids::{OrgId, UserId};
use crate::domain::user::User;
use crate::domain::value_objects::{Email, Role};

/// Fields an admin may change on a user. `None` = leave as-is.
#[derive(Debug, Default)]
pub struct UserPatch {
    pub full_name: Option<String>,
    pub role: Option<Role>,
    pub is_active: Option<bool>,
    pub password: Option<String>,
}

#[derive(Clone)]
pub struct UserService {
    users: Arc<dyn UserRepository>,
    hasher: Arc<dyn PasswordHasher>,
}

impl UserService {
    pub fn new(users: Arc<dyn UserRepository>, hasher: Arc<dyn PasswordHasher>) -> Self {
        Self { users, hasher }
    }

    pub async fn list(&self, org: OrgId, limit: i64, offset: i64) -> AppResult<(Vec<User>, i64)> {
        self.users.list(org, limit, offset).await
    }

    pub async fn get(&self, org: OrgId, id: UserId) -> AppResult<User> {
        self.users.find_by_id(org, id).await
    }

    pub async fn create(
        &self,
        org: OrgId,
        actor_role: Role,
        email: String,
        password: String,
        full_name: String,
        role: Role,
    ) -> AppResult<User> {
        require_role(actor_role, Role::Admin)?;
        let email = Email::parse(&email)?;
        if self.users.exists_by_email(&email).await? {
            return Err(ApplicationError::Conflict(
                "email already registered".into(),
            ));
        }
        let now = Utc::now();
        let user = User {
            id: UserId::new(),
            organization_id: org,
            email,
            password_hash: self.hasher.hash(&password)?,
            full_name,
            role,
            is_active: true,
            created_at: now,
            updated_at: now,
        };
        self.users.create(&user).await?;
        Ok(user)
    }

    pub async fn update(
        &self,
        org: OrgId,
        actor_role: Role,
        id: UserId,
        patch: UserPatch,
    ) -> AppResult<User> {
        require_role(actor_role, Role::Admin)?;
        let mut user = self.users.find_by_id(org, id).await?;
        if let Some(n) = patch.full_name {
            user.full_name = n;
        }
        if let Some(r) = patch.role {
            user.role = r;
        }
        if let Some(a) = patch.is_active {
            user.is_active = a;
        }
        if let Some(p) = patch.password {
            user.password_hash = self.hasher.hash(&p)?;
        }
        user.updated_at = Utc::now();
        self.users.update(&user).await?;
        Ok(user)
    }

    pub async fn delete(&self, org: OrgId, actor_role: Role, id: UserId) -> AppResult<()> {
        require_role(actor_role, Role::Admin)?;
        // Resolve first so a cross-org / missing id is a clean NotFound.
        self.users.find_by_id(org, id).await?;
        self.users.delete(org, id).await
    }
}
