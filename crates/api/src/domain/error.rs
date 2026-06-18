//! Domain invariant errors. Hand-rolled `Display`/`Error` so the domain layer
//! stays dependency-free (std + uuid + chrono only — not even thiserror).

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DomainError {
    InvalidEmail(String),
    InvalidRole(String),
    InvalidStorageKind(String),
    InvalidMachineStatus(String),
    InvalidTier(String),
    InvalidCurrency(String),
    InvalidSubscriptionStatus(String),
    CurrencyMismatch,
    InvalidVersionNo,
    EmptyDocumentName,
}

impl std::fmt::Display for DomainError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DomainError::InvalidEmail(s) => write!(f, "invalid email: {s}"),
            DomainError::InvalidRole(s) => write!(f, "invalid role: {s}"),
            DomainError::InvalidStorageKind(s) => write!(f, "invalid storage kind: {s}"),
            DomainError::InvalidMachineStatus(s) => write!(f, "invalid machine status: {s}"),
            DomainError::InvalidTier(s) => write!(f, "invalid tier: {s}"),
            DomainError::InvalidCurrency(s) => write!(f, "invalid currency: {s}"),
            DomainError::InvalidSubscriptionStatus(s) => {
                write!(f, "invalid subscription status: {s}")
            }
            DomainError::CurrencyMismatch => write!(f, "currency mismatch"),
            DomainError::InvalidVersionNo => write!(f, "version number must be positive"),
            DomainError::EmptyDocumentName => write!(f, "document name is required"),
        }
    }
}

impl std::error::Error for DomainError {}
