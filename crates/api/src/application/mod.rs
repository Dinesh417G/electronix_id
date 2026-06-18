//! Application layer — use cases (services) + ports (repository/infra traits).
//! Depends on `domain` only.

pub mod error;
pub mod ports;

pub mod auth_service;
pub mod document_service;
pub mod machine_service;
pub mod organization_service;
pub mod pricing_service;
pub mod user_service;
