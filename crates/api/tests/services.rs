//! Service-layer tests against in-memory fakes (no DB). Covers auth flow,
//! RBAC, cross-org isolation, the document version-bump + restore transaction,
//! and pricing math.

mod common;

use common::World;

use electronix_id_api::application::document_service::FileUpload;
use electronix_id_api::application::error::ApplicationError;
use electronix_id_api::application::machine_service::NewMachine;
use electronix_id_api::application::ports::subscription_repo::SubscriptionRepository;
use electronix_id_api::domain::value_objects::{
    DocumentCategory, MachineStatus, Role, StorageKind,
};

use sha2::{Digest, Sha256};

fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    format!("{:x}", h.finalize())
}

async fn register_owner(
    w: &World,
    org_name: &str,
    email: &str,
) -> (
    electronix_id_api::domain::ids::OrgId,
    electronix_id_api::domain::ids::UserId,
) {
    let (user, org, _tokens) = w
        .auth
        .register(
            org_name.into(),
            email.into(),
            "password123".into(),
            "Owner".into(),
        )
        .await
        .expect("register");
    (org.id, user.id)
}

#[tokio::test]
async fn register_creates_org_owner_and_trial_subscription() {
    let w = World::new();
    let (user, org, tokens) = w
        .auth
        .register(
            "Acme Robotics".into(),
            "owner@acme.com".into(),
            "password123".into(),
            "Owner".into(),
        )
        .await
        .unwrap();

    assert_eq!(user.role, Role::Owner);
    assert_eq!(org.slug, "acme-robotics");
    assert!(!tokens.access_token.is_empty());
    assert!(!tokens.refresh_token.is_empty());

    // trialing subscription was created
    let sub = w
        .subs
        .find_by_org(org.id)
        .await
        .unwrap()
        .expect("subscription");
    assert_eq!(
        sub.status,
        electronix_id_api::domain::plan::SubscriptionStatus::Trialing
    );

    // me() returns user + org
    let (me, me_org) = w.auth.me(org.id, user.id).await.unwrap();
    assert_eq!(me.id, user.id);
    assert_eq!(me_org.id, org.id);
}

#[tokio::test]
async fn duplicate_email_register_conflicts() {
    let w = World::new();
    register_owner(&w, "A", "dup@x.com").await;
    let err = w
        .auth
        .register(
            "B".into(),
            "dup@x.com".into(),
            "password123".into(),
            "Two".into(),
        )
        .await
        .unwrap_err();
    assert!(matches!(err, ApplicationError::Conflict(_)));
}

#[tokio::test]
async fn login_succeeds_and_rejects_bad_password() {
    let w = World::new();
    register_owner(&w, "Acme", "owner@acme.com").await;

    assert!(
        w.auth
            .login("owner@acme.com".into(), "password123".into())
            .await
            .is_ok()
    );

    let err = w
        .auth
        .login("owner@acme.com".into(), "wrong".into())
        .await
        .unwrap_err();
    assert!(matches!(err, ApplicationError::Unauthorized(_)));

    let err = w
        .auth
        .login("nobody@acme.com".into(), "password123".into())
        .await
        .unwrap_err();
    assert!(matches!(err, ApplicationError::Unauthorized(_)));
}

#[tokio::test]
async fn refresh_rotates_and_old_token_is_revoked() {
    let w = World::new();
    let (_user, _org, tokens) = w
        .auth
        .register(
            "Acme".into(),
            "owner@acme.com".into(),
            "password123".into(),
            "Owner".into(),
        )
        .await
        .unwrap();

    let rotated = w.auth.refresh(tokens.refresh_token.clone()).await.unwrap();
    assert_ne!(rotated.refresh_token, tokens.refresh_token);

    // the old refresh token no longer works (rotation revoked it)
    let err = w
        .auth
        .refresh(tokens.refresh_token.clone())
        .await
        .unwrap_err();
    assert!(matches!(err, ApplicationError::Unauthorized(_)));

    // the new one works
    assert!(w.auth.refresh(rotated.refresh_token.clone()).await.is_ok());
}

#[tokio::test]
async fn logout_revokes_refresh_token() {
    let w = World::new();
    let (_u, _o, tokens) = w
        .auth
        .register(
            "Acme".into(),
            "owner@acme.com".into(),
            "password123".into(),
            "Owner".into(),
        )
        .await
        .unwrap();
    w.auth.logout(tokens.refresh_token.clone()).await.unwrap();
    let err = w.auth.refresh(tokens.refresh_token).await.unwrap_err();
    assert!(matches!(err, ApplicationError::Unauthorized(_)));
}

#[tokio::test]
async fn rbac_viewer_cannot_create_machine_engineer_can() {
    let w = World::new();
    let (org, actor) = register_owner(&w, "Acme", "owner@acme.com").await;

    let forbidden = w
        .machines_svc
        .create(
            org,
            Role::Viewer,
            actor,
            NewMachine {
                name: "CNC-1".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(forbidden, ApplicationError::Forbidden(_)));

    assert!(
        w.machines_svc
            .create(
                org,
                Role::Engineer,
                actor,
                NewMachine {
                    name: "CNC-1".into(),
                    ..Default::default()
                }
            )
            .await
            .is_ok()
    );
}

#[tokio::test]
async fn cross_org_isolation_machines_and_users() {
    let w = World::new();
    let (org_a, actor_a) = register_owner(&w, "Acme", "a@x.com").await;
    let (org_b, _actor_b) = register_owner(&w, "Beta", "b@x.com").await;

    let machine = w
        .machines_svc
        .create(
            org_a,
            Role::Owner,
            actor_a,
            NewMachine {
                name: "CNC-1".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap();

    // org A sees it
    assert!(w.machines_svc.get(org_a, machine.id).await.is_ok());
    // org B must not
    let err = w.machines_svc.get(org_b, machine.id).await.unwrap_err();
    assert!(matches!(err, ApplicationError::NotFound(_)));

    // user from org A invisible to org B
    let err = w.users_svc.get(org_b, actor_a).await.unwrap_err();
    assert!(matches!(err, ApplicationError::NotFound(_)));
}

#[tokio::test]
async fn machine_tier_and_pagination() {
    let w = World::new();
    let (org, actor) = register_owner(&w, "Acme", "owner@acme.com").await;

    for i in 0..3 {
        w.machines_svc
            .create(
                org,
                Role::Owner,
                actor,
                NewMachine {
                    name: format!("M{i}"),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
    }

    let (page, total) = w.machines_svc.list(org, 2, 0).await.unwrap();
    assert_eq!(total, 3);
    assert_eq!(page.len(), 2);

    let m = &page[0];
    let tiered = w
        .machines_svc
        .set_tier(
            org,
            Role::Admin,
            m.id,
            electronix_id_api::domain::value_objects::Tier::Basic,
        )
        .await
        .unwrap();
    assert_eq!(
        tiered.plan_id,
        Some(
            w.plans
                .id_of(electronix_id_api::domain::value_objects::Tier::Basic)
        )
    );
}

#[tokio::test]
async fn document_file_versioning_and_photo_primary() {
    let w = World::new();
    let (org, actor) = register_owner(&w, "Acme", "owner@acme.com").await;
    let machine = w
        .machines_svc
        .create(
            org,
            Role::Owner,
            actor,
            NewMachine {
                name: "CNC-1".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap();

    // a photo slot (file kind)
    let slot = w
        .docs_svc
        .create_slot(
            org,
            Role::Engineer,
            actor,
            machine.id,
            DocumentCategory::Photo,
            "Front".into(),
            StorageKind::File,
        )
        .await
        .unwrap();

    let v1_bytes = b"image-v1".to_vec();
    let v1 = w
        .docs_svc
        .add_file_version(
            org,
            Role::Engineer,
            actor,
            slot.id,
            FileUpload {
                original_filename: "front.jpg".into(),
                mime_type: Some("image/jpeg".into()),
                bytes: v1_bytes.clone(),
            },
            Some("first".into()),
            None,
        )
        .await
        .unwrap();
    assert_eq!(v1.version_no, 1);
    assert_eq!(
        v1.checksum_sha256.as_deref(),
        Some(sha256_hex(&v1_bytes).as_str())
    );

    let v2_bytes = b"image-v2-bigger".to_vec();
    let v2 = w
        .docs_svc
        .add_file_version(
            org,
            Role::Engineer,
            actor,
            slot.id,
            FileUpload {
                original_filename: "front.jpg".into(),
                mime_type: Some("image/jpeg".into()),
                bytes: v2_bytes.clone(),
            },
            Some("second".into()),
            None,
        )
        .await
        .unwrap();
    assert_eq!(v2.version_no, 2);

    // history shows 2, current = v2
    let (_doc, current, versions) = w.docs_svc.get(org, slot.id).await.unwrap();
    assert_eq!(versions.len(), 2);
    assert_eq!(current.unwrap().version_no, 2);

    // photo updated the machine's primary pointer to the latest version
    let m = w.machines_svc.get(org, machine.id).await.unwrap();
    assert_eq!(m.primary_photo_version_id, Some(v2.id));

    // download returns the stored bytes, checksum matches
    let (_v, bytes) = w.docs_svc.download(org, slot.id, 2).await.unwrap();
    assert_eq!(bytes, v2_bytes);

    // restore v1 -> creates v3 (current) copying v1's payload
    let v3 = w
        .docs_svc
        .restore(org, Role::Engineer, actor, slot.id, 1)
        .await
        .unwrap();
    assert_eq!(v3.version_no, 3);
    assert_eq!(v3.change_note.as_deref(), Some("restored from v1"));
    let (_v, restored_bytes) = w.docs_svc.download(org, slot.id, 3).await.unwrap();
    assert_eq!(restored_bytes, v1_bytes);

    // primary photo now points at v3
    let m = w.machines_svc.get(org, machine.id).await.unwrap();
    assert_eq!(m.primary_photo_version_id, Some(v3.id));
}

#[tokio::test]
async fn document_json_versioning_validates_shape_and_kind() {
    let w = World::new();
    let (org, actor) = register_owner(&w, "Acme", "owner@acme.com").await;
    let machine = w
        .machines_svc
        .create(
            org,
            Role::Owner,
            actor,
            NewMachine {
                name: "PLC line".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap();

    let slot = w
        .docs_svc
        .create_slot(
            org,
            Role::Engineer,
            actor,
            machine.id,
            DocumentCategory::VfdParameters,
            "VFD".into(),
            StorageKind::Json,
        )
        .await
        .unwrap();

    // object is accepted
    let v1 = w
        .docs_svc
        .add_json_version(
            org,
            Role::Engineer,
            actor,
            slot.id,
            r#"{"freq":50}"#.into(),
            true,
            None,
            None,
        )
        .await
        .unwrap();
    assert_eq!(v1.version_no, 1);

    // scalar rejected (caller reports is_object_or_array = false)
    let err = w
        .docs_svc
        .add_json_version(
            org,
            Role::Engineer,
            actor,
            slot.id,
            "42".into(),
            false,
            None,
            None,
        )
        .await
        .unwrap_err();
    assert!(matches!(err, ApplicationError::Validation(_)));

    // uploading a file to a json slot is rejected
    let err = w
        .docs_svc
        .add_file_version(
            org,
            Role::Engineer,
            actor,
            slot.id,
            FileUpload {
                original_filename: "x.bin".into(),
                mime_type: None,
                bytes: vec![1, 2, 3],
            },
            None,
            None,
        )
        .await
        .unwrap_err();
    assert!(matches!(err, ApplicationError::Validation(_)));
}

#[tokio::test]
async fn pricing_estimate_sums_active_machines_and_excludes_predict_recurring() {
    use electronix_id_api::domain::value_objects::Tier;
    let w = World::new();
    let (org, actor) = register_owner(&w, "Acme", "owner@acme.com").await;

    // two active machines on Basic
    for i in 0..2 {
        let m = w
            .machines_svc
            .create(
                org,
                Role::Owner,
                actor,
                NewMachine {
                    name: format!("B{i}"),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        w.machines_svc
            .set_tier(org, Role::Admin, m.id, Tier::Basic)
            .await
            .unwrap();
    }

    // one machine in maintenance on Basic -> no recurring
    let maint = w
        .machines_svc
        .create(
            org,
            Role::Owner,
            actor,
            NewMachine {
                name: "maint".into(),
                status: Some(MachineStatus::Maintenance),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    w.machines_svc
        .set_tier(org, Role::Admin, maint.id, Tier::Basic)
        .await
        .unwrap();

    // one active machine on Predict (inactive plan) -> no recurring
    let pred = w
        .machines_svc
        .create(
            org,
            Role::Owner,
            actor,
            NewMachine {
                name: "pred".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    w.machines_svc
        .set_tier(org, Role::Admin, pred.id, Tier::Predict)
        .await
        .unwrap();

    let est = w.pricing_svc.estimate(org).await.unwrap();

    // recurring = 2 * 150_000 (only the two active Basic machines)
    assert_eq!(est.recurring_total.amount_minor, 300_000);

    // the predict line itself has zero recurring
    let pred_line = est.lines.iter().find(|l| l.machine_id == pred.id).unwrap();
    assert_eq!(pred_line.recurring.amount_minor, 0);

    // onboarding: all 4 machines created in-period, each with a plan -> 4 * 60_000
    assert_eq!(est.onboarding_total.amount_minor, 240_000);
}
