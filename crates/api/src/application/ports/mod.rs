//! Ports — the trait boundary the application depends on. Infrastructure
//! provides the adapters. Think of these as the hardware-abstraction layer:
//! a service calls `find_by_id` the same way no matter what backs it.

pub mod document_repo;
pub mod file_storage;
pub mod machine_repo;
pub mod organization_repo;
pub mod password_hasher;
pub mod plan_repo;
pub mod refresh_token_repo;
pub mod subscription_repo;
pub mod token_service;
pub mod user_repo;
