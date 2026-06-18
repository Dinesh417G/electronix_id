//! Generic versioned-document use cases.
//!
//! This service owns the *flow* of versioning (validate, store bytes, compute
//! checksum, decide photo/primary, build the payload, request restore copies).
//! The atomic DB steps of the §5 transaction (lock the slot, assign the next
//! version number, demote older versions, bump counters, repoint primary photo)
//! live inside `DocumentRepository::add_version` so they commit as one unit.
//!
//! Storage-key note: files are keyed by the version's UUID, not its sequential
//! number — `{org}/{machine}/{document}/{version_id}/{filename}`. The number is
//! only assigned inside the DB transaction, so keying on it would force us to
//! write bytes while holding a row lock. Keying on the UUID is collision-free
//! and lock-free. (Deviation from §12's `{version_no}` path, reported in summary.)

use std::sync::Arc;

use sha2::{Digest, Sha256};

use crate::application::auth_service::require_role;
use crate::application::error::{AppResult, ApplicationError};
use crate::application::ports::document_repo::{DocumentRepository, NewVersionInput};
use crate::application::ports::file_storage::FileStorage;
use crate::application::ports::machine_repo::MachineRepository;
use crate::domain::document::{Document, DocumentVersion};
use crate::domain::ids::{DocumentId, MachineId, OrgId, UserId, VersionId};
use crate::domain::value_objects::{DocumentCategory, Role, StorageKind};

/// Bytes + descriptor for a file upload.
pub struct FileUpload {
    pub original_filename: String,
    pub mime_type: Option<String>,
    pub bytes: Vec<u8>,
}

#[derive(Clone)]
pub struct DocumentService {
    docs: Arc<dyn DocumentRepository>,
    machines: Arc<dyn MachineRepository>,
    storage: Arc<dyn FileStorage>,
}

impl DocumentService {
    pub fn new(
        docs: Arc<dyn DocumentRepository>,
        machines: Arc<dyn MachineRepository>,
        storage: Arc<dyn FileStorage>,
    ) -> Self {
        Self {
            docs,
            machines,
            storage,
        }
    }

    // -- slots -------------------------------------------------------------

    pub async fn list_for_machine(
        &self,
        org: OrgId,
        machine: MachineId,
        category: Option<DocumentCategory>,
    ) -> AppResult<Vec<Document>> {
        // Ensure the machine is in this org before listing its documents.
        self.machines.find_by_id(org, machine).await?;
        self.docs.list_by_machine(org, machine, category).await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn create_slot(
        &self,
        org: OrgId,
        actor_role: Role,
        actor_id: UserId,
        machine: MachineId,
        category: DocumentCategory,
        name: String,
        storage_kind: StorageKind,
    ) -> AppResult<Document> {
        require_role(actor_role, Role::Engineer)?;
        self.machines.find_by_id(org, machine).await?;
        Document::validate_name(&name)?;

        let now = chrono::Utc::now();
        let doc = Document {
            id: DocumentId::new(),
            machine_id: machine,
            category,
            name,
            storage_kind,
            current_version_no: 0,
            created_by: Some(actor_id),
            created_at: now,
            updated_at: now,
        };
        self.docs.create(&doc).await?;
        Ok(doc)
    }

    /// Slot + current version + full history (newest first).
    pub async fn get(
        &self,
        org: OrgId,
        doc_id: DocumentId,
    ) -> AppResult<(Document, Option<DocumentVersion>, Vec<DocumentVersion>)> {
        let doc = self.docs.find_by_id(org, doc_id).await?;
        let current = self.docs.current_version(org, doc_id).await?;
        let versions = self.docs.list_versions(org, doc_id).await?;
        Ok((doc, current, versions))
    }

    pub async fn rename(
        &self,
        org: OrgId,
        actor_role: Role,
        doc_id: DocumentId,
        name: Option<String>,
        category: Option<DocumentCategory>,
    ) -> AppResult<Document> {
        require_role(actor_role, Role::Engineer)?;
        let mut doc = self.docs.find_by_id(org, doc_id).await?;
        if let Some(n) = name {
            Document::validate_name(&n)?;
            doc.name = n;
        }
        if let Some(c) = category {
            doc.category = c;
        }
        doc.updated_at = chrono::Utc::now();
        self.docs.update_meta(org, &doc).await?;
        Ok(doc)
    }

    pub async fn delete(&self, org: OrgId, actor_role: Role, doc_id: DocumentId) -> AppResult<()> {
        require_role(actor_role, Role::Admin)?;
        self.docs.find_by_id(org, doc_id).await?;
        self.docs.delete(org, doc_id).await
    }

    // -- versions ----------------------------------------------------------

    pub async fn list_versions(
        &self,
        org: OrgId,
        doc_id: DocumentId,
    ) -> AppResult<Vec<DocumentVersion>> {
        self.docs.find_by_id(org, doc_id).await?;
        self.docs.list_versions(org, doc_id).await
    }

    pub async fn get_version(
        &self,
        org: OrgId,
        doc_id: DocumentId,
        version_no: i32,
    ) -> AppResult<DocumentVersion> {
        self.docs.find_version(org, doc_id, version_no).await
    }

    /// Add a new version from an uploaded file. Stores bytes, computes
    /// size + sha256, then commits the version-bump transaction.
    #[allow(clippy::too_many_arguments)]
    pub async fn add_file_version(
        &self,
        org: OrgId,
        actor_role: Role,
        actor_id: UserId,
        doc_id: DocumentId,
        upload: FileUpload,
        change_note: Option<String>,
        metadata: Option<String>,
    ) -> AppResult<DocumentVersion> {
        require_role(actor_role, Role::Engineer)?;
        let doc = self.docs.find_by_id(org, doc_id).await?;
        if doc.storage_kind != StorageKind::File {
            return Err(ApplicationError::Validation(
                "this document stores JSON; POST a JSON body, not a file".into(),
            ));
        }

        let checksum = sha256_hex(&upload.bytes);
        let size = upload.bytes.len() as i64;
        let version_id = VersionId::new();
        let filename = sanitize_filename(&upload.original_filename);
        let key = format!(
            "{}/{}/{}/{}/{}",
            org, doc.machine_id, doc.id, version_id, filename
        );

        self.storage.put(&key, &upload.bytes).await?;

        let input = NewVersionInput {
            id: version_id,
            storage_key: Some(key.clone()),
            original_filename: Some(upload.original_filename),
            mime_type: upload.mime_type,
            size_bytes: Some(size),
            checksum_sha256: Some(checksum),
            content_json: None,
            change_note,
            metadata,
            created_by: Some(actor_id),
            primary_photo_for: doc.category.is_photo().then_some(doc.machine_id),
        };

        match self.docs.add_version(org, doc_id, input).await {
            Ok(v) => Ok(v),
            Err(e) => {
                // Best-effort: don't leave an orphan blob if the DB step failed.
                let _ = self.storage.delete(&key).await;
                Err(e)
            }
        }
    }

    /// Add a new version from a JSON payload (specs, parameters, BOM rows).
    #[allow(clippy::too_many_arguments)]
    pub async fn add_json_version(
        &self,
        org: OrgId,
        actor_role: Role,
        actor_id: UserId,
        doc_id: DocumentId,
        content_json: String,
        is_object_or_array: bool,
        change_note: Option<String>,
        metadata: Option<String>,
    ) -> AppResult<DocumentVersion> {
        require_role(actor_role, Role::Engineer)?;
        let doc = self.docs.find_by_id(org, doc_id).await?;
        if doc.storage_kind != StorageKind::Json {
            return Err(ApplicationError::Validation(
                "this document stores files; upload multipart, not JSON".into(),
            ));
        }
        if !is_object_or_array {
            return Err(ApplicationError::Validation(
                "content_json must be a JSON object or array".into(),
            ));
        }

        let input = NewVersionInput {
            id: VersionId::new(),
            storage_key: None,
            original_filename: None,
            mime_type: None,
            size_bytes: None,
            checksum_sha256: None,
            content_json: Some(content_json),
            change_note,
            metadata,
            created_by: Some(actor_id),
            primary_photo_for: None,
        };
        self.docs.add_version(org, doc_id, input).await
    }

    /// Download a file version's bytes (along with its metadata for headers).
    pub async fn download(
        &self,
        org: OrgId,
        doc_id: DocumentId,
        version_no: i32,
    ) -> AppResult<(DocumentVersion, Vec<u8>)> {
        let doc = self.docs.find_by_id(org, doc_id).await?;
        if doc.storage_kind != StorageKind::File {
            return Err(ApplicationError::Validation(
                "this document stores JSON; read the version payload instead".into(),
            ));
        }
        let version = self.docs.find_version(org, doc_id, version_no).await?;
        let key = version
            .storage_key
            .clone()
            .ok_or_else(|| ApplicationError::internal("file version has no storage_key"))?;
        let bytes = self.storage.get(&key).await?;
        Ok((version, bytes))
    }

    /// Restore version `version_no` by copying its payload into a *new* highest
    /// version. Lineage preserved; nothing deleted.
    pub async fn restore(
        &self,
        org: OrgId,
        actor_role: Role,
        actor_id: UserId,
        doc_id: DocumentId,
        version_no: i32,
    ) -> AppResult<DocumentVersion> {
        require_role(actor_role, Role::Engineer)?;
        let doc = self.docs.find_by_id(org, doc_id).await?;
        let src = self.docs.find_version(org, doc_id, version_no).await?;
        let note = format!("restored from v{version_no}");
        let version_id = VersionId::new();

        let input = match doc.storage_kind {
            StorageKind::File => {
                let src_key = src
                    .storage_key
                    .clone()
                    .ok_or_else(|| ApplicationError::internal("file version has no storage_key"))?;
                let bytes = self.storage.get(&src_key).await?;
                let filename = src
                    .original_filename
                    .clone()
                    .unwrap_or_else(|| "file".to_string());
                let new_key = format!(
                    "{}/{}/{}/{}/{}",
                    org,
                    doc.machine_id,
                    doc.id,
                    version_id,
                    sanitize_filename(&filename)
                );
                self.storage.put(&new_key, &bytes).await?;
                NewVersionInput {
                    id: version_id,
                    storage_key: Some(new_key),
                    original_filename: src.original_filename.clone(),
                    mime_type: src.mime_type.clone(),
                    size_bytes: src.size_bytes,
                    checksum_sha256: src.checksum_sha256.clone(),
                    content_json: None,
                    change_note: Some(note),
                    metadata: src.metadata.clone(),
                    created_by: Some(actor_id),
                    primary_photo_for: doc.category.is_photo().then_some(doc.machine_id),
                }
            }
            StorageKind::Json => NewVersionInput {
                id: version_id,
                storage_key: None,
                original_filename: None,
                mime_type: None,
                size_bytes: None,
                checksum_sha256: None,
                content_json: src.content_json.clone(),
                change_note: Some(note),
                metadata: src.metadata.clone(),
                created_by: Some(actor_id),
                primary_photo_for: None,
            },
        };

        self.docs.add_version(org, doc_id, input).await
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

/// Keep only the final path component so a malicious filename can't escape the
/// version's storage prefix.
fn sanitize_filename(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or("file").trim();
    if base.is_empty() || base == "." || base == ".." {
        "file".to_string()
    } else {
        base.to_string()
    }
}
