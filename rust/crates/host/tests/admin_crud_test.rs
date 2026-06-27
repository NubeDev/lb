//! Admin-CRUD at the host layer: the destructive workspace/user/team/member verbs + the user
//! lifecycle, with the mandatory capability-deny and two-workspace isolation tests, plus the slice
//! cases — disable-bites-login, soft-before-hard (+ confirm token), teams.delete cascade + revoke,
//! idempotency, and tombstone-not-resurrected (admin-crud scope).

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{
    add_team_member, grants_assign, list_members, remove_member, resolve_caps, teams_create,
    teams_delete, user_create, user_delete, user_disable, user_enable, user_list, user_login_check,
    workspace_create, workspace_delete, workspace_list, workspace_purge, workspace_rename, Subject,
    UsersError,
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
    };
    verify(&key, &mint(&key, &claims), 1).expect("token verifies")
}

const ADMIN: &[&str] = &[
    "mcp:workspace.delete:call",
    "mcp:workspace.purge:call",
    "mcp:workspace.create:call",
    "mcp:workspace.list:call",
    "mcp:user.manage:call",
    "mcp:user.disable:call",
    "mcp:teams.manage:call",
    "mcp:teams.list:call",
    "mcp:members.add:call",
    "mcp:grants.assign:call",
    // example tool caps the admin holds, so granting them satisfies the no-widening rule (slice 1).
    "mcp:x.y:call",
    "mcp:hvac.setpoint:call",
];

// ── Mandatory: capability deny, per destructive verb ──────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn denies_destructive_verbs_without_their_cap() {
    let store = Store::memory().await.unwrap();
    let none = principal("user:mallory", "acme", &[]); // holds nothing

    assert!(workspace_delete(&store, &none, "acme").await.is_err());
    assert!(workspace_rename(&store, &none, "acme", "x", 1)
        .await
        .is_err());
    assert!(user_disable(&store, &none, "acme", "bob").await.is_err());
    assert!(user_delete(&store, &none, "acme", "bob").await.is_err());
    assert!(teams_delete(&store, &none, "acme", "facilities")
        .await
        .is_err());
    assert!(remove_member(&store, &none, "acme", "facilities", "bob")
        .await
        .is_err());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn hard_delete_needs_the_purge_cap_above_the_soft_cap() {
    let store = Store::memory().await.unwrap();
    // Holds only the SOFT delete cap — purge (hard) is refused.
    let soft = principal("user:alice", "acme", &["mcp:workspace.delete:call"]);
    assert!(
        workspace_purge(&store, &soft, "acme", "acme")
            .await
            .is_err(),
        "soft cap must not authorize purge"
    );
    // Even WITH the purge cap, a wrong confirm token is refused.
    let admin = principal("user:alice", "acme", ADMIN);
    assert!(
        workspace_purge(&store, &admin, "acme", "WRONG")
            .await
            .is_err(),
        "purge needs the typed confirm token"
    );
    // With both the purge cap AND the matching confirm token → succeeds.
    workspace_purge(&store, &admin, "acme", "acme")
        .await
        .unwrap();
}

// ── Mandatory: two-workspace isolation ────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn ws_b_admin_cannot_touch_ws_a() {
    let store = Store::memory().await.unwrap();
    let admin_a = principal("user:alice", "acme", ADMIN);
    let admin_b = principal("user:carol", "globex", ADMIN);

    // ws-A seeds a user + team + member.
    user_create(&store, &admin_a, "acme", "bob", "member", "dev", 1)
        .await
        .unwrap();
    teams_create(&store, &admin_a, "acme", "facilities", "Facilities")
        .await
        .unwrap();
    add_team_member(&store, &admin_a, "acme", "facilities", "bob")
        .await
        .unwrap();

    // ws-B admin targeting acme is denied / sees nothing across the verbs.
    assert!(user_list(&store, &admin_b, "acme").await.is_err());
    assert!(user_disable(&store, &admin_b, "acme", "bob").await.is_err());
    assert!(teams_delete(&store, &admin_b, "acme", "facilities")
        .await
        .is_err());
    assert!(remove_member(&store, &admin_b, "acme", "facilities", "bob")
        .await
        .is_err());

    // And ws-A's records are intact — the wall held (bob still a member, still listed).
    let members = list_members(
        &store,
        &principal("user:alice", "acme", &["mcp:members.list:call"]),
        "acme",
        "facilities",
    )
    .await
    .unwrap();
    assert_eq!(members, vec!["bob".to_string()]);
}

// ── Slice cases ───────────────────────────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn disable_bites_login_and_enable_restores_and_list_hides_cred() {
    let store = Store::memory().await.unwrap();
    let admin = principal("user:alice", "acme", ADMIN);
    user_create(&store, &admin, "acme", "bob", "member", "secret-cred", 1)
        .await
        .unwrap();

    // Active user mints.
    user_login_check(&store, "acme", "bob").await.unwrap();
    // Disable → login refuses.
    user_disable(&store, &admin, "acme", "bob").await.unwrap();
    assert!(matches!(
        user_login_check(&store, "acme", "bob").await,
        Err(UsersError::Disabled)
    ));
    // Enable → restored.
    user_enable(&store, &admin, "acme", "bob").await.unwrap();
    user_login_check(&store, "acme", "bob").await.unwrap();

    // user.list never leaks the credential ref (the view has no cred field at all).
    let views = user_list(&store, &admin, "acme").await.unwrap();
    let json = serde_json::to_string(&views).unwrap();
    assert!(!json.contains("secret-cred"), "cred must never be listed");

    // An un-administered user (no record) still mints — auto-seed preserved.
    user_login_check(&store, "acme", "newcomer").await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn delete_user_revokes_grants_and_blocks_login_idempotently() {
    let store = Store::memory().await.unwrap();
    let admin = principal("user:alice", "acme", ADMIN);
    user_create(&store, &admin, "acme", "bob", "member", "dev", 1)
        .await
        .unwrap();
    grants_assign(
        &store,
        &admin,
        "acme",
        &Subject::User("bob".into()),
        "mcp:x.y:call",
    )
    .await
    .unwrap();
    assert_eq!(resolve_caps(&store, "acme", "bob").await.unwrap().len(), 1);

    // Delete revokes the one grant and reports the count; a deleted user can't mint.
    assert_eq!(user_delete(&store, &admin, "acme", "bob").await.unwrap(), 1);
    assert!(resolve_caps(&store, "acme", "bob")
        .await
        .unwrap()
        .is_empty());
    assert!(matches!(
        user_login_check(&store, "acme", "bob").await,
        Err(UsersError::Disabled)
    ));
    // Idempotent: re-deleting is a no-op success (0 grants left).
    assert_eq!(user_delete(&store, &admin, "acme", "bob").await.unwrap(), 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn teams_delete_cascades_members_and_revokes_grants() {
    let store = Store::memory().await.unwrap();
    let admin = principal("user:alice", "acme", ADMIN);
    teams_create(&store, &admin, "acme", "facilities", "Facilities")
        .await
        .unwrap();
    add_team_member(&store, &admin, "acme", "facilities", "bob")
        .await
        .unwrap();
    add_team_member(&store, &admin, "acme", "facilities", "carol")
        .await
        .unwrap();
    grants_assign(
        &store,
        &admin,
        "acme",
        &Subject::Team("facilities".into()),
        "mcp:hvac.setpoint:call",
    )
    .await
    .unwrap();

    // Delete cascades: returns 2 members removed, edges gone, team grant revoked.
    let removed = teams_delete(&store, &admin, "acme", "facilities")
        .await
        .unwrap();
    assert_eq!(removed, 2);
    let members = list_members(
        &store,
        &principal("user:alice", "acme", &["mcp:members.list:call"]),
        "acme",
        "facilities",
    )
    .await
    .unwrap();
    assert!(members.is_empty(), "edges cascade-removed");
    // Bob (no longer a member) inherits nothing from the deleted team.
    assert!(resolve_caps(&store, "acme", "bob")
        .await
        .unwrap()
        .is_empty());

    // Idempotent re-delete.
    assert_eq!(
        teams_delete(&store, &admin, "acme", "facilities")
            .await
            .unwrap(),
        0
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn workspace_soft_then_hard_and_tombstone_not_resurrected() {
    let store = Store::memory().await.unwrap();
    let admin = principal("user:alice", "acme", ADMIN);
    workspace_create(&store, &admin, "pilot", "Pilot", 1)
        .await
        .unwrap();
    // Listed while active.
    assert!(workspace_list(&store, &admin)
        .await
        .unwrap()
        .iter()
        .any(|w| w.ws == "pilot"));
    // Soft archive → hidden from the default list (reversible).
    workspace_delete(&store, &admin, "pilot").await.unwrap();
    assert!(!workspace_list(&store, &admin)
        .await
        .unwrap()
        .iter()
        .any(|w| w.ws == "pilot"));
    // Un-archive via rename → back.
    workspace_rename(&store, &admin, "pilot", "Pilot", 2)
        .await
        .unwrap();
    assert!(workspace_list(&store, &admin)
        .await
        .unwrap()
        .iter()
        .any(|w| w.ws == "pilot"));
    // Hard purge → tombstoned, gone, and a later rename/create cannot resurrect it.
    workspace_purge(&store, &admin, "pilot", "pilot")
        .await
        .unwrap();
    workspace_rename(&store, &admin, "pilot", "Pilot", 3)
        .await
        .unwrap(); // no-op on a tombstone
    workspace_create(&store, &admin, "pilot", "Pilot", 4)
        .await
        .ok(); // create upserts, but list must still treat it as purged via tombstone
    let listed = workspace_list(&store, &admin).await.unwrap();
    assert!(
        !listed.iter().any(|w| w.ws == "pilot"),
        "a purged workspace must not resurrect"
    );
}
