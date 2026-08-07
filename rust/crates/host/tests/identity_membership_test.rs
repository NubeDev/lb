//! Global identity + membership at the host layer: the identity/membership admin verbs over the MCP
//! surface, the mandatory per-verb capability-deny and two-workspace isolation tests, plus the
//! scope-specific cases (login/zero-memberships, leave-is-a-clean-exit, the create_workspace
//! first-member bootstrap). Mirrors `authz_test.rs`'s shape: a `principal()` token factory.

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_authz::{grant_list, membership_is_member, Subject};
use lb_host::{
    call_identity_tool, call_membership_tool, identity_workspaces, login_workspaces,
    membership_add, membership_list, membership_remove,
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
        constraint: None,
        run_id: None,
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
    let p = principal("user:mallory", "nube", &["mcp:workspace.list:call"]);
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
            call_identity_tool(&store, &p, "nube", verb, &input)
                .await
                .unwrap_err()
        } else {
            call_membership_tool(&store, &p, "nube", verb, &input)
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
    let admin_a = principal("user:alice", "nube", MANAGE);
    let admin_b = principal("user:carol", "globex", MANAGE);

    // ws-A admin adds a member to nube.
    membership_add(&store, &admin_a, "nube", "user:bob", 10)
        .await
        .unwrap();

    // ws-B admin's membership.list shows only globex (empty) — never nube's roster.
    let seen = membership_list(&store, &admin_b, "globex").await.unwrap();
    assert!(seen.is_empty(), "ws-B must not see ws-A's members");

    // ws-B admin cannot add/remove in nube — forged cross-workspace call denied at the bridge (ws
    // comes from the token, not the body).
    for (verb, input) in [
        ("membership.add", json!({ "sub": "user:eve", "ts": 1 })),
        ("membership.remove", json!({ "sub": "user:bob" })),
    ] {
        let err = call_membership_tool(&store, &admin_b, "nube", verb, &input)
            .await
            .unwrap_err();
        assert!(
            matches!(err, ToolError::Denied),
            "ws-B → ws-A {verb} must be denied"
        );
    }
    // bob is still a member of nube only.
    assert!(membership_is_member(&store, "nube", "user:bob")
        .await
        .unwrap());
    assert!(!membership_is_member(&store, "globex", "user:bob")
        .await
        .unwrap());

    // identity.workspaces(bob) from ws-B's session resolves only ws-B's membership (bob is not in
    // globex → empty), never nube's. The scan is workspace-namespaced; the wall holds.
    let wss = identity_workspaces(&store, &admin_b, "user:bob")
        .await
        .unwrap();
    assert!(wss.iter().all(|w| w.ws != "nube"), "nube must not leak");
}

// ── identity ↔ membership correctness: one identity in N workspaces ───────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn one_identity_in_n_workspaces_resolves_n_memberships() {
    let store = Store::memory().await.unwrap();
    // Register two workspaces in the node directory so the scan can find them.
    lb_authz::identity_create(&store, "user:test", None, 0)
        .await
        .unwrap();
    let admin_pilot = principal("user:root", "pilot", MANAGE);
    let admin_globex = principal("user:root", "globex", MANAGE);
    seed_directory(&store, "pilot").await;
    seed_directory(&store, "globex").await;

    membership_add(&store, &admin_pilot, "pilot", "user:test", 1)
        .await
        .unwrap();
    membership_add(&store, &admin_globex, "globex", "user:test", 2)
        .await
        .unwrap();

    let wss = identity_workspaces(&store, &admin_pilot, "user:test")
        .await
        .unwrap();
    let ids: Vec<&str> = wss.iter().map(|w| w.ws.as_str()).collect();
    assert_eq!(ids, vec!["globex", "pilot"], "test is a member of both");
}

// ── one source of truth: the roster and the login path agree ─────────────────────────────────

/// The invariant the legacy `user:*` union violated (and the reason it was deleted): `membership.list`
/// (the People tab) and the login path (`identity.workspaces` / `login_workspaces`) read the SAME
/// rows, so they can never disagree about who belongs. The old union had two sources keyed differently
/// — the roster synthesized `"user:" + <bare handle>` from a legacy row while the login path did a
/// keyed read on the already-prefixed sub — so `admin/members` listed a person that `/auth/login` then
/// refused with "not a member of any workspace"
/// (`docs/debugging/app/roster-login-disagree-legacy-user-rows.md`).
///
/// Asserted from BOTH directions, on the real `mem://` store through the real verbs.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn roster_and_login_path_agree_on_the_one_membership_source() {
    let store = Store::memory().await.unwrap();
    seed_directory(&store, "nube").await;
    let admin = principal("user:alice", "nube", MANAGE);

    // A member: `membership_add` is the ONE write. Both readers must see it.
    membership_add(&store, &admin, "nube", "user:ap", 5)
        .await
        .unwrap();
    let roster = membership_list(&store, &admin, "nube").await.unwrap();
    assert!(
        roster.iter().any(|m| m.sub == "user:ap"),
        "the roster lists the member it was told about: {roster:?}"
    );
    let wss = identity_workspaces(&store, &admin, "user:ap")
        .await
        .unwrap();
    assert!(
        wss.iter().any(|w| w.ws == "nube"),
        "identity.workspaces resolves the same membership the roster shows: {wss:?}"
    );
    let login = login_workspaces(&store, "user:ap").await.unwrap();
    assert!(
        login.iter().any(|w| w.ws == "nube"),
        "the un-gated login path resolves it too: {login:?}"
    );

    // A non-member: BOTH are empty. (Decision #4 — a provisioned identity with zero memberships
    // cannot enter a workspace it was never added to; there is no legacy row that could imply one.)
    lb_authz::identity_create(&store, "user:eve", None, 0)
        .await
        .unwrap();
    assert!(
        !roster.iter().any(|m| m.sub == "user:eve"),
        "a sub with no membership row is not on the roster"
    );
    assert!(
        identity_workspaces(&store, &admin, "user:eve")
            .await
            .unwrap()
            .is_empty(),
        "and resolves no workspaces"
    );
    assert!(
        login_workspaces(&store, "user:eve")
            .await
            .unwrap()
            .is_empty(),
        "and cannot log in anywhere"
    );

    // Removal keeps them in step: gone from the roster ⇒ gone from the login path.
    membership_remove(&store, &admin, "nube", "user:ap")
        .await
        .unwrap();
    assert!(
        !membership_list(&store, &admin, "nube")
            .await
            .unwrap()
            .iter()
            .any(|m| m.sub == "user:ap"),
        "removed from the roster"
    );
    assert!(
        login_workspaces(&store, "user:ap")
            .await
            .unwrap()
            .is_empty(),
        "and removed from the login path, in the same step"
    );
}

// ── leave is a clean exit: live token refused after remove ────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn membership_remove_revokes_grants_and_marks_token() {
    let store = Store::memory().await.unwrap();
    let admin = principal("user:alice", "nube", MANAGE);
    membership_add(&store, &admin, "nube", "user:bob", 1)
        .await
        .unwrap();
    // join granted role:member.
    let caps = grant_list(&store, "nube", &Subject::User("bob".into()))
        .await
        .unwrap();
    assert!(caps.iter().any(|c| c == "role:member"));

    let revoked = membership_remove(&store, &admin, "nube", "user:bob")
        .await
        .unwrap();
    assert!(revoked >= 1, "role:member grant was revoked");
    // membership row is tombstoned.
    assert!(!membership_is_member(&store, "nube", "user:bob")
        .await
        .unwrap());
    // grants are tombstoned.
    let caps = grant_list(&store, "nube", &Subject::User("bob".into()))
        .await
        .unwrap();
    assert!(!caps.iter().any(|c| c == "role:member"));
    // live-token marker is set → the verify path refuses bob's current token.
    assert!(
        lb_authz::token_revoked(&store, "nube", &Subject::User("bob".into()))
            .await
            .unwrap()
    );
}

// ── offline / sync: a removed membership is not resurrected by a stale edge ───────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn removed_membership_tombstone_replays_idempotently() {
    let store = Store::memory().await.unwrap();
    let admin = principal("user:alice", "nube", MANAGE);
    membership_add(&store, &admin, "nube", "user:bob", 1)
        .await
        .unwrap();
    membership_remove(&store, &admin, "nube", "user:bob")
        .await
        .unwrap();
    // A stale synced edge re-applies the SAME remove tombstone (sync §6.8) — bob stays removed, not
    // resurrected. Re-applying the raw tombstone is a no-op for membership.
    lb_authz::membership_remove_raw(&store, "nube", "user:bob")
        .await
        .unwrap();
    assert!(!membership_is_member(&store, "nube", "user:bob")
        .await
        .unwrap());
    // And a hub-added membership reaches the read path after "reconnect" (just a fresh read).
    membership_add(&store, &admin, "nube", "user:carol", 2)
        .await
        .unwrap();
    let members = membership_list(&store, &admin, "nube").await.unwrap();
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
