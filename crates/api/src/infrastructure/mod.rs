//! Infrastructure layer — adapters that implement `application::ports`.
//! Only this layer knows MySQL, argon2, JWT, and the filesystem exist.

pub mod db;
pub mod persistence;
pub mod security;
pub mod storage;
