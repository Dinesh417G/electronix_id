use async_trait::async_trait;

use crate::application::error::AppResult;
use crate::domain::document::{Document, DocumentVersion};
use crate::domain::ids::{DocumentId, MachineId, OrgId, UserId, VersionId};
use crate::domain::value_objects::DocumentCategory;

/// Payload for a new version. `version_no`/`is_current`/`created_at` are decided
/// by the repository *inside the locked transaction* (so concurrent uploads
/// serialize correctly) — the caller does not set them.
#[derive(Debug, Clone)]
pub struct NewVersionInput {
    pub id: VersionId,
    // file payload
    pub storage_key: Option<String>,
    pub original_filename: Option<String>,
    pub mime_type: Option<String>,
    pub size_bytes: Option<i64>,
    pub checksum_sha256: Option<String>,
    // json payload
    pub content_json: Option<String>,
    // common
    pub change_note: Option<String>,
    pub metadata: Option<String>,
    pub created_by: Option<UserId>,
    /// When set, this version is a photo: point the machine's
    /// `primary_photo_version_id` at it, inside the same transaction.
    pub primary_photo_for: Option<MachineId>,
}

#[async_trait]
pub trait DocumentRepository: Send + Sync {
    async fn create(&self, doc: &Document) -> AppResult<()>;
    /// Org-scoped: resolves only if the document's machine belongs to `org`.
    async fn find_by_id(&self, org: OrgId, id: DocumentId) -> AppResult<Document>;
    async fn list_by_machine(
        &self,
        org: OrgId,
        machine: MachineId,
        category: Option<DocumentCategory>,
    ) -> AppResult<Vec<Document>>;
    /// Rename / recategorize the slot.
    async fn update_meta(&self, org: OrgId, doc: &Document) -> AppResult<()>;
    async fn delete(&self, org: OrgId, id: DocumentId) -> AppResult<()>;

    // --- versions ---
    /// History, newest first.
    async fn list_versions(&self, org: OrgId, doc: DocumentId) -> AppResult<Vec<DocumentVersion>>;
    async fn find_version(
        &self,
        org: OrgId,
        doc: DocumentId,
        version_no: i32,
    ) -> AppResult<DocumentVersion>;
    async fn current_version(
        &self,
        org: OrgId,
        doc: DocumentId,
    ) -> AppResult<Option<DocumentVersion>>;

    /// The §5 version-bump transaction, atomic:
    /// 1. `SELECT ... FOR UPDATE` the documents row (serialize uploads),
    /// 2. `next = current_version_no + 1`,
    /// 3. INSERT the version with `version_no = next`, `is_current = TRUE`,
    /// 4. demote older versions (`is_current = FALSE`),
    /// 5. bump `documents.current_version_no`,
    /// 6. if `primary_photo_for` is set, update that machine's primary photo,
    /// 7. COMMIT.
    /// Returns the inserted version (with its assigned `version_no`).
    async fn add_version(
        &self,
        org: OrgId,
        doc_id: DocumentId,
        input: NewVersionInput,
    ) -> AppResult<DocumentVersion>;
}
