//! Host-layer tests for the atomic `workspace.provision` + `workspace.reconcile` pair
//! (workspace-provision scope, NubeDev/lb#121). Mandatory categories: **capability-deny with zero
//! residue**, **atomicity / no orphans** (the torn intermediate is never listable), **crash
//! durability across a real on-disk store reopen** (the regression test for the observed `nube`
//! orphan), plus admin-other-than-caller, idempotency vs a revoked role, and tombstone-no-resurrect.
//! Real store (`mem://` and SurrealKV on disk), no mocks — rule 9.

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_authz::{grant_list, grant_revoke, membership_is_member, membership_list, Subject};
use lb_host::{
    login_workspaces, workspace_create, workspace_list, workspace_provision, workspace_purge,
    workspace_reconcile, workspace_register, Node, WorkspacesError,
};
use lb_store::Store;

fn principal(sub: &str, ws: &str, caps: &[&str]) -> Principal {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: sub.into(),
        ws: ws.into(),
        role: Role::Member,
        caps: caps.iter().map(|s| s.to_string()).collect(),
        iat: 0,
        exp: u64::MAX,
        constraint: None,
        run_id: None,
    };
    verify(&key, &mint(&key, &claims), 1).expect("token verifies")
}

const ADMIN: &[&str] = &[
    "mcp:workspace.provision:call",
    "mcp:workspace.reconcile:call",
    "mcp:workspace.create:call",
    "mcp:workspace.list:call",
    "mcp:workspace.purge:call",
    "mcp:workspace.delete:call",
];

fn temp_path(tag: &str) -> String {
    std::env::temp_dir()
        .join(format!("lb-provision-{tag}-{}", std::process::id()))
        .to_string_lossy()
        .into_owned()
}

fn cleanup(path: &str) {
    let _ = std::fs::remove_dir_all(path);
}

// ── Capability deny: refused AND zero residue ─────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn provision_and_reconcile_denied_without_cap_leave_zero_residue() {
    let node = Node::boot().await.expect("node boots");
    let member = principal("user:bob", "acme", &["mcp:workspace.list:call"]);

    let err = workspace_provision(&node.store, &member, "nube", "Nube iO", None, None, 1)
        .await
        .expect_err("member must be denied");
    assert!(matches!(err, WorkspacesError::Denied));
    assert!(workspace_reconcile(&node.store, &member, "nube", None, 1)
        .await
        .is_err());

    // Zero residue: no directory row, no membership row.
    let listed = workspace_list(&node.store, &member).await.unwrap();
    assert!(!listed.iter().any(|w| w.ws == "nube"));
    assert!(!membership_is_member(&node.store, "nube", "user:bob")
        .await
        .unwrap());
}

// ── Atomicity: the torn intermediate is never a listable orphan ───────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn torn_provision_is_never_listable_and_retry_completes_it() {
    let node = Node::boot().await.expect("node boots");
    let ada = principal("user:ada", "acme", ADMIN);

    // Hand-craft the exact state a crash between the bootstrap batch and the directory write
    // leaves: membership present, directory row absent (the directory row is written LAST).
    lb_authz::membership_add_raw(&node.store, "nube", "user:ada", 1)
        .await
        .unwrap();
    let listed = workspace_list(&node.store, &ada).await.unwrap();
    assert!(
        !listed.iter().any(|w| w.ws == "nube"),
        "a torn provision must be absent from the directory, not a listable orphan"
    );
    let roster = login_workspaces(&node.store, "user:ada").await.unwrap();
    assert!(
        !roster.iter().any(|w| w.ws == "nube"),
        "a torn provision must not appear in the creator's login roster"
    );

    // Retrying the SAME provision (same admin) is legitimate and completes it.
    let report = workspace_provision(&node.store, &ada, "nube", "Nube iO", None, None, 2)
        .await
        .expect("retry completes the torn provision");
    assert_eq!(report.admin_sub, "user:ada");
    let roster = login_workspaces(&node.store, "user:ada").await.unwrap();
    assert!(roster.iter().any(|w| w.ws == "nube"), "now enterable");

    // But a namespace populated by SOMEONE ELSE is not provisionable — that would grant admin
    // into a populated workspace.
    lb_authz::membership_add_raw(&node.store, "other", "user:eve", 1)
        .await
        .unwrap();
    assert!(matches!(
        workspace_provision(&node.store, &ada, "other", "Other", None, None, 2).await,
        Err(WorkspacesError::Denied)
    ));
}

// ── Crash durability: provision on disk, drop WITHOUT clean shutdown, reboot ──────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn provision_survives_unclean_restart_listable_and_enterable() {
    let path = temp_path("crash");
    cleanup(&path);
    {
        let store = Store::open(&path).await.unwrap();
        let ada = principal("user:ada", "acme", ADMIN);
        workspace_provision(&store, &ada, "nube", "Nube iO", None, None, 1)
            .await
            .expect("provisions");
        // No clean shutdown: the store is simply dropped here (the observed-orphan scenario).
    }
    let store = Store::open(&path).await.unwrap();
    let ada = principal("user:ada", "acme", ADMIN);
    let listed = workspace_list(&store, &ada).await.unwrap();
    assert!(
        listed.iter().any(|w| w.ws == "nube"),
        "the provisioned workspace must still be LISTABLE after an unclean restart"
    );
    let roster = login_workspaces(&store, "user:ada").await.unwrap();
    assert!(
        roster.iter().any(|w| w.ws == "nube"),
        "the provisioned workspace must still be ENTERABLE (in the login roster) after restart"
    );
    cleanup(&path);
}

// ── The old orphan (directory row, no membership) is repairable via reconcile ─────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn reconcile_repairs_a_memberless_orphan_but_never_a_populated_workspace() {
    let node = Node::boot().await.expect("node boots");
    let ada = principal("user:ada", "acme", ADMIN);

    // Hand-craft the old path's orphan: directory row present, membership empty.
    workspace_register(&node.store, "nube", "Nube iO", 1)
        .await
        .unwrap();
    // The symptom: the workspace is listable but in nobody's roster ("not a member of that
    // workspace" on every switch attempt).
    let roster = login_workspaces(&node.store, "user:ada").await.unwrap();
    assert!(!roster.iter().any(|w| w.ws == "nube"));

    let report = workspace_reconcile(&node.store, &ada, "nube", None, 2)
        .await
        .expect("reconcile repairs the orphan");
    assert_eq!(report.admin_sub, "user:ada");
    assert!(report.fixed.contains(&"membership".to_string()));
    let roster = login_workspaces(&node.store, "user:ada").await.unwrap();
    assert!(
        roster.iter().any(|w| w.ws == "nube"),
        "after reconcile the switch succeeds (workspace is in the roster)"
    );

    // Strictly limited to memberless workspaces: a populated one is refused — reconcile is never
    // a way to add yourself to a workspace that has members.
    let eve = principal("user:eve", "evil", ADMIN);
    assert!(matches!(
        workspace_reconcile(&node.store, &eve, "nube", None, 3).await,
        Err(WorkspacesError::Denied)
    ));
    assert!(!membership_is_member(&node.store, "nube", "user:eve")
        .await
        .unwrap());
}

// ── Admin other than the caller ───────────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn provision_for_another_admin_does_not_membership_the_caller() {
    let node = Node::boot().await.expect("node boots");
    let ada = principal("user:ada", "acme", ADMIN);

    let report = workspace_provision(
        &node.store,
        &ada,
        "nube",
        "Nube iO",
        Some("user:alice"),
        None,
        1,
    )
    .await
    .expect("provisions for alice");
    assert_eq!(report.admin_sub, "user:alice");
    assert_eq!(
        report.roles_granted,
        vec![
            "role:member".to_string(),
            "role:workspace-admin".to_string()
        ]
    );
    assert!(!report.skills_granted.is_empty());

    assert!(membership_is_member(&node.store, "nube", "user:alice")
        .await
        .unwrap());
    let caps = grant_list(&node.store, "nube", &Subject::User("alice".into()))
        .await
        .unwrap();
    assert!(caps.contains(&"role:workspace-admin".to_string()));
    // The caller is NOT silently a member, and her principal still names acme.
    assert!(!membership_is_member(&node.store, "nube", "user:ada")
        .await
        .unwrap());
    assert_eq!(ada.ws(), "acme");
}

// ── Idempotency + tombstone ───────────────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn reprovision_is_a_noop_that_does_not_regrant_a_revoked_role() {
    let node = Node::boot().await.expect("node boots");
    let ada = principal("user:ada", "acme", ADMIN);

    workspace_provision(&node.store, &ada, "nube", "Nube iO", None, None, 1)
        .await
        .unwrap();
    grant_revoke(
        &node.store,
        "nube",
        &Subject::User("ada".into()),
        "role:workspace-admin",
    )
    .await
    .unwrap();

    let report = workspace_provision(&node.store, &ada, "nube", "Nube iO", None, None, 2)
        .await
        .expect("re-provision is an idempotent no-op");
    assert!(report.roles_granted.is_empty());
    assert!(report.skills_granted.is_empty());
    let caps = grant_list(&node.store, "nube", &Subject::User("ada".into()))
        .await
        .unwrap();
    assert!(
        !caps.contains(&"role:workspace-admin".to_string()),
        "a revoked admin role must NOT be re-granted by a re-provision"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn provision_never_resurrects_a_purged_tombstone() {
    let node = Node::boot().await.expect("node boots");
    let ada = principal("user:ada", "acme", ADMIN);

    workspace_create(&node.store, &ada, "pilot", "Pilot", 1)
        .await
        .unwrap();
    workspace_purge(&node.store, &ada, "pilot", "pilot")
        .await
        .unwrap();

    assert!(matches!(
        workspace_provision(&node.store, &ada, "pilot", "Pilot", None, None, 2).await,
        Err(WorkspacesError::Purged)
    ));
    assert!(matches!(
        workspace_reconcile(&node.store, &ada, "pilot", None, 2).await,
        Err(WorkspacesError::Purged)
    ));
    let listed = workspace_list(&node.store, &ada).await.unwrap();
    assert!(!listed.iter().any(|w| w.ws == "pilot"));
}

// ── create.rs is now a thin delegation and inherits atomicity ─────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn workspace_create_still_bootstraps_the_caller_as_admin() {
    let node = Node::boot().await.expect("node boots");
    let ada = principal("user:ada", "acme", ADMIN);

    let record = workspace_create(&node.store, &ada, "pilot", "Pilot", 1)
        .await
        .unwrap();
    assert_eq!(record.ws, "pilot");
    assert!(membership_is_member(&node.store, "pilot", "user:ada")
        .await
        .unwrap());
    assert_eq!(
        membership_list(&node.store, "pilot").await.unwrap().len(),
        1
    );
    let roster = login_workspaces(&node.store, "user:ada").await.unwrap();
    assert!(roster.iter().any(|w| w.ws == "pilot"));
}
