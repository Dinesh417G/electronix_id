use chrono::{DateTime, Utc};

use crate::domain::ids::{OrgId, UserId};
use crate::domain::value_objects::{Email, Role};

#[derive(Debug, Clone)]
pub struct User {
    pub id: UserId,
    pub organization_id: OrgId,
    pub email: Email,
    /// Argon2id PHC string. Never logged, never serialized to clients.
    pub password_hash: String,
    pub full_name: String,
    pub role: Role,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl User {
    /// Can this user act with at least the given role, and is the account live?
    pub fn can(&self, min: Role) -> bool {
        self.is_active && self.role.at_least(min)
    }
}
