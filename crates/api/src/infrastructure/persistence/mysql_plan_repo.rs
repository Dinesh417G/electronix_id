//! MySQL adapter for [`PlanRepository`] — the read-only pricing catalog.

use std::str::FromStr;

use async_trait::async_trait;
use sqlx::MySqlPool;

use crate::application::error::{AppResult, ApplicationError};
use crate::application::ports::plan_repo::PlanRepository;
use crate::domain::ids::PlanId;
use crate::domain::plan::Plan;
use crate::domain::value_objects::{Currency, Money, Tier};
use crate::infrastructure::persistence::{map_sqlx, parse_uuid, to_utc};

pub struct MySqlPlanRepo {
    pool: MySqlPool,
}

impl MySqlPlanRepo {
    pub fn new(pool: MySqlPool) -> Self {
        Self { pool }
    }
}

struct PlanRow {
    id: String,
    code: String,
    name: String,
    price_per_machine_year: i64,
    onboarding_fee: i64,
    currency: String,
    features: Option<String>,
    is_active: bool,
    created_at: chrono::NaiveDateTime,
}

impl PlanRow {
    fn into_domain(self) -> AppResult<Plan> {
        let currency = Currency::parse(&self.currency).map_err(ApplicationError::internal)?;
        Ok(Plan {
            id: parse_uuid(&self.id)?,
            code: Tier::from_str(&self.code).map_err(ApplicationError::internal)?,
            name: self.name,
            price_per_machine_year: Money::new(self.price_per_machine_year, currency.clone()),
            onboarding_fee: Money::new(self.onboarding_fee, currency),
            features: self.features,
            is_active: self.is_active,
            created_at: to_utc(self.created_at),
        })
    }
}

#[async_trait]
impl PlanRepository for MySqlPlanRepo {
    async fn list(&self, active_only: bool) -> AppResult<Vec<Plan>> {
        // `active_only` is a plain bool, not user input — branch the query so the
        // compile-time checker sees two fully-formed statements.
        let rows = if active_only {
            sqlx::query_as!(
                PlanRow,
                r#"
                SELECT id, code, name, price_per_machine_year, onboarding_fee, currency,
                       CAST(features AS CHAR) AS `features: String`,
                       is_active AS `is_active: bool`, created_at
                FROM plans WHERE is_active = TRUE
                ORDER BY price_per_machine_year ASC
                "#,
            )
            .fetch_all(&self.pool)
            .await
        } else {
            sqlx::query_as!(
                PlanRow,
                r#"
                SELECT id, code, name, price_per_machine_year, onboarding_fee, currency,
                       CAST(features AS CHAR) AS `features: String`,
                       is_active AS `is_active: bool`, created_at
                FROM plans
                ORDER BY price_per_machine_year ASC
                "#,
            )
            .fetch_all(&self.pool)
            .await
        }
        .map_err(|e| map_sqlx(e, "plan"))?;

        rows.into_iter().map(PlanRow::into_domain).collect()
    }

    async fn find_by_id(&self, id: PlanId) -> AppResult<Option<Plan>> {
        let row = sqlx::query_as!(
            PlanRow,
            r#"
            SELECT id, code, name, price_per_machine_year, onboarding_fee, currency,
                   CAST(features AS CHAR) AS `features: String`,
                   is_active AS `is_active: bool`, created_at
            FROM plans WHERE id = ?
            "#,
            id.to_string(),
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| map_sqlx(e, "plan"))?;
        row.map(PlanRow::into_domain).transpose()
    }

    async fn find_by_code(&self, code: Tier) -> AppResult<Option<Plan>> {
        let row = sqlx::query_as!(
            PlanRow,
            r#"
            SELECT id, code, name, price_per_machine_year, onboarding_fee, currency,
                   CAST(features AS CHAR) AS `features: String`,
                   is_active AS `is_active: bool`, created_at
            FROM plans WHERE code = ?
            "#,
            code.as_str(),
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| map_sqlx(e, "plan"))?;
        row.map(PlanRow::into_domain).transpose()
    }
}
