//! MySQL adapter for [`MachineRepository`]. Every method is org-scoped.

use std::str::FromStr;

use async_trait::async_trait;
use sqlx::MySqlPool;

use crate::application::error::{AppResult, ApplicationError};
use crate::application::ports::machine_repo::MachineRepository;
use crate::domain::ids::{MachineId, OrgId};
use crate::domain::machine::Machine;
use crate::domain::value_objects::MachineStatus;
use crate::infrastructure::persistence::{map_sqlx, parse_uuid, to_utc};

pub struct MySqlMachineRepo {
    pool: MySqlPool,
}

impl MySqlMachineRepo {
    pub fn new(pool: MySqlPool) -> Self {
        Self { pool }
    }
}

struct MachineRow {
    id: String,
    organization_id: String,
    plan_id: Option<String>,
    name: String,
    make: Option<String>,
    model: Option<String>,
    serial_number: Option<String>,
    asset_tag: Option<String>,
    location: Option<String>,
    year_installed: Option<i16>,
    status: String,
    primary_photo_version_id: Option<String>,
    created_by: Option<String>,
    created_at: chrono::NaiveDateTime,
    updated_at: chrono::NaiveDateTime,
}

impl MachineRow {
    fn into_domain(self) -> AppResult<Machine> {
        Ok(Machine {
            id: parse_uuid(&self.id)?,
            organization_id: parse_uuid(&self.organization_id)?,
            plan_id: self.plan_id.as_deref().map(parse_uuid).transpose()?,
            name: self.name,
            make: self.make,
            model: self.model,
            serial_number: self.serial_number,
            asset_tag: self.asset_tag,
            location: self.location,
            year_installed: self.year_installed,
            status: MachineStatus::from_str(&self.status).map_err(ApplicationError::internal)?,
            primary_photo_version_id: self
                .primary_photo_version_id
                .as_deref()
                .map(parse_uuid)
                .transpose()?,
            created_by: self.created_by.as_deref().map(parse_uuid).transpose()?,
            created_at: to_utc(self.created_at),
            updated_at: to_utc(self.updated_at),
        })
    }
}

#[async_trait]
impl MachineRepository for MySqlMachineRepo {
    async fn create(&self, machine: &Machine) -> AppResult<()> {
        sqlx::query!(
            r#"
            INSERT INTO machines
              (id, organization_id, plan_id, name, make, model, serial_number, asset_tag,
               location, year_installed, status, primary_photo_version_id, created_by,
               created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
            machine.id.to_string(),
            machine.organization_id.to_string(),
            machine.plan_id.map(|p| p.to_string()),
            machine.name,
            machine.make,
            machine.model,
            machine.serial_number,
            machine.asset_tag,
            machine.location,
            machine.year_installed,
            machine.status.as_str(),
            machine.primary_photo_version_id.map(|v| v.to_string()),
            machine.created_by.map(|u| u.to_string()),
            machine.created_at.naive_utc(),
            machine.updated_at.naive_utc(),
        )
        .execute(&self.pool)
        .await
        .map_err(|e| map_sqlx(e, "machine"))?;
        Ok(())
    }

    async fn find_by_id(&self, org: OrgId, id: MachineId) -> AppResult<Machine> {
        sqlx::query_as!(
            MachineRow,
            r#"
            SELECT id, organization_id, plan_id, name, make, model, serial_number, asset_tag,
                   location, year_installed, status, primary_photo_version_id, created_by,
                   created_at, updated_at
            FROM machines WHERE id = ? AND organization_id = ?
            "#,
            id.to_string(),
            org.to_string(),
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| map_sqlx(e, "machine"))?
        .into_domain()
    }

    async fn list(&self, org: OrgId, limit: i64, offset: i64) -> AppResult<(Vec<Machine>, i64)> {
        let rows = sqlx::query_as!(
            MachineRow,
            r#"
            SELECT id, organization_id, plan_id, name, make, model, serial_number, asset_tag,
                   location, year_installed, status, primary_photo_version_id, created_by,
                   created_at, updated_at
            FROM machines WHERE organization_id = ?
            ORDER BY created_at ASC, id ASC
            LIMIT ? OFFSET ?
            "#,
            org.to_string(),
            limit,
            offset,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| map_sqlx(e, "machine"))?;

        let total = sqlx::query!(
            r#"SELECT COUNT(*) AS `total!: i64` FROM machines WHERE organization_id = ?"#,
            org.to_string(),
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| map_sqlx(e, "machine"))?
        .total;

        let machines = rows
            .into_iter()
            .map(MachineRow::into_domain)
            .collect::<AppResult<Vec<_>>>()?;
        Ok((machines, total))
    }

    async fn list_all(&self, org: OrgId) -> AppResult<Vec<Machine>> {
        let rows = sqlx::query_as!(
            MachineRow,
            r#"
            SELECT id, organization_id, plan_id, name, make, model, serial_number, asset_tag,
                   location, year_installed, status, primary_photo_version_id, created_by,
                   created_at, updated_at
            FROM machines WHERE organization_id = ?
            ORDER BY created_at ASC, id ASC
            "#,
            org.to_string(),
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| map_sqlx(e, "machine"))?;
        rows.into_iter().map(MachineRow::into_domain).collect()
    }

    async fn update(&self, machine: &Machine) -> AppResult<()> {
        sqlx::query!(
            r#"
            UPDATE machines SET
              plan_id = ?, name = ?, make = ?, model = ?, serial_number = ?, asset_tag = ?,
              location = ?, year_installed = ?, status = ?, primary_photo_version_id = ?,
              updated_at = ?
            WHERE id = ? AND organization_id = ?
            "#,
            machine.plan_id.map(|p| p.to_string()),
            machine.name,
            machine.make,
            machine.model,
            machine.serial_number,
            machine.asset_tag,
            machine.location,
            machine.year_installed,
            machine.status.as_str(),
            machine.primary_photo_version_id.map(|v| v.to_string()),
            machine.updated_at.naive_utc(),
            machine.id.to_string(),
            machine.organization_id.to_string(),
        )
        .execute(&self.pool)
        .await
        .map_err(|e| map_sqlx(e, "machine"))?;
        Ok(())
    }

    async fn delete(&self, org: OrgId, id: MachineId) -> AppResult<()> {
        let res = sqlx::query!(
            r#"DELETE FROM machines WHERE id = ? AND organization_id = ?"#,
            id.to_string(),
            org.to_string(),
        )
        .execute(&self.pool)
        .await
        .map_err(|e| map_sqlx(e, "machine"))?;
        if res.rows_affected() == 0 {
            return Err(ApplicationError::not_found("machine"));
        }
        Ok(())
    }
}
