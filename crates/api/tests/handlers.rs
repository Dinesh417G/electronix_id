//! HTTP-level integration tests: the router driven with `oneshot` against a real
//! MySQL schema (via `#[sqlx::test]`). Covers the auth lifecycle, cross-org
//! isolation, RBAC, and the pricing/billing reads.

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
use electronix_id_api::web::router::build_router;

fn test_app(pool: MySqlPool) -> Router {
    let storage_root = std::env::temp_dir()
        .join(format!("eid-h-{}", uuid::Uuid::now_v7().simple()))
        .to_string_lossy()
        .into_owned();
    let settings = Arc::new(Settings {
        database_url: String::new(),
        bind_addr: "0.0.0.0:0".to_string(),
        jwt_secret: "test-secret-at-least-32-bytes-long-1234567890".to_string(),
        access_token_ttl: Duration::from_secs(900),
        refresh_token_ttl: Duration::from_secs(2_592_000),
        storage_root,
        max_upload_bytes: 52_428_800,
        cors_allowed_origins: vec!["http://localhost:3000".to_string()],
    });
    build_router(AppState::new(pool, settings))
}

async fn send(app: &Router, req: Request<Body>) -> (StatusCode, Value) {
    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let body = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap()
    };
    (status, body)
}

fn post_json(uri: &str, body: Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
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

fn patch_json_auth(uri: &str, body: Value, token: &str) -> Request<Body> {
    Request::builder()
        .method("PATCH")
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::from(body.to_string()))
        .unwrap()
}

fn get_auth(uri: &str, token: &str) -> Request<Body> {
    Request::builder()
        .method("GET")
        .uri(uri)
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap()
}

/// Register an org owner and return its access token.
async fn register(app: &Router, org: &str, email: &str) -> String {
    let (status, body) = send(
        app,
        post_json(
            "/api/v1/auth/register",
            json!({
                "organization_name": org,
                "email": email,
                "password": "password123",
                "full_name": "Owner",
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    body["access_token"].as_str().unwrap().to_string()
}

#[sqlx::test(migrations = "../../migrations")]
async fn auth_lifecycle(pool: MySqlPool) {
    let app = test_app(pool);

    // register
    let (status, body) = send(
        &app,
        post_json(
            "/api/v1/auth/register",
            json!({
                "organization_name": "Acme",
                "email": "owner@acme.com",
                "password": "password123",
                "full_name": "Owner",
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let access = body["access_token"].as_str().unwrap().to_string();
    let refresh = body["refresh_token"].as_str().unwrap().to_string();

    // me with a valid token
    let (status, body) = send(&app, get_auth("/api/v1/auth/me", &access)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["user"]["email"], "owner@acme.com");
    assert_eq!(body["organization"]["slug"], "acme");

    // me without a token -> 401
    let (status, _) = send(
        &app,
        Request::builder()
            .uri("/api/v1/auth/me")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    // refresh rotates the token
    let (status, body) = send(
        &app,
        post_json("/api/v1/auth/refresh", json!({ "refresh_token": refresh })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let new_refresh = body["refresh_token"].as_str().unwrap().to_string();
    assert_ne!(new_refresh, refresh);

    // the old refresh token is now revoked
    let (status, _) = send(
        &app,
        post_json("/api/v1/auth/refresh", json!({ "refresh_token": refresh })),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    // logout revokes the current refresh token
    let (status, _) = send(
        &app,
        post_json(
            "/api/v1/auth/logout",
            json!({ "refresh_token": new_refresh }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _) = send(
        &app,
        post_json(
            "/api/v1/auth/refresh",
            json!({ "refresh_token": new_refresh }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "../../migrations")]
async fn cross_org_machine_isolation(pool: MySqlPool) {
    let app = test_app(pool);
    let token_a = register(&app, "Acme", "a@x.com").await;
    let token_b = register(&app, "Beta", "b@x.com").await;

    // org A creates a machine
    let (status, body) = send(
        &app,
        post_json_auth("/api/v1/machines", json!({ "name": "CNC-1" }), &token_a),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let machine_id = body["id"].as_str().unwrap().to_string();

    // org A can read it
    let (status, _) = send(
        &app,
        get_auth(&format!("/api/v1/machines/{machine_id}"), &token_a),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    // org B cannot — 404, not a leak
    let (status, _) = send(
        &app,
        get_auth(&format!("/api/v1/machines/{machine_id}"), &token_b),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../../migrations")]
async fn rbac_viewer_cannot_write(pool: MySqlPool) {
    let app = test_app(pool);
    let owner = register(&app, "Acme", "owner@acme.com").await;

    // owner creates a viewer
    let (status, _) = send(
        &app,
        post_json_auth(
            "/api/v1/users",
            json!({
                "email": "viewer@acme.com",
                "password": "password123",
                "full_name": "Viewer",
                "role": "viewer",
            }),
            &owner,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    // viewer logs in
    let (status, body) = send(
        &app,
        post_json(
            "/api/v1/auth/login",
            json!({ "email": "viewer@acme.com", "password": "password123" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let viewer = body["access_token"].as_str().unwrap().to_string();

    // viewer cannot create a machine
    let (status, _) = send(
        &app,
        post_json_auth("/api/v1/machines", json!({ "name": "X" }), &viewer),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[sqlx::test(migrations = "../../migrations")]
async fn pricing_estimate_and_catalog(pool: MySqlPool) {
    let app = test_app(pool);
    let owner = register(&app, "Acme", "owner@acme.com").await;

    // two machines, each on the Basic tier
    for i in 0..2 {
        let (status, body) = send(
            &app,
            post_json_auth(
                "/api/v1/machines",
                json!({ "name": format!("M{i}") }),
                &owner,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let id = body["id"].as_str().unwrap().to_string();
        let (status, _) = send(
            &app,
            patch_json_auth(
                &format!("/api/v1/machines/{id}/tier"),
                json!({ "plan_code": "basic" }),
                &owner,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
    }

    // catalog lists all three plans
    let (status, body) = send(&app, get_auth("/api/v1/plans", &owner)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data"].as_array().unwrap().len(), 3);

    // subscription is trialing
    let (status, body) = send(&app, get_auth("/api/v1/organization/subscription", &owner)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "trialing");

    // estimate: 2 active Basic machines -> 300_000 recurring, 120_000 onboarding
    let (status, body) = send(
        &app,
        get_auth("/api/v1/organization/billing/estimate", &owner),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["recurring_total"]["amount_minor"], 300_000);
    assert_eq!(body["onboarding_total"]["amount_minor"], 120_000);
}

#[sqlx::test(migrations = "../../migrations")]
async fn document_file_version_roundtrip_over_http(pool: MySqlPool) {
    let app = test_app(pool);
    let owner = register(&app, "Acme", "owner@acme.com").await;

    let (status, body) = send(
        &app,
        post_json_auth("/api/v1/machines", json!({ "name": "Press" }), &owner),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let machine_id = body["id"].as_str().unwrap().to_string();

    // create a JSON spec slot
    let (status, body) = send(
        &app,
        post_json_auth(
            &format!("/api/v1/machines/{machine_id}/documents"),
            json!({ "category": "specification", "name": "Specs", "storage_kind": "json" }),
            &owner,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let doc_id = body["id"].as_str().unwrap().to_string();

    // add v1 then v2 (JSON kind)
    let (status, body) = send(
        &app,
        post_json_auth(
            &format!("/api/v1/documents/{doc_id}/versions"),
            json!({ "content": { "rpm": 1000 }, "change_note": "first" }),
            &owner,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(body["version_no"], 1);

    let (status, body) = send(
        &app,
        post_json_auth(
            &format!("/api/v1/documents/{doc_id}/versions"),
            json!({ "content": { "rpm": 1200 } }),
            &owner,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(body["version_no"], 2);

    // detail shows current = v2 and two versions
    let (status, body) = send(
        &app,
        get_auth(&format!("/api/v1/documents/{doc_id}"), &owner),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["current_version"]["version_no"], 2);
    assert_eq!(body["versions"].as_array().unwrap().len(), 2);

    // restore v1 -> creates v3 (current)
    let (status, body) = send(
        &app,
        post_json_auth(
            &format!("/api/v1/documents/{doc_id}/versions/1/restore"),
            json!({}),
            &owner,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(body["version_no"], 3);
    assert_eq!(body["change_note"], "restored from v1");
    assert_eq!(body["content_json"]["rpm"], 1000);
}
