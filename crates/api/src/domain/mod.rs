//! Domain layer — pure business types. Depends on nothing but std + uuid + chrono.
//! No sqlx, no axum, no serde-for-the-wire concerns leak in here.

pub mod document;
pub mod error;
pub mod ids;
pub mod machine;
pub mod organization;
pub mod plan;
pub mod user;
pub mod value_objects;
