//! Documents and their immutable version history.
//!
//! Mental model (PLC analogy): a `Document` is a program slot — "Main OB1".
//! Each `DocumentVersion` is a dated backup you never delete. You always know
//! what changed and you can reload an old one.
//!
//! JSON payloads (`content_json`, `metadata`) are held as raw JSON **text** so
//! the domain stays free of serde_json. Parsing/validation is the application
//! layer's job.

use chrono::{DateTime, Utc};

use crate::domain::error::DomainError;
use crate::domain::ids::{DocumentId, MachineId, UserId, VersionId};
use crate::domain::value_objects::{DocumentCategory, StorageKind};

/// A logical, versioned artifact slot on a machine.
#[derive(Debug, Clone)]
pub struct Document {
    pub id: DocumentId,
    pub machine_id: MachineId,
    pub category: DocumentCategory,
    /// Human label (required — especially when `category = Other`).
    pub name: String,
    pub storage_kind: StorageKind,
    /// Highest version number issued so far. 0 = slot created, no versions yet.
    pub current_version_no: i32,
    pub created_by: Option<UserId>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Document {
    /// The version number the next upload will receive. Monotonic: always
    /// `current + 1`, never reused.
    pub fn next_version_no(&self) -> i32 {
        self.current_version_no + 1
    }

    /// Names are required; reject empty/whitespace-only slot names.
    pub fn validate_name(name: &str) -> Result<(), DomainError> {
        if name.trim().is_empty() {
            Err(DomainError::EmptyDocumentName)
        } else {
            Ok(())
        }
    }
}

/// An immutable snapshot. INSERT only — the payload is never UPDATEd.
#[derive(Debug, Clone)]
pub struct DocumentVersion {
    pub id: VersionId,
    pub document_id: DocumentId,
    pub version_no: i32,
    pub is_current: bool,
    // file payload (when the document's storage_kind = File)
    pub storage_key: Option<String>,
    pub original_filename: Option<String>,
    pub mime_type: Option<String>,
    pub size_bytes: Option<i64>,
    pub checksum_sha256: Option<String>,
    // json payload (when storage_kind = Json): raw JSON text
    pub content_json: Option<String>,
    // common
    pub change_note: Option<String>,
    /// Typed extras as raw JSON text, e.g. {"o_number":..,"controller":..}.
    pub metadata: Option<String>,
    pub created_by: Option<UserId>,
    pub created_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::ids::MachineId;

    fn slot(current: i32) -> Document {
        Document {
            id: DocumentId::new(),
            machine_id: MachineId::new(),
            category: DocumentCategory::PlcProgram,
            name: "Main OB1".to_string(),
            storage_kind: StorageKind::File,
            current_version_no: current,
            created_by: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn next_version_no_is_monotonic() {
        assert_eq!(slot(0).next_version_no(), 1);
        assert_eq!(slot(2).next_version_no(), 3);
    }

    #[test]
    fn name_validation() {
        assert!(Document::validate_name("BOM").is_ok());
        assert_eq!(
            Document::validate_name("   "),
            Err(DomainError::EmptyDocumentName)
        );
    }
}
