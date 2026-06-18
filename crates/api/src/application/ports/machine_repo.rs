use async_trait::async_trait;

use crate::application::error::AppResult;
use crate::domain::ids::{MachineId, OrgId};
use crate::domain::machine::Machine;

#[async_trait]
pub trait MachineRepository: Send + Sync {
    async fn create(&self, machine: &Machine) -> AppResult<()>;
    async fn find_by_id(&self, org: OrgId, id: MachineId) -> AppResult<Machine>;
    /// One page of an org's machines, plus the total count for the envelope.
    async fn list(&self, org: OrgId, limit: i64, offset: i64) -> AppResult<(Vec<Machine>, i64)>;
    /// All of an org's machines (used by pricing — no pagination).
    async fn list_all(&self, org: OrgId) -> AppResult<Vec<Machine>>;
    async fn update(&self, machine: &Machine) -> AppResult<()>;
    async fn delete(&self, org: OrgId, id: MachineId) -> AppResult<()>;
}
