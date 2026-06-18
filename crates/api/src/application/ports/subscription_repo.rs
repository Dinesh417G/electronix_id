//! NOTE: Not in the §4 port list, but subscriptions need persistence and
//! folding them into `plan_repo` would muddle two concerns. Dedicated port =
//! cleaner layering. (Deviation reported in the milestone summary.)

use async_trait::async_trait;

use crate::application::error::AppResult;
use crate::domain::ids::OrgId;
use crate::domain::plan::Subscription;

#[async_trait]
pub trait SubscriptionRepository: Send + Sync {
    async fn create(&self, sub: &Subscription) -> AppResult<()>;
    async fn find_by_org(&self, org: OrgId) -> AppResult<Option<Subscription>>;
    async fn update(&self, sub: &Subscription) -> AppResult<()>;
}
