//! Application configuration, loaded from environment variables.
//!
//! Mental model: this is the machine's parameter set — read once at power-on,
//! validated, then held immutable for the run.

use std::time::Duration;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("missing required env var: {0}")]
    Missing(String),
    #[error("invalid value for {0}: {1}")]
    Invalid(String, String),
    #[error("JWT_SECRET must be at least 32 bytes")]
    JwtSecretTooShort,
}

#[derive(Debug, Clone)]
pub struct Settings {
    pub database_url: String,
    pub bind_addr: String,
    pub jwt_secret: String,
    pub access_token_ttl: Duration,
    pub refresh_token_ttl: Duration,
    pub storage_root: String,
    pub max_upload_bytes: usize,
    pub cors_allowed_origins: Vec<String>,
}

impl Settings {
    pub fn from_env() -> Result<Self, ConfigError> {
        let jwt_secret = req("JWT_SECRET")?;
        if jwt_secret.len() < 32 {
            return Err(ConfigError::JwtSecretTooShort);
        }

        let cors_allowed_origins = opt("CORS_ALLOWED_ORIGINS", "http://localhost:3000")
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        Ok(Self {
            database_url: req("DATABASE_URL")?,
            bind_addr: opt("BIND_ADDR", "0.0.0.0:8080"),
            jwt_secret,
            access_token_ttl: Duration::from_secs(parse_u64("ACCESS_TOKEN_TTL_SECS", 900)?),
            refresh_token_ttl: Duration::from_secs(parse_u64("REFRESH_TOKEN_TTL_SECS", 2_592_000)?),
            storage_root: opt("STORAGE_ROOT", "./storage"),
            max_upload_bytes: parse_u64("MAX_UPLOAD_BYTES", 52_428_800)? as usize,
            cors_allowed_origins,
        })
    }
}

fn req(key: &str) -> Result<String, ConfigError> {
    std::env::var(key).map_err(|_| ConfigError::Missing(key.to_string()))
}

fn opt(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

fn parse_u64(key: &str, default: u64) -> Result<u64, ConfigError> {
    match std::env::var(key) {
        Ok(v) => v
            .parse()
            .map_err(|_| ConfigError::Invalid(key.to_string(), v)),
        Err(_) => Ok(default),
    }
}
