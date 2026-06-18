//! Request and response payloads. Requests derive `validator::Validate`;
//! responses map from domain types. JSON is `snake_case` (serde default).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use validator::Validate;

use crate::application::auth_service::AuthTokens;
use crate::application::pricing_service::{Estimate, MachineLine};
use crate::domain::document::{Document, DocumentVersion};
use crate::domain::machine::Machine;
use crate::domain::organization::Organization;
use crate::domain::plan::{Plan, Subscription};
use crate::domain::user::User;
use crate::domain::value_objects::Money;

/// Parse stored JSON text into a value so it embeds as JSON (not a quoted
/// string) in responses. Unparseable text is dropped rather than leaked raw.
fn json_text(raw: Option<String>) -> Option<Value> {
    raw.and_then(|t| serde_json::from_str(&t).ok())
}

// ---------------------------------------------------------------------------
// Auth
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, Validate)]
pub struct RegisterRequest {
    #[validate(length(min = 1, max = 160))]
    pub organization_name: String,
    #[validate(email)]
    pub email: String,
    #[validate(length(min = 8, max = 200))]
    pub password: String,
    #[validate(length(min = 1, max = 160))]
    pub full_name: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct LoginRequest {
    #[validate(email)]
    pub email: String,
    #[validate(length(min = 1))]
    pub password: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct RefreshRequest {
    #[validate(length(min = 1))]
    pub refresh_token: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct LogoutRequest {
    #[validate(length(min = 1))]
    pub refresh_token: String,
}

#[derive(Debug, Serialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: i64,
}

impl From<AuthTokens> for TokenResponse {
    fn from(t: AuthTokens) -> Self {
        Self {
            access_token: t.access_token,
            refresh_token: t.refresh_token,
            expires_in: t.expires_in,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct MeResponse {
    pub user: UserResponse,
    pub organization: OrganizationResponse,
}

// ---------------------------------------------------------------------------
// Users
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, Validate)]
pub struct CreateUserRequest {
    #[validate(email)]
    pub email: String,
    #[validate(length(min = 8, max = 200))]
    pub password: String,
    #[validate(length(min = 1, max = 160))]
    pub full_name: String,
    /// `owner | admin | engineer | viewer`
    #[validate(length(min = 1))]
    pub role: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateUserRequest {
    #[validate(length(min = 1, max = 160))]
    pub full_name: Option<String>,
    pub role: Option<String>,
    pub is_active: Option<bool>,
    #[validate(length(min = 8, max = 200))]
    pub password: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct UserResponse {
    pub id: String,
    pub organization_id: String,
    pub email: String,
    pub full_name: String,
    pub role: String,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<User> for UserResponse {
    fn from(u: User) -> Self {
        Self {
            id: u.id.to_string(),
            organization_id: u.organization_id.to_string(),
            email: u.email.to_string(),
            full_name: u.full_name,
            role: u.role.as_str().to_string(),
            is_active: u.is_active,
            created_at: u.created_at,
            updated_at: u.updated_at,
        }
    }
}

// ---------------------------------------------------------------------------
// Organization
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateOrganizationRequest {
    #[validate(length(min = 1, max = 160))]
    pub name: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct OrganizationResponse {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<Organization> for OrganizationResponse {
    fn from(o: Organization) -> Self {
        Self {
            id: o.id.to_string(),
            name: o.name,
            slug: o.slug,
            created_at: o.created_at,
            updated_at: o.updated_at,
        }
    }
}

// ---------------------------------------------------------------------------
// Machines
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, Validate)]
pub struct CreateMachineRequest {
    #[validate(length(min = 1, max = 160))]
    pub name: String,
    #[validate(length(max = 120))]
    pub make: Option<String>,
    #[validate(length(max = 120))]
    pub model: Option<String>,
    #[validate(length(max = 120))]
    pub serial_number: Option<String>,
    #[validate(length(max = 64))]
    pub asset_tag: Option<String>,
    #[validate(length(max = 160))]
    pub location: Option<String>,
    pub year_installed: Option<i16>,
    /// `active | maintenance | retired`
    pub status: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateMachineRequest {
    #[validate(length(min = 1, max = 160))]
    pub name: Option<String>,
    pub make: Option<String>,
    pub model: Option<String>,
    pub serial_number: Option<String>,
    pub asset_tag: Option<String>,
    pub location: Option<String>,
    pub year_installed: Option<i16>,
    pub status: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct TierRequest {
    #[validate(length(min = 1))]
    pub plan_code: String,
}

#[derive(Debug, Serialize)]
pub struct MachineResponse {
    pub id: String,
    pub organization_id: String,
    pub plan_id: Option<String>,
    pub name: String,
    pub make: Option<String>,
    pub model: Option<String>,
    pub serial_number: Option<String>,
    pub asset_tag: Option<String>,
    pub location: Option<String>,
    pub year_installed: Option<i16>,
    pub status: String,
    pub primary_photo_version_id: Option<String>,
    pub created_by: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<Machine> for MachineResponse {
    fn from(m: Machine) -> Self {
        Self {
            id: m.id.to_string(),
            organization_id: m.organization_id.to_string(),
            plan_id: m.plan_id.map(|p| p.to_string()),
            name: m.name,
            make: m.make,
            model: m.model,
            serial_number: m.serial_number,
            asset_tag: m.asset_tag,
            location: m.location,
            year_installed: m.year_installed,
            status: m.status.as_str().to_string(),
            primary_photo_version_id: m.primary_photo_version_id.map(|v| v.to_string()),
            created_by: m.created_by.map(|u| u.to_string()),
            created_at: m.created_at,
            updated_at: m.updated_at,
        }
    }
}

// ---------------------------------------------------------------------------
// Documents & versions
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, Validate)]
pub struct CreateDocumentRequest {
    /// A `DocumentCategory` string; unknown values map to `other`.
    #[validate(length(min = 1, max = 40))]
    pub category: String,
    #[validate(length(min = 1, max = 200))]
    pub name: String,
    /// `file | json`
    #[validate(length(min = 1))]
    pub storage_kind: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateDocumentRequest {
    #[validate(length(min = 1, max = 200))]
    pub name: Option<String>,
    pub category: Option<String>,
}

/// JSON-kind version body: the payload plus optional change note / metadata.
#[derive(Debug, Deserialize)]
pub struct JsonVersionRequest {
    pub content: Value,
    pub change_note: Option<String>,
    pub metadata: Option<Value>,
}

#[derive(Debug, Serialize)]
pub struct DocumentResponse {
    pub id: String,
    pub machine_id: String,
    pub category: String,
    pub name: String,
    pub storage_kind: String,
    pub current_version_no: i32,
    pub created_by: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<Document> for DocumentResponse {
    fn from(d: Document) -> Self {
        Self {
            id: d.id.to_string(),
            machine_id: d.machine_id.to_string(),
            category: d.category.as_str().to_string(),
            name: d.name,
            storage_kind: d.storage_kind.as_str().to_string(),
            current_version_no: d.current_version_no,
            created_by: d.created_by.map(|u| u.to_string()),
            created_at: d.created_at,
            updated_at: d.updated_at,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct VersionResponse {
    pub id: String,
    pub document_id: String,
    pub version_no: i32,
    pub is_current: bool,
    pub storage_key: Option<String>,
    pub original_filename: Option<String>,
    pub mime_type: Option<String>,
    pub size_bytes: Option<i64>,
    pub checksum_sha256: Option<String>,
    pub content_json: Option<Value>,
    pub change_note: Option<String>,
    pub metadata: Option<Value>,
    pub created_by: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl From<DocumentVersion> for VersionResponse {
    fn from(v: DocumentVersion) -> Self {
        Self {
            id: v.id.to_string(),
            document_id: v.document_id.to_string(),
            version_no: v.version_no,
            is_current: v.is_current,
            storage_key: v.storage_key,
            original_filename: v.original_filename,
            mime_type: v.mime_type,
            size_bytes: v.size_bytes,
            checksum_sha256: v.checksum_sha256,
            content_json: json_text(v.content_json),
            change_note: v.change_note,
            metadata: json_text(v.metadata),
            created_by: v.created_by.map(|u| u.to_string()),
            created_at: v.created_at,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct DocumentDetailResponse {
    #[serde(flatten)]
    pub document: DocumentResponse,
    pub current_version: Option<VersionResponse>,
    pub versions: Vec<VersionResponse>,
}

// ---------------------------------------------------------------------------
// Pricing
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct MoneyResponse {
    pub amount_minor: i64,
    pub currency: String,
}

impl From<Money> for MoneyResponse {
    fn from(m: Money) -> Self {
        Self {
            amount_minor: m.amount_minor,
            currency: m.currency.as_str().to_string(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct PlanResponse {
    pub id: String,
    pub code: String,
    pub name: String,
    pub price_per_machine_year: MoneyResponse,
    pub onboarding_fee: MoneyResponse,
    pub features: Option<Value>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
}

impl From<Plan> for PlanResponse {
    fn from(p: Plan) -> Self {
        Self {
            id: p.id.to_string(),
            code: p.code.as_str().to_string(),
            name: p.name,
            price_per_machine_year: p.price_per_machine_year.into(),
            onboarding_fee: p.onboarding_fee.into(),
            features: json_text(p.features),
            is_active: p.is_active,
            created_at: p.created_at,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct SubscriptionResponse {
    pub id: String,
    pub organization_id: String,
    pub status: String,
    pub trial_ends_at: Option<DateTime<Utc>>,
    pub current_period_start: Option<DateTime<Utc>>,
    pub current_period_end: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<Subscription> for SubscriptionResponse {
    fn from(s: Subscription) -> Self {
        Self {
            id: s.id.to_string(),
            organization_id: s.organization_id.to_string(),
            status: s.status.as_str().to_string(),
            trial_ends_at: s.trial_ends_at,
            current_period_start: s.current_period_start,
            current_period_end: s.current_period_end,
            created_at: s.created_at,
            updated_at: s.updated_at,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct EstimateLineResponse {
    pub machine_id: String,
    pub machine_name: String,
    pub plan_code: Option<String>,
    pub recurring: MoneyResponse,
    pub onboarding: MoneyResponse,
}

impl From<MachineLine> for EstimateLineResponse {
    fn from(l: MachineLine) -> Self {
        Self {
            machine_id: l.machine_id.to_string(),
            machine_name: l.machine_name,
            plan_code: l.plan_code.map(|c| c.as_str().to_string()),
            recurring: l.recurring.into(),
            onboarding: l.onboarding.into(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct EstimateResponse {
    pub currency: String,
    pub lines: Vec<EstimateLineResponse>,
    pub recurring_total: MoneyResponse,
    pub onboarding_total: MoneyResponse,
}

impl From<Estimate> for EstimateResponse {
    fn from(e: Estimate) -> Self {
        Self {
            currency: e.currency.as_str().to_string(),
            lines: e.lines.into_iter().map(Into::into).collect(),
            recurring_total: e.recurring_total.into(),
            onboarding_total: e.onboarding_total.into(),
        }
    }
}
