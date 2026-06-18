//! Resolver-specific configuration. The DB url, JWT secret, storage root, and
//! CORS list are reused from the api crate's [`Settings`](electronix_id_api::config::Settings);
//! only the bind address differs (the resolver is a separate public service on
//! its own port).

/// Address the public resolver binds to. Defaults to `0.0.0.0:8081` so it never
/// collides with the tenant api on `:8080`.
pub fn resolver_bind_addr() -> String {
    std::env::var("RESOLVER_BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:8081".to_string())
}
