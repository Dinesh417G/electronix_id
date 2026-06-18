//! Machine CRUD + tier assignment. Engineer may create/update; admin deletes
//! and sets tiers. Everything is org-scoped.

use std::sync::Arc;

use chrono::Utc;

use crate::application::auth_service::require_role;
use crate::application::error::{AppResult, ApplicationError};
use crate::application::ports::machine_repo::MachineRepository;
use crate::application::ports::plan_repo::PlanRepository;
use crate::domain::ids::{MachineId, OrgId, UserId};
use crate::domain::machine::Machine;
use crate::domain::value_objects::{MachineStatus, Role, Tier};

#[derive(Debug, Default)]
pub struct NewMachine {
    pub name: String,
    pub make: Option<String>,
    pub model: Option<String>,
    pub serial_number: Option<String>,
    pub asset_tag: Option<String>,
    pub location: Option<String>,
    pub year_installed: Option<i16>,
    pub status: Option<MachineStatus>,
}

#[derive(Debug, Default)]
pub struct MachinePatch {
    pub name: Option<String>,
    pub make: Option<Option<String>>,
    pub model: Option<Option<String>>,
    pub serial_number: Option<Option<String>>,
    pub asset_tag: Option<Option<String>>,
    pub location: Option<Option<String>>,
    pub year_installed: Option<Option<i16>>,
    pub status: Option<MachineStatus>,
}

#[derive(Clone)]
pub struct MachineService {
    machines: Arc<dyn MachineRepository>,
    plans: Arc<dyn PlanRepository>,
}

impl MachineService {
    pub fn new(machines: Arc<dyn MachineRepository>, plans: Arc<dyn PlanRepository>) -> Self {
        Self { machines, plans }
    }

    pub async fn list(
        &self,
        org: OrgId,
        limit: i64,
        offset: i64,
    ) -> AppResult<(Vec<Machine>, i64)> {
        self.machines.list(org, limit, offset).await
    }

    pub async fn get(&self, org: OrgId, id: MachineId) -> AppResult<Machine> {
        self.machines.find_by_id(org, id).await
    }

    pub async fn create(
        &self,
        org: OrgId,
        actor_role: Role,
        actor_id: UserId,
        input: NewMachine,
    ) -> AppResult<Machine> {
        require_role(actor_role, Role::Engineer)?;
        let now = Utc::now();
        let machine = Machine {
            id: MachineId::new(),
            organization_id: org,
            plan_id: None,
            name: input.name,
            make: input.make,
            model: input.model,
            serial_number: input.serial_number,
            asset_tag: input.asset_tag,
            location: input.location,
            year_installed: input.year_installed,
            status: input.status.unwrap_or_default(),
            primary_photo_version_id: None,
            created_by: Some(actor_id),
            created_at: now,
            updated_at: now,
        };
        self.machines.create(&machine).await?;
        Ok(machine)
    }

    pub async fn update(
        &self,
        org: OrgId,
        actor_role: Role,
        id: MachineId,
        patch: MachinePatch,
    ) -> AppResult<Machine> {
        require_role(actor_role, Role::Engineer)?;
        let mut m = self.machines.find_by_id(org, id).await?;
        if let Some(v) = patch.name {
            m.name = v;
        }
        if let Some(v) = patch.make {
            m.make = v;
        }
        if let Some(v) = patch.model {
            m.model = v;
        }
        if let Some(v) = patch.serial_number {
            m.serial_number = v;
        }
        if let Some(v) = patch.asset_tag {
            m.asset_tag = v;
        }
        if let Some(v) = patch.location {
            m.location = v;
        }
        if let Some(v) = patch.year_installed {
            m.year_installed = v;
        }
        if let Some(v) = patch.status {
            m.status = v;
        }
        m.updated_at = Utc::now();
        self.machines.update(&m).await?;
        Ok(m)
    }

    pub async fn delete(&self, org: OrgId, actor_role: Role, id: MachineId) -> AppResult<()> {
        require_role(actor_role, Role::Admin)?;
        self.machines.find_by_id(org, id).await?;
        self.machines.delete(org, id).await
    }

    /// Assign a pricing tier (plan) to a machine. Admin/owner only.
    pub async fn set_tier(
        &self,
        org: OrgId,
        actor_role: Role,
        id: MachineId,
        plan_code: Tier,
    ) -> AppResult<Machine> {
        require_role(actor_role, Role::Admin)?;
        let plan = self
            .plans
            .find_by_code(plan_code)
            .await?
            .ok_or_else(|| ApplicationError::not_found("plan"))?;
        let mut m = self.machines.find_by_id(org, id).await?;
        m.plan_id = Some(plan.id);
        m.updated_at = Utc::now();
        self.machines.update(&m).await?;
        Ok(m)
    }
}
