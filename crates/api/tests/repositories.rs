//! Repository integration tests against a real MySQL schema. Each `#[sqlx::test]`
//! gets an isolated, freshly-migrated database (so the three plans are seeded)
//! and is rolled back afterwards. Covers CRUD + the §5 document version-bump
//! transaction.

use chrono::Utc;
use sqlx::MySqlPool;

use electronix_id_api::application::ports::document_repo::{DocumentRepository, NewVersionInput};
use electronix_id_api::application::ports::machine_repo::MachineRepository;
use electronix_id_api::application::ports::organization_repo::OrganizationRepository;
use electronix_id_api::application::ports::plan_repo::PlanRepository;
use electronix_id_api::application::ports::subscription_repo::SubscriptionRepository;
use electronix_id_api::application::ports::user_repo::UserRepository;
use electronix_id_api::domain::document::Document;
use electronix_id_api::domain::ids::{
    DocumentId, MachineId, OrgId, SubscriptionId, UserId, VersionId,
};
use electronix_id_api::domain::machine::Machine;
use electronix_id_api::domain::organization::Organization;
use electronix_id_api::domain::plan::{Subscription, SubscriptionStatus};
use electronix_id_api::domain::user::User;
use electronix_id_api::domain::value_objects::{
    DocumentCategory, Email, MachineStatus, Role, StorageKind, Tier,
};
use electronix_id_api::infrastructure::persistence::mysql_document_repo::MySqlDocumentRepo;
use electronix_id_api::infrastructure::persistence::mysql_machine_repo::MySqlMachineRepo;
use electronix_id_api::infrastructure::persistence::mysql_organization_repo::MySqlOrganizationRepo;
use electronix_id_api::infrastructure::persistence::mysql_plan_repo::MySqlPlanRepo;
use electronix_id_api::infrastructure::persistence::mysql_subscription_repo::MySqlSubscriptionRepo;
use electronix_id_api::infrastructure::persistence::mysql_user_repo::MySqlUserRepo;

fn org(name: &str) -> Organization {
    let now = Utc::now();
    Organization {
        id: OrgId::new(),
        name: name.to_string(),
        slug: Organization::slugify(name),
        created_at: now,
        updated_at: now,
    }
}

fn user(org_id: OrgId, email: &str, role: Role) -> User {
    let now = Utc::now();
    User {
        id: UserId::new(),
        organization_id: org_id,
        email: Email::parse(email).unwrap(),
        password_hash: "hash".to_string(),
        full_name: "Test User".to_string(),
        role,
        is_active: true,
        created_at: now,
        updated_at: now,
    }
}

fn machine(org_id: OrgId, name: &str) -> Machine {
    let now = Utc::now();
    Machine {
        id: MachineId::new(),
        organization_id: org_id,
        plan_id: None,
        name: name.to_string(),
        make: None,
        model: None,
        serial_number: None,
        asset_tag: None,
        location: None,
        year_installed: None,
        status: MachineStatus::Active,
        primary_photo_version_id: None,
        created_by: None,
        created_at: now,
        updated_at: now,
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn organization_crud(pool: MySqlPool) {
    let repo = MySqlOrganizationRepo::new(pool);
    let o = org("Acme Robotics");

    repo.create(&o).await.unwrap();

    let found = repo.find_by_id(o.id).await.unwrap();
    assert_eq!(found.name, "Acme Robotics");
    assert_eq!(found.slug, "acme-robotics");

    assert!(repo.find_by_slug("acme-robotics").await.unwrap().is_some());
    assert!(repo.find_by_slug("nope").await.unwrap().is_none());

    let mut updated = found;
    updated.name = "Acme Inc".to_string();
    repo.update(&updated).await.unwrap();
    assert_eq!(repo.find_by_id(o.id).await.unwrap().name, "Acme Inc");
}

#[sqlx::test(migrations = "../../migrations")]
async fn user_crud_is_org_scoped(pool: MySqlPool) {
    let orgs = MySqlOrganizationRepo::new(pool.clone());
    let users = MySqlUserRepo::new(pool.clone());

    let a = org("Org A");
    let b = org("Org B");
    orgs.create(&a).await.unwrap();
    orgs.create(&b).await.unwrap();

    let u = user(a.id, "owner@a.com", Role::Owner);
    users.create(&u).await.unwrap();

    // org-scoped lookup resolves only within the owning org
    assert!(users.find_by_id(a.id, u.id).await.is_ok());
    assert!(users.find_by_id(b.id, u.id).await.is_err());

    assert!(
        users
            .exists_by_email(&Email::parse("owner@a.com").unwrap())
            .await
            .unwrap()
    );
    let by_email = users
        .find_by_email(&Email::parse("owner@a.com").unwrap())
        .await
        .unwrap();
    assert_eq!(by_email.unwrap().id, u.id);

    // list is org-scoped with a correct total
    let (page, total) = users.list(a.id, 10, 0).await.unwrap();
    assert_eq!(total, 1);
    assert_eq!(page.len(), 1);
    let (page_b, total_b) = users.list(b.id, 10, 0).await.unwrap();
    assert_eq!(total_b, 0);
    assert!(page_b.is_empty());

    // update
    let mut u2 = u.clone();
    u2.full_name = "Renamed".to_string();
    u2.role = Role::Admin;
    users.update(&u2).await.unwrap();
    let reloaded = users.find_by_id(a.id, u.id).await.unwrap();
    assert_eq!(reloaded.full_name, "Renamed");
    assert_eq!(reloaded.role, Role::Admin);

    // cross-org delete is a no-op NotFound; same-org delete works
    assert!(users.delete(b.id, u.id).await.is_err());
    assert!(users.delete(a.id, u.id).await.is_ok());
    assert!(users.find_by_id(a.id, u.id).await.is_err());
}

#[sqlx::test(migrations = "../../migrations")]
async fn plans_are_seeded(pool: MySqlPool) {
    let plans = MySqlPlanRepo::new(pool);

    let all = plans.list(false).await.unwrap();
    assert_eq!(all.len(), 3);

    let active = plans.list(true).await.unwrap();
    assert_eq!(active.len(), 2); // predict is a roadmap placeholder

    let basic = plans.find_by_code(Tier::Basic).await.unwrap().unwrap();
    assert_eq!(basic.price_per_machine_year.amount_minor, 150_000);
    assert_eq!(basic.onboarding_fee.amount_minor, 60_000);
    assert!(basic.allows("static_passport"));
    assert!(!basic.allows("live_data"));
}

#[sqlx::test(migrations = "../../migrations")]
async fn subscription_round_trip(pool: MySqlPool) {
    let orgs = MySqlOrganizationRepo::new(pool.clone());
    let subs = MySqlSubscriptionRepo::new(pool.clone());

    let o = org("Subbed");
    orgs.create(&o).await.unwrap();

    let now = Utc::now();
    let sub = Subscription {
        id: SubscriptionId::new(),
        organization_id: o.id,
        status: SubscriptionStatus::Trialing,
        trial_ends_at: Some(now),
        current_period_start: Some(now),
        current_period_end: Some(now),
        created_at: now,
        updated_at: now,
    };
    subs.create(&sub).await.unwrap();

    let found = subs.find_by_org(o.id).await.unwrap().unwrap();
    assert_eq!(found.status, SubscriptionStatus::Trialing);

    let mut active = found;
    active.status = SubscriptionStatus::Active;
    subs.update(&active).await.unwrap();
    assert_eq!(
        subs.find_by_org(o.id).await.unwrap().unwrap().status,
        SubscriptionStatus::Active
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn machine_tier_assignment(pool: MySqlPool) {
    let orgs = MySqlOrganizationRepo::new(pool.clone());
    let machines = MySqlMachineRepo::new(pool.clone());
    let plans = MySqlPlanRepo::new(pool.clone());

    let o = org("Plant");
    orgs.create(&o).await.unwrap();
    let mut m = machine(o.id, "CNC-1");
    machines.create(&m).await.unwrap();

    let basic = plans.find_by_code(Tier::Basic).await.unwrap().unwrap();
    m.plan_id = Some(basic.id);
    m.updated_at = Utc::now();
    machines.update(&m).await.unwrap();

    let reloaded = machines.find_by_id(o.id, m.id).await.unwrap();
    assert_eq!(reloaded.plan_id, Some(basic.id));

    let (page, total) = machines.list(o.id, 10, 0).await.unwrap();
    assert_eq!(total, 1);
    assert_eq!(page.len(), 1);
}

#[sqlx::test(migrations = "../../migrations")]
async fn document_version_bump_and_photo_primary(pool: MySqlPool) {
    let orgs = MySqlOrganizationRepo::new(pool.clone());
    let machines = MySqlMachineRepo::new(pool.clone());
    let docs = MySqlDocumentRepo::new(pool.clone());

    let o = org("Doc Org");
    orgs.create(&o).await.unwrap();
    let m = machine(o.id, "Press");
    machines.create(&m).await.unwrap();

    // a photo slot (file kind)
    let slot = Document {
        id: DocumentId::new(),
        machine_id: m.id,
        category: DocumentCategory::Photo,
        name: "Front".to_string(),
        storage_kind: StorageKind::File,
        current_version_no: 0,
        created_by: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };
    docs.create(&slot).await.unwrap();

    let mk_input = |key: &str, photo_for: Option<MachineId>| NewVersionInput {
        id: VersionId::new(),
        storage_key: Some(key.to_string()),
        original_filename: Some("front.jpg".to_string()),
        mime_type: Some("image/jpeg".to_string()),
        size_bytes: Some(123),
        checksum_sha256: Some("a".repeat(64)),
        content_json: None,
        change_note: Some("note".to_string()),
        metadata: None,
        created_by: None,
        primary_photo_for: photo_for,
    };

    let v1 = docs
        .add_version(o.id, slot.id, mk_input("k1", Some(m.id)))
        .await
        .unwrap();
    assert_eq!(v1.version_no, 1);
    assert!(v1.is_current);

    let v2 = docs
        .add_version(o.id, slot.id, mk_input("k2", Some(m.id)))
        .await
        .unwrap();
    assert_eq!(v2.version_no, 2);

    // history newest-first, slot counter bumped, exactly one current
    let history = docs.list_versions(o.id, slot.id).await.unwrap();
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].version_no, 2);
    assert_eq!(
        docs.find_by_id(o.id, slot.id)
            .await
            .unwrap()
            .current_version_no,
        2
    );
    let current = docs.current_version(o.id, slot.id).await.unwrap().unwrap();
    assert_eq!(current.version_no, 2);

    // older version demoted
    assert!(
        !docs
            .find_version(o.id, slot.id, 1)
            .await
            .unwrap()
            .is_current
    );

    // photo repointed the machine's primary pointer to the latest version
    let reloaded = machines.find_by_id(o.id, m.id).await.unwrap();
    assert_eq!(reloaded.primary_photo_version_id, Some(v2.id));

    // a foreign org cannot see the document or add versions to it
    let other = OrgId::new();
    assert!(docs.find_by_id(other, slot.id).await.is_err());
    assert!(
        docs.add_version(other, slot.id, mk_input("k3", None))
            .await
            .is_err()
    );
}
