//! MySQL adapter for [`OrganizationRepository`].

use async_trait::async_trait;
use sqlx::MySqlPool;

use crate::application::error::AppResult;
use crate::application::ports::organization_repo::OrganizationRepository;
use crate::domain::ids::OrgId;
use crate::domain::organization::Organization;
use crate::infrastructure::persistence::{map_sqlx, parse_uuid, to_utc};

pub struct MySqlOrganizationRepo {
    pool: MySqlPool,
}

impl MySqlOrganizationRepo {
    pub fn new(pool: MySqlPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl OrganizationRepository for MySqlOrganizationRepo {
    async fn create(&self, org: &Organization) -> AppResult<()> {
        sqlx::query!(
            r#"
            INSERT INTO organizations (id, name, slug, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?)
            "#,
            org.id.to_string(),
            org.name,
            org.slug,
            org.created_at.naive_utc(),
            org.updated_at.naive_utc(),
        )
        .execute(&self.pool)
        .await
        .map_err(|e| map_sqlx(e, "organization"))?;
        Ok(())
    }

    async fn find_by_id(&self, id: OrgId) -> AppResult<Organization> {
        let row = sqlx::query!(
            r#"
            SELECT id, name, slug, created_at, updated_at
            FROM organizations WHERE id = ?
            "#,
            id.to_string(),
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| map_sqlx(e, "organization"))?;

        Ok(Organization {
            id: parse_uuid(&row.id)?,
            name: row.name,
            slug: row.slug,
            created_at: to_utc(row.created_at),
            updated_at: to_utc(row.updated_at),
        })
    }

    async fn find_by_slug(&self, slug: &str) -> AppResult<Option<Organization>> {
        let row = sqlx::query!(
            r#"
            SELECT id, name, slug, created_at, updated_at
            FROM organizations WHERE slug = ?
            "#,
            slug,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| map_sqlx(e, "organization"))?;

        row.map(|row| {
            Ok(Organization {
                id: parse_uuid(&row.id)?,
                name: row.name,
                slug: row.slug,
                created_at: to_utc(row.created_at),
                updated_at: to_utc(row.updated_at),
            })
        })
        .transpose()
    }

    async fn update(&self, org: &Organization) -> AppResult<()> {
        sqlx::query!(
            r#"UPDATE organizations SET name = ?, slug = ?, updated_at = ? WHERE id = ?"#,
            org.name,
            org.slug,
            org.updated_at.naive_utc(),
            org.id.to_string(),
        )
        .execute(&self.pool)
        .await
        .map_err(|e| map_sqlx(e, "organization"))?;
        Ok(())
    }
}
