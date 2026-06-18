//! MySQL adapter for [`DocumentRepository`], including the §5 version-bump
//! transaction in [`add_version`](MySqlDocumentRepo::add_version).
//!
//! Org scoping is done with an `IN (SELECT id FROM machines WHERE organization_id = ?)`
//! subquery rather than a JOIN, so the selected `document`/`document_version`
//! columns keep their base-table nullability for the compile-time checker. The
//! one exception is the `FOR UPDATE` lock, which must JOIN to serialize uploads.

use std::str::FromStr;

use async_trait::async_trait;
use chrono::Utc;
use sqlx::MySqlPool;

use crate::application::error::{AppResult, ApplicationError};
use crate::application::ports::document_repo::{DocumentRepository, NewVersionInput};
use crate::domain::document::{Document, DocumentVersion};
use crate::domain::ids::{DocumentId, MachineId, OrgId, VersionId};
use crate::domain::value_objects::{DocumentCategory, StorageKind};
use crate::infrastructure::persistence::{map_sqlx, parse_uuid, to_utc};

pub struct MySqlDocumentRepo {
    pool: MySqlPool,
}

impl MySqlDocumentRepo {
    pub fn new(pool: MySqlPool) -> Self {
        Self { pool }
    }
}

struct DocRow {
    id: String,
    machine_id: String,
    category: String,
    name: String,
    storage_kind: String,
    current_version_no: i32,
    created_by: Option<String>,
    created_at: chrono::NaiveDateTime,
    updated_at: chrono::NaiveDateTime,
}

impl DocRow {
    fn into_domain(self) -> AppResult<Document> {
        Ok(Document {
            id: parse_uuid(&self.id)?,
            machine_id: parse_uuid(&self.machine_id)?,
            category: DocumentCategory::parse(&self.category),
            name: self.name,
            storage_kind: StorageKind::from_str(&self.storage_kind)
                .map_err(ApplicationError::internal)?,
            current_version_no: self.current_version_no,
            created_by: self.created_by.as_deref().map(parse_uuid).transpose()?,
            created_at: to_utc(self.created_at),
            updated_at: to_utc(self.updated_at),
        })
    }
}

struct VersionRow {
    id: String,
    document_id: String,
    version_no: i32,
    is_current: bool,
    storage_key: Option<String>,
    original_filename: Option<String>,
    mime_type: Option<String>,
    size_bytes: Option<i64>,
    checksum_sha256: Option<String>,
    content_json: Option<String>,
    change_note: Option<String>,
    metadata: Option<String>,
    created_by: Option<String>,
    created_at: chrono::NaiveDateTime,
}

impl VersionRow {
    fn into_domain(self) -> AppResult<DocumentVersion> {
        Ok(DocumentVersion {
            id: parse_uuid(&self.id)?,
            document_id: parse_uuid(&self.document_id)?,
            version_no: self.version_no,
            is_current: self.is_current,
            storage_key: self.storage_key,
            original_filename: self.original_filename,
            mime_type: self.mime_type,
            size_bytes: self.size_bytes,
            checksum_sha256: self.checksum_sha256,
            content_json: self.content_json,
            change_note: self.change_note,
            metadata: self.metadata,
            created_by: self.created_by.as_deref().map(parse_uuid).transpose()?,
            created_at: to_utc(self.created_at),
        })
    }
}

#[async_trait]
impl DocumentRepository for MySqlDocumentRepo {
    async fn create(&self, doc: &Document) -> AppResult<()> {
        sqlx::query!(
            r#"
            INSERT INTO documents
              (id, machine_id, category, name, storage_kind, current_version_no,
               created_by, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
            doc.id.to_string(),
            doc.machine_id.to_string(),
            doc.category.as_str(),
            doc.name,
            doc.storage_kind.as_str(),
            doc.current_version_no,
            doc.created_by.map(|u| u.to_string()),
            doc.created_at.naive_utc(),
            doc.updated_at.naive_utc(),
        )
        .execute(&self.pool)
        .await
        .map_err(|e| map_sqlx(e, "document"))?;
        Ok(())
    }

    async fn find_by_id(&self, org: OrgId, id: DocumentId) -> AppResult<Document> {
        sqlx::query_as!(
            DocRow,
            r#"
            SELECT id, machine_id, category, name, storage_kind, current_version_no,
                   created_by, created_at, updated_at
            FROM documents
            WHERE id = ?
              AND machine_id IN (SELECT id FROM machines WHERE organization_id = ?)
            "#,
            id.to_string(),
            org.to_string(),
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| map_sqlx(e, "document"))?
        .into_domain()
    }

    async fn list_by_machine(
        &self,
        org: OrgId,
        machine: MachineId,
        category: Option<DocumentCategory>,
    ) -> AppResult<Vec<Document>> {
        let rows = match category {
            Some(cat) => {
                sqlx::query_as!(
                    DocRow,
                    r#"
                    SELECT id, machine_id, category, name, storage_kind, current_version_no,
                           created_by, created_at, updated_at
                    FROM documents
                    WHERE machine_id = ? AND category = ?
                      AND machine_id IN (SELECT id FROM machines WHERE organization_id = ?)
                    ORDER BY created_at ASC, id ASC
                    "#,
                    machine.to_string(),
                    cat.as_str(),
                    org.to_string(),
                )
                .fetch_all(&self.pool)
                .await
            }
            None => {
                sqlx::query_as!(
                    DocRow,
                    r#"
                    SELECT id, machine_id, category, name, storage_kind, current_version_no,
                           created_by, created_at, updated_at
                    FROM documents
                    WHERE machine_id = ?
                      AND machine_id IN (SELECT id FROM machines WHERE organization_id = ?)
                    ORDER BY created_at ASC, id ASC
                    "#,
                    machine.to_string(),
                    org.to_string(),
                )
                .fetch_all(&self.pool)
                .await
            }
        }
        .map_err(|e| map_sqlx(e, "document"))?;

        rows.into_iter().map(DocRow::into_domain).collect()
    }

    async fn update_meta(&self, org: OrgId, doc: &Document) -> AppResult<()> {
        let res = sqlx::query!(
            r#"
            UPDATE documents SET category = ?, name = ?, updated_at = ?
            WHERE id = ?
              AND machine_id IN (SELECT id FROM machines WHERE organization_id = ?)
            "#,
            doc.category.as_str(),
            doc.name,
            doc.updated_at.naive_utc(),
            doc.id.to_string(),
            org.to_string(),
        )
        .execute(&self.pool)
        .await
        .map_err(|e| map_sqlx(e, "document"))?;
        if res.rows_affected() == 0 {
            return Err(ApplicationError::not_found("document"));
        }
        Ok(())
    }

    async fn delete(&self, org: OrgId, id: DocumentId) -> AppResult<()> {
        let res = sqlx::query!(
            r#"
            DELETE FROM documents
            WHERE id = ?
              AND machine_id IN (SELECT id FROM machines WHERE organization_id = ?)
            "#,
            id.to_string(),
            org.to_string(),
        )
        .execute(&self.pool)
        .await
        .map_err(|e| map_sqlx(e, "document"))?;
        if res.rows_affected() == 0 {
            return Err(ApplicationError::not_found("document"));
        }
        Ok(())
    }

    async fn list_versions(&self, org: OrgId, doc: DocumentId) -> AppResult<Vec<DocumentVersion>> {
        let rows = sqlx::query_as!(
            VersionRow,
            r#"
            SELECT id, document_id, version_no, is_current AS `is_current: bool`,
                   storage_key, original_filename, mime_type, size_bytes, checksum_sha256,
                   CAST(content_json AS CHAR) AS `content_json: String`,
                   change_note, CAST(metadata AS CHAR) AS `metadata: String`,
                   created_by, created_at
            FROM document_versions
            WHERE document_id = ?
              AND document_id IN (
                SELECT id FROM documents
                WHERE machine_id IN (SELECT id FROM machines WHERE organization_id = ?)
              )
            ORDER BY version_no DESC
            "#,
            doc.to_string(),
            org.to_string(),
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| map_sqlx(e, "document version"))?;

        rows.into_iter().map(VersionRow::into_domain).collect()
    }

    async fn find_version(
        &self,
        org: OrgId,
        doc: DocumentId,
        version_no: i32,
    ) -> AppResult<DocumentVersion> {
        sqlx::query_as!(
            VersionRow,
            r#"
            SELECT id, document_id, version_no, is_current AS `is_current: bool`,
                   storage_key, original_filename, mime_type, size_bytes, checksum_sha256,
                   CAST(content_json AS CHAR) AS `content_json: String`,
                   change_note, CAST(metadata AS CHAR) AS `metadata: String`,
                   created_by, created_at
            FROM document_versions
            WHERE document_id = ? AND version_no = ?
              AND document_id IN (
                SELECT id FROM documents
                WHERE machine_id IN (SELECT id FROM machines WHERE organization_id = ?)
              )
            "#,
            doc.to_string(),
            version_no,
            org.to_string(),
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| map_sqlx(e, "document version"))?
        .into_domain()
    }

    async fn current_version(
        &self,
        org: OrgId,
        doc: DocumentId,
    ) -> AppResult<Option<DocumentVersion>> {
        let row = sqlx::query_as!(
            VersionRow,
            r#"
            SELECT id, document_id, version_no, is_current AS `is_current: bool`,
                   storage_key, original_filename, mime_type, size_bytes, checksum_sha256,
                   CAST(content_json AS CHAR) AS `content_json: String`,
                   change_note, CAST(metadata AS CHAR) AS `metadata: String`,
                   created_by, created_at
            FROM document_versions
            WHERE document_id = ? AND is_current = TRUE
              AND document_id IN (
                SELECT id FROM documents
                WHERE machine_id IN (SELECT id FROM machines WHERE organization_id = ?)
              )
            "#,
            doc.to_string(),
            org.to_string(),
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| map_sqlx(e, "document version"))?;

        row.map(VersionRow::into_domain).transpose()
    }

    async fn find_version_by_id(&self, id: VersionId) -> AppResult<Option<DocumentVersion>> {
        let row = sqlx::query_as!(
            VersionRow,
            r#"
            SELECT id, document_id, version_no, is_current AS `is_current: bool`,
                   storage_key, original_filename, mime_type, size_bytes, checksum_sha256,
                   CAST(content_json AS CHAR) AS `content_json: String`,
                   change_note, CAST(metadata AS CHAR) AS `metadata: String`,
                   created_by, created_at
            FROM document_versions
            WHERE id = ?
            "#,
            id.to_string(),
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| map_sqlx(e, "document version"))?;

        row.map(VersionRow::into_domain).transpose()
    }

    async fn add_version(
        &self,
        org: OrgId,
        doc_id: DocumentId,
        input: NewVersionInput,
    ) -> AppResult<DocumentVersion> {
        let now = Utc::now();
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| map_sqlx(e, "document"))?;

        // 1. Lock the slot (and its machine, for the org check) — serializes
        //    concurrent uploads to the same document.
        let locked = sqlx::query!(
            r#"
            SELECT d.current_version_no AS `current_version_no!: i32`,
                   d.machine_id AS `machine_id!`
            FROM documents d
            JOIN machines m ON m.id = d.machine_id
            WHERE d.id = ? AND m.organization_id = ?
            FOR UPDATE
            "#,
            doc_id.to_string(),
            org.to_string(),
        )
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| map_sqlx(e, "document"))?
        .ok_or_else(|| ApplicationError::not_found("document"))?;

        let next = locked.current_version_no + 1;

        // 2. Insert the new version as current.
        sqlx::query!(
            r#"
            INSERT INTO document_versions
              (id, document_id, version_no, is_current, storage_key, original_filename,
               mime_type, size_bytes, checksum_sha256, content_json, change_note, metadata,
               created_by, created_at)
            VALUES (?, ?, ?, TRUE, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
            input.id.to_string(),
            doc_id.to_string(),
            next,
            input.storage_key.clone(),
            input.original_filename.clone(),
            input.mime_type.clone(),
            input.size_bytes,
            input.checksum_sha256.clone(),
            input.content_json.clone(),
            input.change_note.clone(),
            input.metadata.clone(),
            input.created_by.map(|u| u.to_string()),
            now.naive_utc(),
        )
        .execute(&mut *tx)
        .await
        .map_err(|e| map_sqlx(e, "document version"))?;

        // 3. Demote every older version.
        sqlx::query!(
            r#"UPDATE document_versions SET is_current = FALSE WHERE document_id = ? AND version_no < ?"#,
            doc_id.to_string(),
            next,
        )
        .execute(&mut *tx)
        .await
        .map_err(|e| map_sqlx(e, "document version"))?;

        // 4. Bump the slot's counter.
        sqlx::query!(
            r#"UPDATE documents SET current_version_no = ?, updated_at = ? WHERE id = ?"#,
            next,
            now.naive_utc(),
            doc_id.to_string(),
        )
        .execute(&mut *tx)
        .await
        .map_err(|e| map_sqlx(e, "document"))?;

        // 5. Photo versions repoint the machine's primary-photo pointer.
        if let Some(machine_id) = input.primary_photo_for {
            sqlx::query!(
                r#"UPDATE machines SET primary_photo_version_id = ?, updated_at = ? WHERE id = ? AND organization_id = ?"#,
                input.id.to_string(),
                now.naive_utc(),
                machine_id.to_string(),
                org.to_string(),
            )
            .execute(&mut *tx)
            .await
            .map_err(|e| map_sqlx(e, "machine"))?;
        }

        tx.commit().await.map_err(|e| map_sqlx(e, "document"))?;

        Ok(DocumentVersion {
            id: input.id,
            document_id: doc_id,
            version_no: next,
            is_current: true,
            storage_key: input.storage_key,
            original_filename: input.original_filename,
            mime_type: input.mime_type,
            size_bytes: input.size_bytes,
            checksum_sha256: input.checksum_sha256,
            content_json: input.content_json,
            change_note: input.change_note,
            metadata: input.metadata,
            created_by: input.created_by,
            created_at: now,
        })
    }
}
