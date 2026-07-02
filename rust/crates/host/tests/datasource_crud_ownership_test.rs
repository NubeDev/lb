//! Datasource CRUD ownership regression (datasources scope). The bug: `datasource.add` stored the
//! DSN secret owned by the (varying) admin caller, so a SECOND admin — a different dev login, the
//! boot seed's bootstrap principal, a future IdP user — overwriting or removing the same source hit
//! the secrets owner wall (gate 3) and got an opaque `denied`. The UI Test/Save then failed for a
//! reason no message revealed.
//!
//! The fix: every federation DSN secret is owned by the STABLE `ext:federation` principal (via
//! `secret::store_dsn`/`reclaim`), decoupled from whoever runs the verb — so any admin may CRUD a
//! source and the owner never collides. These tests prove it end to end through the PUBLIC verbs,
//! with real embedded SurrealDB (no Postgres/sidecar needed: `add`/`remove` + secret mediation never
//! touch the child). Mandatory categories covered: the CRUD-across-two-admins happy path, the
//! stale-owner heal, workspace isolation, and the DSN-never-in-a-record redaction.

use std::sync::Arc;

use lb_assets::{record_install, Install};
use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{datasource_add, datasource_list, datasource_remove, Node};

const FED: &str = "federation";

fn admin_named(sub: &str, ws: &str) -> Principal {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: sub.into(),
        ws: ws.into(),
        role: Role::Member,
        caps: vec![
            "mcp:datasource.add:call".into(),
            "mcp:datasource.remove:call".into(),
            "mcp:datasource.list:call".into(),
            "secret:federation/*:write".into(),
            "secret:federation/*:get".into(),
        ],
        iat: 0,
        exp: u64::MAX,
    };
    verify(&key, &mint(&key, &claims), 1).unwrap()
}

/// Persist the federation install record so `store_dsn`/`mediate_dsn` find the `ext:federation`
/// grant (`secret:federation/*:get`; the write authority the host adds itself). No sidecar spawned.
async fn install_federation_record(node: &Node, ws: &str) {
    let mut install = Install::new(
        FED,
        "0.1.0",
        vec![
            "net:tls:*:*:connect".into(),
            "secret:federation/*:get".into(),
        ],
        1,
    );
    install.tier = lb_assets::Tier::Native;
    record_install(&node.store, ws, &install).await.unwrap();
}

/// The DSN read back from `lb-secrets` as the mediator principal (what the pool would receive). A
/// `Workspace`-visible secret owned by `ext:federation` reads for the mediator regardless of who
/// wrote it — the invariant the fix guarantees.
async fn stored_dsn(node: &Node, ws: &str, name: &str) -> Result<String, lb_secrets::SecretsError> {
    let mediator = Principal::routed(
        format!("ext:{FED}"),
        ws.to_string(),
        vec!["secret:federation/*:get".into()],
    );
    lb_secrets::get(&node.store, &mediator, ws, &format!("federation/{name}")).await
}

/// The headline regression: admin A registers a source with a DSN; admin B (a different subject)
/// updates the SAME source with a new DSN; admin A removes it. None is denied, and the mediated DSN
/// tracks the last write — exactly the UI's add → edit → delete by whichever admin is logged in.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn two_admins_crud_one_source_without_owner_wall_denial() {
    let node = Arc::new(Node::boot().await.unwrap());
    let ws = "acme";
    install_federation_record(&node, ws).await;

    let ada = admin_named("user:ada", ws);
    let bob = admin_named("user:bob", ws);

    // A adds it.
    datasource_add(
        &node,
        &ada,
        ws,
        "timescale",
        "postgres",
        "127.0.0.1:5433",
        None,
        Some("postgres://a@127.0.0.1:5433/db"),
        1,
    )
    .await
    .expect("admin A add");
    assert_eq!(
        stored_dsn(&node, ws, "timescale").await.unwrap(),
        "postgres://a@127.0.0.1:5433/db"
    );

    // B updates it — this is the path that used to hit the owner wall and return `denied`.
    datasource_add(
        &node,
        &bob,
        ws,
        "timescale",
        "postgres",
        "127.0.0.1:5433",
        None,
        Some("postgres://b@127.0.0.1:5433/db"),
        2,
    )
    .await
    .expect("admin B update must NOT be denied");
    assert_eq!(
        stored_dsn(&node, ws, "timescale").await.unwrap(),
        "postgres://b@127.0.0.1:5433/db"
    );

    // A removes it — forgets the secret (owned by ext:federation, so the delete passes the wall).
    datasource_remove(&node, &ada, ws, "timescale", 3)
        .await
        .expect("admin A remove");
    assert!(matches!(
        stored_dsn(&node, ws, "timescale").await.unwrap_err(),
        lb_secrets::SecretsError::NotFound
    ));

    // Re-add after remove: a clean lifecycle, no stale-secret collision.
    datasource_add(
        &node,
        &bob,
        ws,
        "timescale",
        "postgres",
        "127.0.0.1:5433",
        None,
        Some("postgres://c@127.0.0.1:5433/db"),
        4,
    )
    .await
    .expect("re-add after remove");
    assert_eq!(
        stored_dsn(&node, ws, "timescale").await.unwrap(),
        "postgres://c@127.0.0.1:5433/db"
    );
}

/// A secret left by an EARLIER bootstrap principal (a store seeded before the single-owner invariant)
/// is healed: the next `add` reclaims ownership, and CRUD proceeds without denial.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn heals_a_secret_owned_by_the_boot_bootstrap_principal() {
    let node = Arc::new(Node::boot().await.unwrap());
    let ws = "acme";
    install_federation_record(&node, ws).await;

    // Simulate the poisoned store: the DSN owned by `ext:federation-bootstrap` (the old seed sub).
    let bootstrap = Principal::routed(
        "ext:federation-bootstrap".to_string(),
        ws.to_string(),
        vec![
            "secret:federation/*:write".into(),
            "secret:federation/*:get".into(),
        ],
    );
    lb_secrets::set_with(
        &node.store,
        &bootstrap,
        ws,
        "federation/timescale",
        "postgres://stale",
        lb_secrets::Visibility::Workspace,
    )
    .await
    .unwrap();

    // A normal admin updates the source — must succeed (reclaim heals the owner drift).
    let ada = admin_named("user:ada", ws);
    datasource_add(
        &node,
        &ada,
        ws,
        "timescale",
        "postgres",
        "127.0.0.1:5433",
        None,
        Some("postgres://fresh@127.0.0.1:5433/db"),
        5,
    )
    .await
    .expect("healing update must NOT be denied");
    assert_eq!(
        stored_dsn(&node, ws, "timescale").await.unwrap(),
        "postgres://fresh@127.0.0.1:5433/db"
    );
}

/// Workspace isolation (mandatory): a source added in `acme` is invisible in `beta`, and the DSN
/// secret never crosses the wall.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn crud_is_workspace_isolated() {
    let node = Arc::new(Node::boot().await.unwrap());
    install_federation_record(&node, "acme").await;
    install_federation_record(&node, "beta").await;

    let ada = admin_named("user:ada", "acme");
    datasource_add(
        &node,
        &ada,
        "acme",
        "timescale",
        "postgres",
        "127.0.0.1:5433",
        None,
        Some("postgres://secret@127.0.0.1:5433/db"),
        1,
    )
    .await
    .unwrap();

    // Not visible in beta.
    let beta_admin = admin_named("user:zoe", "beta");
    let beta_list = datasource_list(&node, &beta_admin, "beta").await.unwrap();
    assert!(beta_list.is_empty(), "beta must not see acme's source");

    // The DSN secret is not readable in beta.
    assert!(stored_dsn(&node, "beta", "timescale").await.is_err());
}

/// Redaction (mandatory): the DSN never appears in the `datasource.list` output — only a secret ref.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn list_never_leaks_the_dsn() {
    let node = Arc::new(Node::boot().await.unwrap());
    let ws = "acme";
    install_federation_record(&node, ws).await;

    let ada = admin_named("user:ada", ws);
    let dsn = "postgres://topsecret@127.0.0.1:5433/db";
    datasource_add(
        &node,
        &ada,
        ws,
        "timescale",
        "postgres",
        "127.0.0.1:5433",
        None,
        Some(dsn),
        1,
    )
    .await
    .unwrap();

    let list = datasource_list(&node, &ada, ws).await.unwrap();
    let json = serde_json::to_string(&list).unwrap();
    assert!(
        !json.contains("topsecret"),
        "DSN must never appear in list output: {json}"
    );
}
