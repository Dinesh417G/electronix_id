//! Value objects: small, validated, self-checking types. Construct via the
//! `parse`/`from_str` constructors so an invalid value can never exist.

use std::str::FromStr;

use crate::domain::error::DomainError;

// ---------------------------------------------------------------------------
// Email
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Email(String);

impl Email {
    /// Trim + lowercase, then do a minimal structural check. We are not trying
    /// to fully validate RFC 5322 — just reject obviously broken addresses.
    pub fn parse(raw: &str) -> Result<Self, DomainError> {
        let v = raw.trim().to_lowercase();
        if Self::is_valid(&v) {
            Ok(Self(v))
        } else {
            Err(DomainError::InvalidEmail(raw.to_string()))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }

    fn is_valid(s: &str) -> bool {
        if s.contains(char::is_whitespace) {
            return false;
        }
        let mut parts = s.splitn(2, '@');
        let local = parts.next().unwrap_or("");
        let domain = parts.next().unwrap_or("");
        !local.is_empty()
            && domain.len() >= 3
            && domain.contains('.')
            && !domain.starts_with('.')
            && !domain.ends_with('.')
    }
}

impl std::fmt::Display for Email {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

// ---------------------------------------------------------------------------
// Role  (owner ⊃ admin ⊃ engineer ⊃ viewer)
// ---------------------------------------------------------------------------

/// Ordering matters: derived `PartialOrd`/`Ord` follow declaration order, so
/// `Viewer < Engineer < Admin < Owner`. `require_role(min)` is `role >= min`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Role {
    Viewer,
    Engineer,
    Admin,
    Owner,
}

impl Role {
    pub fn as_str(&self) -> &'static str {
        match self {
            Role::Owner => "owner",
            Role::Admin => "admin",
            Role::Engineer => "engineer",
            Role::Viewer => "viewer",
        }
    }

    /// True if this role is at least as privileged as `min`.
    pub fn at_least(self, min: Role) -> bool {
        self >= min
    }
}

impl FromStr for Role {
    type Err = DomainError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "owner" => Ok(Role::Owner),
            "admin" => Ok(Role::Admin),
            "engineer" => Ok(Role::Engineer),
            "viewer" => Ok(Role::Viewer),
            other => Err(DomainError::InvalidRole(other.to_string())),
        }
    }
}

impl std::fmt::Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// DocumentCategory  (unknown/custom -> Other)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocumentCategory {
    Photo,
    Bom,
    MechanicalManual,
    ElectricalManual,
    PlcProgram,
    CncProgram,
    RobotProgram,
    HmiProgram,
    VfdParameters,
    ParameterBackup,
    ElectricalDrawing,
    MechanicalDrawing,
    Datasheet,
    Specification,
    Certificate,
    MaintenanceRecord,
    Other,
}

impl DocumentCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            DocumentCategory::Photo => "photo",
            DocumentCategory::Bom => "bom",
            DocumentCategory::MechanicalManual => "mechanical_manual",
            DocumentCategory::ElectricalManual => "electrical_manual",
            DocumentCategory::PlcProgram => "plc_program",
            DocumentCategory::CncProgram => "cnc_program",
            DocumentCategory::RobotProgram => "robot_program",
            DocumentCategory::HmiProgram => "hmi_program",
            DocumentCategory::VfdParameters => "vfd_parameters",
            DocumentCategory::ParameterBackup => "parameter_backup",
            DocumentCategory::ElectricalDrawing => "electrical_drawing",
            DocumentCategory::MechanicalDrawing => "mechanical_drawing",
            DocumentCategory::Datasheet => "datasheet",
            DocumentCategory::Specification => "specification",
            DocumentCategory::Certificate => "certificate",
            DocumentCategory::MaintenanceRecord => "maintenance_record",
            DocumentCategory::Other => "other",
        }
    }

    /// Parse from the stored/string form. Unknown or custom values map to
    /// `Other` (the system is deliberately open-ended about artifact types).
    pub fn parse(s: &str) -> Self {
        match s {
            "photo" => DocumentCategory::Photo,
            "bom" => DocumentCategory::Bom,
            "mechanical_manual" => DocumentCategory::MechanicalManual,
            "electrical_manual" => DocumentCategory::ElectricalManual,
            "plc_program" => DocumentCategory::PlcProgram,
            "cnc_program" => DocumentCategory::CncProgram,
            "robot_program" => DocumentCategory::RobotProgram,
            "hmi_program" => DocumentCategory::HmiProgram,
            "vfd_parameters" => DocumentCategory::VfdParameters,
            "parameter_backup" => DocumentCategory::ParameterBackup,
            "electrical_drawing" => DocumentCategory::ElectricalDrawing,
            "mechanical_drawing" => DocumentCategory::MechanicalDrawing,
            "datasheet" => DocumentCategory::Datasheet,
            "specification" => DocumentCategory::Specification,
            "certificate" => DocumentCategory::Certificate,
            "maintenance_record" => DocumentCategory::MaintenanceRecord,
            _ => DocumentCategory::Other,
        }
    }

    pub fn is_photo(&self) -> bool {
        matches!(self, DocumentCategory::Photo)
    }
}

impl std::fmt::Display for DocumentCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// StorageKind
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageKind {
    File,
    Json,
}

impl StorageKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            StorageKind::File => "file",
            StorageKind::Json => "json",
        }
    }
}

impl FromStr for StorageKind {
    type Err = DomainError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "file" => Ok(StorageKind::File),
            "json" => Ok(StorageKind::Json),
            other => Err(DomainError::InvalidStorageKind(other.to_string())),
        }
    }
}

impl std::fmt::Display for StorageKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// MachineStatus
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MachineStatus {
    #[default]
    Active,
    Maintenance,
    Retired,
}

impl MachineStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            MachineStatus::Active => "active",
            MachineStatus::Maintenance => "maintenance",
            MachineStatus::Retired => "retired",
        }
    }

    pub fn is_active(&self) -> bool {
        matches!(self, MachineStatus::Active)
    }
}

impl FromStr for MachineStatus {
    type Err = DomainError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "active" => Ok(MachineStatus::Active),
            "maintenance" => Ok(MachineStatus::Maintenance),
            "retired" => Ok(MachineStatus::Retired),
            other => Err(DomainError::InvalidMachineStatus(other.to_string())),
        }
    }
}

impl std::fmt::Display for MachineStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// Tier  (plan code: basic | live | predict)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier {
    Basic,
    Live,
    Predict,
}

impl Tier {
    pub fn as_str(&self) -> &'static str {
        match self {
            Tier::Basic => "basic",
            Tier::Live => "live",
            Tier::Predict => "predict",
        }
    }
}

impl FromStr for Tier {
    type Err = DomainError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "basic" => Ok(Tier::Basic),
            "live" => Ok(Tier::Live),
            "predict" => Ok(Tier::Predict),
            other => Err(DomainError::InvalidTier(other.to_string())),
        }
    }
}

impl std::fmt::Display for Tier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// PublicCode  (opaque machine tag for the QR/scan resolver)
// ---------------------------------------------------------------------------

/// Generator for a machine's public tag code.
///
/// The code goes in the QR on the machine and is the only identifier a scanner
/// presents to the resolver. It must be **opaque** (not the time-sortable UUID,
/// which is guessable) and **rotatable** (issue a fresh code to revoke a tag).
///
/// 16 chars of Crockford base32 (alphabet excludes I, L, O, U to avoid misreads)
/// drawn from a CSPRNG → 32^16 ≈ 2^80 space, far too large to enumerate.
pub struct PublicCode;

impl PublicCode {
    /// Crockford base32 alphabet (no I, L, O, U).
    const ALPHABET: &'static [u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
    pub const LEN: usize = 16;

    /// Generate a fresh random 16-char code.
    pub fn generate() -> String {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        (0..Self::LEN)
            .map(|_| Self::ALPHABET[rng.gen_range(0..32)] as char)
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Currency + Money  (integer minor units; never floats)
// ---------------------------------------------------------------------------

/// A 3-letter ISO-4217-style code, stored uppercase.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Currency(String);

impl Currency {
    pub fn parse(s: &str) -> Result<Self, DomainError> {
        let u = s.trim().to_uppercase();
        if u.len() == 3 && u.chars().all(|c| c.is_ascii_alphabetic()) {
            Ok(Self(u))
        } else {
            Err(DomainError::InvalidCurrency(s.to_string()))
        }
    }

    pub fn inr() -> Self {
        Self("INR".to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for Currency {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Money is integer minor units (paise) + a currency code. No floats, ever.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Money {
    pub amount_minor: i64,
    pub currency: Currency,
}

impl Money {
    pub fn new(amount_minor: i64, currency: Currency) -> Self {
        Self {
            amount_minor,
            currency,
        }
    }

    pub fn zero(currency: Currency) -> Self {
        Self {
            amount_minor: 0,
            currency,
        }
    }

    /// Add two amounts of the same currency. Mixing currencies is an error.
    pub fn add(&self, other: &Money) -> Result<Money, DomainError> {
        if self.currency != other.currency {
            return Err(DomainError::CurrencyMismatch);
        }
        Ok(Money::new(
            self.amount_minor + other.amount_minor,
            self.currency.clone(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn email_normalizes_and_validates() {
        assert_eq!(
            Email::parse("  Foo@Bar.COM ").unwrap().as_str(),
            "foo@bar.com"
        );
        assert!(Email::parse("nope").is_err());
        assert!(Email::parse("a@b").is_err());
        assert!(Email::parse("a b@c.com").is_err());
        assert!(Email::parse("@bar.com").is_err());
    }

    #[test]
    fn role_ordering_and_parsing() {
        assert!(Role::Owner > Role::Admin);
        assert!(Role::Admin > Role::Engineer);
        assert!(Role::Engineer > Role::Viewer);
        assert!(Role::Admin.at_least(Role::Engineer));
        assert!(!Role::Viewer.at_least(Role::Engineer));
        assert_eq!("admin".parse::<Role>().unwrap(), Role::Admin);
        assert!("root".parse::<Role>().is_err());
        assert_eq!(Role::Owner.as_str(), "owner");
    }

    #[test]
    fn document_category_unknown_maps_to_other() {
        assert_eq!(
            DocumentCategory::parse("plc_program"),
            DocumentCategory::PlcProgram
        );
        assert_eq!(
            DocumentCategory::parse("totally_custom"),
            DocumentCategory::Other
        );
        // round-trip every known variant through its string form
        for c in [
            DocumentCategory::Photo,
            DocumentCategory::Specification,
            DocumentCategory::MaintenanceRecord,
        ] {
            assert_eq!(DocumentCategory::parse(c.as_str()), c);
        }
        assert!(DocumentCategory::Photo.is_photo());
    }

    #[test]
    fn storage_kind_parse() {
        assert_eq!("file".parse::<StorageKind>().unwrap(), StorageKind::File);
        assert_eq!("json".parse::<StorageKind>().unwrap(), StorageKind::Json);
        assert!("blob".parse::<StorageKind>().is_err());
    }

    #[test]
    fn machine_status_default_is_active() {
        assert_eq!(MachineStatus::default(), MachineStatus::Active);
        assert!(MachineStatus::Active.is_active());
        assert!("retired".parse::<MachineStatus>().unwrap() == MachineStatus::Retired);
        assert!("exploded".parse::<MachineStatus>().is_err());
    }

    #[test]
    fn public_code_is_16_chars_from_alphabet_and_random() {
        let a = PublicCode::generate();
        let b = PublicCode::generate();
        assert_eq!(a.len(), PublicCode::LEN);
        assert!(a.bytes().all(|c| PublicCode::ALPHABET.contains(&c)));
        assert_ne!(a, b, "two draws should not collide");
    }

    #[test]
    fn money_add_and_currency_rules() {
        let a = Money::new(150_000, Currency::inr());
        let b = Money::new(60_000, Currency::inr());
        assert_eq!(a.add(&b).unwrap().amount_minor, 210_000);

        let usd = Money::new(1, Currency::parse("usd").unwrap());
        assert_eq!(usd.currency.as_str(), "USD");
        assert!(a.add(&usd).is_err());

        assert!(Currency::parse("INRR").is_err());
        assert!(Currency::parse("I1R").is_err());
        assert_eq!(Money::zero(Currency::inr()).amount_minor, 0);
    }
}
