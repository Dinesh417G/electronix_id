use async_trait::async_trait;

use crate::application::error::AppResult;
use crate::domain::ids::PlanId;
use crate::domain::plan::Plan;
use crate::domain::value_objects::Tier;

#[async_trait]
pub trait PlanRepository: Send + Sync {
    /// The catalog. `active_only` filters out roadmap placeholders (e.g. Predict).
    async fn list(&self, active_only: bool) -> AppResult<Vec<Plan>>;
    async fn find_by_id(&self, id: PlanId) -> AppResult<Option<Plan>>;
    async fn find_by_code(&self, code: Tier) -> AppResult<Option<Plan>>;
}
