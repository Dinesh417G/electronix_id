//! Resolver response payloads. `snake_case` (serde default), matching the api.
//!
//! Two shells: [`PassportSummary`] is what an anonymous QR scan returns (machine
//! identity + a photo link). [`PassportFull`] adds the document inventory and is
//! only built for an authenticated org member.

use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value;

use electronix_id_api::domain::document::{Document, DocumentVersion};
use electronix_id_api::domain::machine::Machine;
use electronix_id_api::domain::organization::Organization;

/// Parse stored JSON text into a value so it embeds as JSON (not a quoted
/// string). Unparseable text is dropped rather than leaked raw.
fn json_text(raw: Option<String>) -> Option<Value> {
    raw.and_then(|t| serde_json::from_str(&t).ok())
}

/// Public scan result: just enough to recognise the machine, plus a link to its
/// primary photo. No documents, no internal ids.
#[derive(Debug, Serialize)]
pub struct PassportSummary {
    pub public_code: String,
    pub machine_name: String,
    pub make: Option<String>,
    pub model: Option<String>,
    pub status: String,
    pub organization_name: String,
    pub has_photo: bool,
    /// Relative URL to the public photo stream, when a primary photo exists.
    pub photo_url: Option<String>,
}

impl PassportSummary {
    pub fn build(m: &Machine, org: &Organization) -> Self {
        let code = m.public_code.clone().unwrap_or_default();
        let has_photo = m.primary_photo_version_id.is_some();
        Self {
            public_code: code.clone(),
            machine_name: m.name.clone(),
            make: m.make.clone(),
            model: m.model.clone(),
            status: m.status.as_str().to_string(),
            organization_name: org.name.clone(),
            has_photo,
            photo_url: has_photo.then(|| format!("/r/{code}/photo")),
        }
    }
}

/// Authenticated scan result: the summary plus the machine's document inventory
/// (each slot with its current version's metadata).
#[derive(Debug, Serialize)]
pub struct PassportFull {
    #[serde(flatten)]
    pub summary: PassportSummary,
    pub documents: Vec<PassportDocument>,
}

#[derive(Debug, Serialize)]
pub struct PassportDocument {
    pub id: String,
    pub category: String,
    pub name: String,
    pub storage_kind: String,
    pub current_version_no: i32,
    pub updated_at: DateTime<Utc>,
    pub current_version: Option<PassportVersion>,
}

impl PassportDocument {
    pub fn build(doc: Document, current: Option<DocumentVersion>) -> Self {
        Self {
            id: doc.id.to_string(),
            category: doc.category.as_str().to_string(),
            name: doc.name,
            storage_kind: doc.storage_kind.as_str().to_string(),
            current_version_no: doc.current_version_no,
            updated_at: doc.updated_at,
            current_version: current.map(PassportVersion::from),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct PassportVersion {
    pub version_no: i32,
    pub original_filename: Option<String>,
    pub mime_type: Option<String>,
    pub size_bytes: Option<i64>,
    pub change_note: Option<String>,
    pub content_json: Option<Value>,
    pub created_at: DateTime<Utc>,
}

impl From<DocumentVersion> for PassportVersion {
    fn from(v: DocumentVersion) -> Self {
        Self {
            version_no: v.version_no,
            original_filename: v.original_filename,
            mime_type: v.mime_type,
            size_bytes: v.size_bytes,
            change_note: v.change_note,
            content_json: json_text(v.content_json),
            created_at: v.created_at,
        }
    }
}
