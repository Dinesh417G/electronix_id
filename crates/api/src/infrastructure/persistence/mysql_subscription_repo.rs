//! MySQL adapter for [`SubscriptionRepository`] — one subscription per org.

use std::str::FromStr;

use async_trait::async_trait;
use sqlx::MySqlPool;

use crate::application::error::{AppResult, ApplicationError};
use crate::application::ports::subscription_repo::SubscriptionRepository;
use crate::domain::ids::OrgId;
use crate::domain::plan::{Subscription, SubscriptionStatus};
use crate::infrastructure::persistence::{map_sqlx, parse_uuid, to_utc};

pub struct MySqlSubscriptionRepo {
    pool: MySqlPool,
}

impl MySqlSubscriptionRepo {
    pub fn new(pool: MySqlPool) -> Self {
        Self { pool }
    }
}

struct SubscriptionRow {
    id: String,
    organization_id: String,
    status: String,
    trial_ends_at: Option<chrono::NaiveDateTime>,
    current_period_start: Option<chrono::NaiveDateTime>,
    current_period_end: Option<chrono::NaiveDateTime>,
    created_at: chrono::NaiveDateTime,
    updated_at: chrono::NaiveDateTime,
}

impl SubscriptionRow {
    fn into_domain(self) -> AppResult<Subscription> {
        Ok(Subscription {
            id: parse_uuid(&self.id)?,
            organization_id: parse_uuid(&self.organization_id)?,
            status: SubscriptionStatus::from_str(&self.status)
                .map_err(ApplicationError::internal)?,
            trial_ends_at: self.trial_ends_at.map(to_utc),
            current_period_start: self.current_period_start.map(to_utc),
            current_period_end: self.current_period_end.map(to_utc),
            created_at: to_utc(self.created_at),
            updated_at: to_utc(self.updated_at),
        })
    }
}

#[async_trait]
impl SubscriptionRepository for MySqlSubscriptionRepo {
    async fn create(&self, sub: &Subscription) -> AppResult<()> {
        sqlx::query!(
            r#"
            INSERT INTO subscriptions
              (id, organization_id, status, trial_ends_at, current_period_start,
               current_period_end, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#,
            sub.id.to_string(),
            sub.organization_id.to_string(),
            sub.status.as_str(),
            sub.trial_ends_at.map(|t| t.naive_utc()),
            sub.current_period_start.map(|t| t.naive_utc()),
            sub.current_period_end.map(|t| t.naive_utc()),
            sub.created_at.naive_utc(),
            sub.updated_at.naive_utc(),
        )
        .execute(&self.pool)
        .await
        .map_err(|e| map_sqlx(e, "subscription"))?;
        Ok(())
    }

    async fn find_by_org(&self, org: OrgId) -> AppResult<Option<Subscription>> {
        let row = sqlx::query_as!(
            SubscriptionRow,
            r#"
            SELECT id, organization_id, status, trial_ends_at, current_period_start,
                   current_period_end, created_at, updated_at
            FROM subscriptions WHERE organization_id = ?
            "#,
            org.to_string(),
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| map_sqlx(e, "subscription"))?;
        row.map(SubscriptionRow::into_domain).transpose()
    }

    async fn update(&self, sub: &Subscription) -> AppResult<()> {
        sqlx::query!(
            r#"
            UPDATE subscriptions SET
              status = ?, trial_ends_at = ?, current_period_start = ?,
              current_period_end = ?, updated_at = ?
            WHERE organization_id = ?
            "#,
            sub.status.as_str(),
            sub.trial_ends_at.map(|t| t.naive_utc()),
            sub.current_period_start.map(|t| t.naive_utc()),
            sub.current_period_end.map(|t| t.naive_utc()),
            sub.updated_at.naive_utc(),
            sub.organization_id.to_string(),
        )
        .execute(&self.pool)
        .await
        .map_err(|e| map_sqlx(e, "subscription"))?;
        Ok(())
    }
}
