use async_trait::async_trait;

use crate::application::error::AppResult;
use crate::domain::ids::OrgId;
use crate::domain::organization::Organization;

#[async_trait]
pub trait OrganizationRepository: Send + Sync {
    async fn create(&self, org: &Organization) -> AppResult<()>;
    async fn find_by_id(&self, id: OrgId) -> AppResult<Organization>;
    async fn find_by_slug(&self, slug: &str) -> AppResult<Option<Organization>>;
    async fn update(&self, org: &Organization) -> AppResult<()>;
}
