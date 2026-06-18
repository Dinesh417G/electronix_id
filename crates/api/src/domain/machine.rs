use chrono::{DateTime, Utc};

use crate::domain::ids::{MachineId, OrgId, PlanId, UserId, VersionId};
use crate::domain::value_objects::MachineStatus;

/// Core machine identity. Rich, evolving specs live in versioned documents,
/// not here.
#[derive(Debug, Clone)]
pub struct Machine {
    pub id: MachineId,
    pub organization_id: OrgId,
    /// This machine's pricing tier (a plan). `None` = no tier assigned yet.
    pub plan_id: Option<PlanId>,
    pub name: String,
    pub make: Option<String>,
    pub model: Option<String>,
    pub serial_number: Option<String>,
    pub asset_tag: Option<String>,
    /// Opaque public tag code for the QR/scan resolver (`None` = no active tag).
    /// Encoded in the machine's QR; rotatable to revoke a tag. See
    /// [`crate::domain::value_objects::PublicCode`].
    pub public_code: Option<String>,
    pub location: Option<String>,
    pub year_installed: Option<i16>,
    pub status: MachineStatus,
    /// Convenience pointer to the current primary photo version.
    pub primary_photo_version_id: Option<VersionId>,
    pub created_by: Option<UserId>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
