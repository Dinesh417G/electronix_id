//! Organization read/update use cases.

use std::sync::Arc;

use chrono::Utc;

use crate::application::auth_service::require_role;
use crate::application::error::AppResult;
use crate::application::ports::organization_repo::OrganizationRepository;
use crate::domain::ids::OrgId;
use crate::domain::organization::Organization;
use crate::domain::value_objects::Role;

#[derive(Clone)]
pub struct OrganizationService {
    orgs: Arc<dyn OrganizationRepository>,
}

impl OrganizationService {
    pub fn new(orgs: Arc<dyn OrganizationRepository>) -> Self {
        Self { orgs }
    }

    pub async fn get(&self, org: OrgId) -> AppResult<Organization> {
        self.orgs.find_by_id(org).await
    }

    pub async fn update(
        &self,
        org: OrgId,
        actor_role: Role,
        name: Option<String>,
    ) -> AppResult<Organization> {
        require_role(actor_role, Role::Admin)?;
        let mut organization = self.orgs.find_by_id(org).await?;
        if let Some(n) = name {
            organization.name = n;
        }
        organization.updated_at = Utc::now();
        self.orgs.update(&organization).await?;
        Ok(organization)
    }
}
