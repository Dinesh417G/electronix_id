//! Resolver HTTP tests: drive both routers (api to seed, resolver under test)
//! with `oneshot` against a real MySQL schema via `#[sqlx::test]`. They share
//! one pool and one JWT secret, so api-issued tokens verify in the resolver.

use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use sqlx::MySqlPool;
use tower::ServiceExt;

use electronix_id_api::config::Settings;
use electronix_id_api::state::AppState;
use electronix_id_resolver::router::build_router as build_resolver;
use electronix_id_resolver::state::ResolverState;

fn settings() -> Arc<Settings> {
    let storage_root = std::env::temp_dir()
        .join(format!("eid-r-{}", uuid::Uuid::now_v7().simple()))
        .to_string_lossy()
        .into_owned();
    Arc::new(Settings {
        database_url: String::new(),
        bind_addr: "0.0.0.0:0".to_string(),
        jwt_secret: "test-secret-at-least-32-bytes-long-1234567890".to_string(),
        access_token_ttl: Duration::from_secs(900),
        refresh_token_ttl: Duration::from_secs(2_592_000),
        storage_root,
        max_upload_bytes: 52_428_800,
        cors_allowed_origins: vec!["http://localhost:3000".to_string()],
    })
}

async fn send(app: &Router, req: Request<Body>) -> (StatusCode, Value) {
    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let body = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(Value::Null)
    };
    (status, body)
}

fn post_json_auth(uri: &str, body: Value, token: &str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::from(body.to_string()))
        .unwrap()
}

fn get(uri: &str) -> Request<Body> {
    Request::builder().uri(uri).body(Body::empty()).unwrap()
}

fn get_auth(uri: &str, token: &str) -> Request<Body> {
    Request::builder()
        .uri(uri)
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap()
}

/// Register an org owner via the api, returning its access token.
async fn register(api: &Router, org: &str, email: &str) -> String {
    let (status, body) = send(
        api,
        Request::builder()
            .method("POST")
            .uri("/api/v1/auth/register")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                json!({
                    "organization_name": org,
                    "email": email,
                    "password": "password123",
                    "full_name": "Owner",
                })
                .to_string(),
            ))
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    body["access_token"].as_str().unwrap().to_string()
}

#[sqlx::test(migrations = "../../migrations")]
async fn scan_summary_full_and_isolation(pool: MySqlPool) {
    let settings = settings();
    let api =
        electronix_id_api::web::router::build_router(AppState::new(pool.clone(), settings.clone()));
    let resolver = build_resolver(ResolverState::new(pool, &settings));

    let owner = register(&api, "Acme", "owner@acme.com").await;
    let other = register(&api, "Beta", "b@beta.com").await;

    // create a machine in Acme; the api auto-issues a public_code
    let (status, machine) = send(
        &api,
        post_json_auth(
            "/api/v1/machines",
            json!({ "name": "CNC-1", "make": "Haas", "model": "VF-2" }),
            &owner,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let code = machine["public_code"].as_str().unwrap().to_string();
    assert_eq!(code.len(), 16);
    let machine_id = machine["id"].as_str().unwrap().to_string();

    // give it one JSON document (a spec) so the full view has contents
    let (status, doc) = send(
        &api,
        post_json_auth(
            &format!("/api/v1/machines/{machine_id}/documents"),
            json!({ "category": "specification", "name": "Specs", "storage_kind": "json" }),
            &owner,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let doc_id = doc["id"].as_str().unwrap().to_string();
    let (status, _) = send(
        &api,
        post_json_auth(
            &format!("/api/v1/documents/{doc_id}/versions"),
            json!({ "content": { "rpm": 1000 }, "change_note": "v1" }),
            &owner,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    // --- public summary: no auth ---
    let (status, body) = send(&resolver, get(&format!("/r/{code}"))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["machine_name"], "CNC-1");
    assert_eq!(body["make"], "Haas");
    assert_eq!(body["organization_name"], "Acme");
    assert_eq!(body["public_code"], code);
    assert_eq!(body["has_photo"], false);
    assert_eq!(body["photo_url"], Value::Null);
    // summary must NOT leak the document inventory
    assert!(body.get("documents").is_none());

    // --- unknown code -> 404 ---
    let (status, _) = send(&resolver, get("/r/NOPENOPENOPE0000")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // --- full view requires auth ---
    let (status, _) = send(&resolver, get(&format!("/r/{code}/full"))).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    // --- full view: cross-org token -> 404 (no existence leak) ---
    let (status, _) = send(&resolver, get_auth(&format!("/r/{code}/full"), &other)).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // --- full view: owner -> 200 with the document inventory ---
    let (status, body) = send(&resolver, get_auth(&format!("/r/{code}/full"), &owner)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["machine_name"], "CNC-1");
    let docs = body["documents"].as_array().unwrap();
    assert_eq!(docs.len(), 1);
    assert_eq!(docs[0]["name"], "Specs");
    assert_eq!(docs[0]["category"], "specification");
    assert_eq!(docs[0]["current_version_no"], 1);
    assert_eq!(docs[0]["current_version"]["content_json"]["rpm"], 1000);

    // --- photo: none uploaded -> 404 ---
    let (status, _) = send(&resolver, get(&format!("/r/{code}/photo"))).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../../migrations")]
async fn rotating_the_tag_revokes_the_old_code(pool: MySqlPool) {
    let settings = settings();
    let api =
        electronix_id_api::web::router::build_router(AppState::new(pool.clone(), settings.clone()));
    let resolver = build_resolver(ResolverState::new(pool, &settings));

    let owner = register(&api, "Acme", "owner@acme.com").await;
    let (_, machine) = send(
        &api,
        post_json_auth("/api/v1/machines", json!({ "name": "Press" }), &owner),
    )
    .await;
    let machine_id = machine["id"].as_str().unwrap().to_string();
    let old_code = machine["public_code"].as_str().unwrap().to_string();

    // old code resolves
    let (status, _) = send(&resolver, get(&format!("/r/{old_code}"))).await;
    assert_eq!(status, StatusCode::OK);

    // rotate the tag
    let (status, rotated) = send(
        &api,
        post_json_auth(
            &format!("/api/v1/machines/{machine_id}/tag/rotate"),
            json!({}),
            &owner,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let new_code = rotated["public_code"].as_str().unwrap().to_string();
    assert_ne!(new_code, old_code);

    // old code no longer resolves; new one does
    let (status, _) = send(&resolver, get(&format!("/r/{old_code}"))).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = send(&resolver, get(&format!("/r/{new_code}"))).await;
    assert_eq!(status, StatusCode::OK);
}
