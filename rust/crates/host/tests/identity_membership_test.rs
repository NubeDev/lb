//! Global identity + membership at the host layer: the identity/membership admin verbs over the MCP
//! surface, the mandatory per-verb capability-deny and two-workspace isolation tests, plus the
//! scope-specific cases (login/zero-memberships, leave-is-a-clean-exit, migration, the create_workspace
//! first-member bootstrap). Mirrors `authz_test.rs`'s shape: a `principal()` token factory.

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_authz::{grant_list, membership_is_member, Subject};
use lb_host::{
    call_identity_tool, call_membership_tool, identity_list, identity_workspaces, membership_add,
    membership_list, membership_login_resolve, membership_remove,
};
use lb_mcp::ToolError;
use lb_store::Store;
use serde_json::json;

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
    let token = mint(&key, &claims);
    verify(&key, &token, 1).expect("token verifies")
}

const MANAGE: &[&str] = &["mcp:identity.manage:call", "mcp:members.manage:call"];

// ── Mandatory: capability deny, per verb, over the real MCP bridge ────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn denies_each_identity_membership_verb_without_its_grant() {
    let store = Store::memory().await.unwrap();
    // Holds NEITHER manage cap — every identity/membership verb is denied.
    let p = principal("user:mallory", "acme", &["mcp:workspace.list:call"]);
    for (bridge, verb, input) in [
        (
            "identity",
            "identity.create",
            json!({ "sub": "user:x", "ts": 1 }),
        ),
        ("identity", "identity.get", json!({ "sub": "user:x" })),
        ("identity", "identity.list", json!({})),
        (
            "identity",
            "identity.workspaces",
            json!({ "sub": "user:x" }),
        ),
        (
            "membership",
            "membership.add",
            json!({ "sub": "user:x", "ts": 1 }),
        ),
        (
            "membership",
            "membership.remove",
            json!({ "sub": "user:x" }),
        ),
        ("membership", "membership.list", json!({})),
    ] {
        let err = if bridge == "identity" {
            call_identity_tool(&store, &p, "acme", verb, &input)
                .await
                .unwrap_err()
        } else {
            call_membership_tool(&store, &p, "acme", verb, &input)
                .await
                .unwrap_err()
        };
        assert!(matches!(err, ToolError::Denied), "{verb} must be denied");
    }
}

// ── Mandatory: two-workspace isolation ────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn ws_b_admin_cannot_see_or_touch_ws_a_membership() {
    let store = Store::memory().await.unwrap();
    let admin_a = principal("user:alice", "acme", MANAGE);
    let admin_b = principal("user:carol", "globex", MANAGE);

    // ws-A admin adds a member to acme.
    membership_add(&store, &admin_a, "acme", "user:bob", 10)
        .await
        .unwrap();

    // ws-B admin's membership.list shows only globex (empty) — never acme's roster.
    let seen = membership_list(&store, &admin_b, "globex").await.unwrap();
    assert!(seen.is_empty(), "ws-B must not see ws-A's members");

    // ws-B admin cannot add/remove in acme — forged cross-workspace call denied at the bridge (ws
    // comes from the token, not the body).
    for (verb, input) in [
        ("membership.add", json!({ "sub": "user:eve", "ts": 1 })),
        ("membership.remove", json!({ "sub": "user:bob" })),
    ] {
        let err = call_membership_tool(&store, &admin_b, "acme", verb, &input)
            .await
            .unwrap_err();
        assert!(
            matches!(err, ToolError::Denied),
            "ws-B → ws-A {verb} must be denied"
        );
    }
    // bob is still a member of acme only.
    assert!(membership_is_member(&store, "acme", "user:bob")
        .await
        .unwrap());
    assert!(!membership_is_member(&store, "globex", "user:bob")
        .await
        .unwrap());

    // identity.workspaces(bob) from ws-B's session resolves only ws-B's membership (bob is not in
    // globex → empty), never acme's. The scan is workspace-namespaced; the wall holds.
    let wss = identity_workspaces(&store, &admin_b, "user:bob")
        .await
        .unwrap();
    assert!(wss.iter().all(|w| w.ws != "acme"), "acme must not leak");
}

// ── identity ↔ membership correctness: one identity in N workspaces ───────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn one_identity_in_n_workspaces_resolves_n_memberships() {
    let store = Store::memory().await.unwrap();
    // Register two workspaces in the node directory so the scan can find them.
    lb_authz::identity_create(&store, "user:ada", None, 0)
        .await
        .unwrap();
    let admin_pilot = principal("user:root", "pilot", MANAGE);
    let admin_globex = principal("user:root", "globex", MANAGE);
    seed_directory(&store, "pilot").await;
    seed_directory(&store, "globex").await;

    membership_add(&store, &admin_pilot, "pilot", "user:ada", 1)
        .await
        .unwrap();
    membership_add(&store, &admin_globex, "globex", "user:ada", 2)
        .await
        .unwrap();

    let wss = identity_workspaces(&store, &admin_pilot, "user:ada")
        .await
        .unwrap();
    let ids: Vec<&str> = wss.iter().map(|w| w.ws.as_str()).collect();
    assert_eq!(ids, vec!["globex", "pilot"], "ada is a member of both");
}

// ── login / zero-memberships (decision #4) ───────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn login_refuses_a_non_member_of_a_workspace_that_has_members() {
    let store = Store::memory().await.unwrap();
    let admin = principal("user:alice", "acme", MANAGE);
    // acme now has a member (alice bootstraps via membership_login_resolve on empty).
    membership_login_resolve(&store, "acme", "user:alice", 1)
        .await
        .unwrap();
    // A provisioned identity with zero memberships cannot enter acme.
    lb_authz::identity_create(&store, "user:eve", None, 0)
        .await
        .unwrap();
    let err = membership_login_resolve(&store, "acme", "user:eve", 2)
        .await
        .unwrap_err();
    assert!(matches!(err, lb_host::MembershipError::Denied));
    // But the empty-workspace bootstrap still works (decision #3).
    membership_login_resolve(&store, "brandnew", "user:eve", 3)
        .await
        .unwrap();
    assert!(membership_is_member(&store, "brandnew", "user:eve")
        .await
        .unwrap());
    // identity.workspaces(eve) shows brandnew only — eve never got into acme.
    seed_directory(&store, "acme").await;
    seed_directory(&store, "brandnew").await;
    let admin_b = principal("user:alice", "brandnew", MANAGE);
    let wss = identity_workspaces(&store, &admin_b, "user:eve")
        .await
        .unwrap();
    assert!(wss.iter().any(|w| w.ws == "brandnew"));
    assert!(wss.iter().all(|w| w.ws != "acme"));
    let _ = admin; // admin used for the un-gated resolve's type; resolve is un-gated
}

// ── leave is a clean exit: live token refused after remove ────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn membership_remove_revokes_grants_and_marks_token() {
    let store = Store::memory().await.unwrap();
    let admin = principal("user:alice", "acme", MANAGE);
    membership_add(&store, &admin, "acme", "user:bob", 1)
        .await
        .unwrap();
    // join granted role:member.
    let caps = grant_list(&store, "acme", &Subject::User("bob".into()))
        .await
        .unwrap();
    assert!(caps.iter().any(|c| c == "role:member"));

    let revoked = membership_remove(&store, &admin, "acme", "user:bob")
        .await
        .unwrap();
    assert!(revoked >= 1, "role:member grant was revoked");
    // membership row is tombstoned.
    assert!(!membership_is_member(&store, "acme", "user:bob")
        .await
        .unwrap());
    // grants are tombstoned.
    let caps = grant_list(&store, "acme", &Subject::User("bob".into()))
        .await
        .unwrap();
    assert!(!caps.iter().any(|c| c == "role:member"));
    // live-token marker is set → the verify path refuses bob's current token.
    assert!(
        lb_authz::token_revoked(&store, "acme", &Subject::User("bob".into()))
            .await
            .unwrap()
    );
}

// ── migration: legacy user rows → no access gained or lost (decision #10) ─────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn legacy_user_rows_are_implicit_memberships_no_access_change() {
    let store = Store::memory().await.unwrap();
    let admin = principal("user:alice", "acme", MANAGE);
    // Seed a legacy workspace-scoped user row directly (a row from BEFORE this slice, the shape
    // `user_create` writes) — the migration must treat it as an implicit membership.
    let legacy = serde_json::json!({
        "user": "bob", "active": true, "role": "member", "cred_ref": "dev", "kind": "user", "ts": 5,
    });
    lb_store::write(&store, "acme", "user", "bob", &legacy)
        .await
        .unwrap();
    // legacy bob's existing grant is unchanged.
    lb_authz::grant_assign(
        &store,
        "acme",
        &Subject::User("bob".into()),
        "mcp:hvac.setpoint:call",
    )
    .await
    .unwrap();

    // membership.list includes bob (the implicit/legacy member) — no access gained or lost.
    let members = membership_list(&store, &admin, "acme").await.unwrap();
    assert!(members.iter().any(|m| m.sub == "user:bob"));
    // bob's grant is intact (migration does not touch grants).
    let caps = grant_list(&store, "acme", &Subject::User("bob".into()))
        .await
        .unwrap();
    assert!(caps.iter().any(|c| c == "mcp:hvac.setpoint:call"));
    // identity was lazy-created on first resolution.
    let identities = identity_list(&store, &admin).await.unwrap();
    assert!(identities.iter().any(|i| i.sub == "user:bob"));
}

// ── offline / sync: a removed membership is not resurrected by a stale edge ───────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn removed_membership_tombstone_replays_idempotently() {
    let store = Store::memory().await.unwrap();
    let admin = principal("user:alice", "acme", MANAGE);
    membership_add(&store, &admin, "acme", "user:bob", 1)
        .await
        .unwrap();
    membership_remove(&store, &admin, "acme", "user:bob")
        .await
        .unwrap();
    // A stale synced edge re-applies the SAME remove tombstone (sync §6.8) — bob stays removed, not
    // resurrected. Re-applying the raw tombstone is a no-op for membership.
    lb_authz::membership_remove_raw(&store, "acme", "user:bob")
        .await
        .unwrap();
    assert!(!membership_is_member(&store, "acme", "user:bob")
        .await
        .unwrap());
    // And a hub-added membership reaches the read path after "reconnect" (just a fresh read).
    membership_add(&store, &admin, "acme", "user:carol", 2)
        .await
        .unwrap();
    let members = membership_list(&store, &admin, "acme").await.unwrap();
    assert!(members.iter().any(|m| m.sub == "user:carol"));
    assert!(members.iter().all(|m| m.sub != "user:bob"));
}

/// Write a workspace directory entry directly (the scan reads `_lb_workspaces` to enumerate).
async fn seed_directory(store: &Store, ws: &str) {
    let row = serde_json::json!({
        "ws": ws,
        "name": ws,
        "kind": "workspace",
        "status": "active",
        "ts": 0,
    });
    lb_store::write(store, "_lb_workspaces", "workspace", ws, &row)
        .await
        .unwrap();
}
