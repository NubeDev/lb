//! Authz at the host layer: the grant/role/team admin verbs over the MCP surface, the mandatory
//! per-verb capability-deny and two-workspace isolation tests, and the session cap projection
//! (`resolve_caps`) incl. the no-widening guard, idempotency, and the revoke seam (authz-grants
//! scope). Mirrors `tags_test.rs`'s shape: a `principal()` token factory, deny-without-grant, and a
//! two-principal isolation check across store + MCP.

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{
    call_authz_tool, grants_assign, resolve_caps, revoke_subject, roles_define, teams_create,
    AuthzError, Subject,
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

/// Every authz admin cap, plus the example tool caps an admin hands out (so the no-widening rule is
/// satisfied when this admin grants them).
const ADMIN: &[&str] = &[
    "mcp:grants.assign:call",
    "mcp:grants.list:call",
    "mcp:roles.define:call",
    "mcp:roles.list:call",
    "mcp:teams.manage:call",
    "mcp:teams.list:call",
    "mcp:hvac.setpoint:call",
    "store:series/hvac:read",
];

// ── Mandatory: capability deny, per verb, over the real MCP bridge ────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn denies_each_admin_verb_without_its_grant() {
    let store = Store::memory().await.unwrap();
    // Holds only grants.list — every mutating/other verb is denied.
    let p = principal("user:mallory", "acme", &["mcp:grants.list:call"]);
    for (verb, input) in [
        (
            "grants.assign",
            json!({ "subject": "user:bob", "cap": "mcp:hvac.setpoint:call" }),
        ),
        (
            "grants.revoke",
            json!({ "subject": "user:bob", "cap": "mcp:hvac.setpoint:call" }),
        ),
        (
            "roles.define",
            json!({ "name": "operator", "caps": ["mcp:hvac.setpoint:call"] }),
        ),
        ("roles.list", json!({})),
        (
            "teams.create",
            json!({ "team": "facilities", "name": "Facilities" }),
        ),
        ("teams.list", json!({})),
    ] {
        let err = call_authz_tool(&store, &p, "acme", verb, &input)
            .await
            .unwrap_err();
        assert!(matches!(err, ToolError::Denied), "{verb} must be denied");
    }
    // The one held read verb is allowed (returns empty).
    call_authz_tool(
        &store,
        &p,
        "acme",
        "grants.list",
        &json!({ "subject": "user:bob" }),
    )
    .await
    .unwrap();
}

// ── Mandatory: two-workspace isolation, over store + MCP ──────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn ws_b_admin_cannot_see_or_touch_ws_a_authz() {
    let store = Store::memory().await.unwrap();
    let admin_a = principal("user:alice", "acme", ADMIN);
    // A ws-B admin: full admin caps, but in workspace `globex`.
    let admin_b = principal("user:carol", "globex", ADMIN);

    // ws-A admin seeds a team + a grant in acme.
    teams_create(&store, &admin_a, "acme", "facilities", "Facilities")
        .await
        .unwrap();
    grants_assign(
        &store,
        &admin_a,
        "acme",
        &Subject::User("bob".into()),
        "mcp:hvac.setpoint:call",
    )
    .await
    .unwrap();

    // ws-B admin targeting acme: gate 1 (workspace) denies — opaque, over the MCP bridge.
    for (verb, input) in [
        ("teams.list", json!({})),
        ("grants.list", json!({ "subject": "user:bob" })),
        (
            "grants.assign",
            json!({ "subject": "user:bob", "cap": "mcp:hvac.setpoint:call" }),
        ),
    ] {
        let err = call_authz_tool(&store, &admin_b, "acme", verb, &input)
            .await
            .unwrap_err();
        assert!(
            matches!(err, ToolError::Denied),
            "ws-B → ws-A {verb} must be denied"
        );
    }

    // And at the store layer: ws-B's own namespace shows none of ws-A's authz records.
    assert!(
        resolve_caps(&store, "globex", "bob")
            .await
            .unwrap()
            .is_empty(),
        "bob has no caps in globex — ws-A's grant must not leak across the wall"
    );
}

// ── Slice cases: grant resolution, role bundles, team inheritance, no-widening, idempotency ───

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn resolve_unions_direct_role_and_team_inherited_caps() {
    let store = Store::memory().await.unwrap();
    let admin = principal("user:alice", "acme", ADMIN);

    // Direct user grant.
    grants_assign(
        &store,
        &admin,
        "acme",
        &Subject::User("bob".into()),
        "mcp:hvac.setpoint:call",
    )
    .await
    .unwrap();

    // A role `operator` bundling a store cap, assigned (as a grant) to the `facilities` team.
    roles_define(
        &store,
        &admin,
        "acme",
        "operator",
        &["store:series/hvac:read".to_string()],
    )
    .await
    .unwrap();
    teams_create(&store, &admin, "acme", "facilities", "Facilities")
        .await
        .unwrap();
    grants_assign(
        &store,
        &admin,
        "acme",
        &Subject::Team("facilities".into()),
        "role:operator",
    )
    .await
    .unwrap();
    // Bob joins facilities (the member edge the resolver walks).
    lb_host::add_team_member(
        &store,
        &principal("user:alice", "acme", &["mcp:members.add:call"]),
        "acme",
        "facilities",
        "bob",
    )
    .await
    .unwrap();

    let mut caps = resolve_caps(&store, "acme", "bob").await.unwrap();
    caps.sort();
    assert_eq!(
        caps,
        vec![
            "mcp:hvac.setpoint:call".to_string(),
            "store:series/hvac:read".to_string()
        ],
        "caps = direct ∪ team's role bundle"
    );

    // A non-member gets none of the team-inherited caps.
    assert!(resolve_caps(&store, "acme", "eve")
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn no_widening_blocks_granting_a_cap_the_admin_lacks() {
    let store = Store::memory().await.unwrap();
    // Admin can assign grants, but does NOT hold the hvac cap.
    let admin = principal("user:alice", "acme", &["mcp:grants.assign:call"]);
    let err = grants_assign(
        &store,
        &admin,
        "acme",
        &Subject::User("bob".into()),
        "mcp:hvac.setpoint:call",
    )
    .await
    .unwrap_err();
    assert!(matches!(err, AuthzError::Widen(_)), "must refuse to widen");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn assign_and_revoke_are_idempotent_and_revoke_seam_strips_all() {
    let store = Store::memory().await.unwrap();
    let admin = principal("user:alice", "acme", ADMIN);
    let bob = Subject::User("bob".into());

    // Double-assign the same grant → still just one live cap.
    grants_assign(&store, &admin, "acme", &bob, "mcp:hvac.setpoint:call")
        .await
        .unwrap();
    grants_assign(&store, &admin, "acme", &bob, "mcp:hvac.setpoint:call")
        .await
        .unwrap();
    assert_eq!(resolve_caps(&store, "acme", "bob").await.unwrap().len(), 1);

    // The revoke seam strips every grant (admin-crud calls this on user.delete). Returns the count.
    let n = revoke_subject(&store, "acme", &bob).await.unwrap();
    assert_eq!(n, 1);
    assert!(resolve_caps(&store, "acme", "bob")
        .await
        .unwrap()
        .is_empty());

    // Re-running the seam is a harmless no-op (idempotent replay).
    assert_eq!(revoke_subject(&store, "acme", &bob).await.unwrap(), 0);
}

// ── access-console scope: the three new verbs (authz.resolve, authz.revoke-tokens, roles.delete) —
//    per-verb deny over the MCP bridge + two-workspace isolation. The verbs compose the seams above
//    (resolve_caps_sourced / token_revoke + revoke_subject / role_delete), so these prove the GATE. ─

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn denies_each_access_console_verb_without_its_grant() {
    let store = Store::memory().await.unwrap();
    // Holds grants.list only — none of the access-console admin caps.
    let p = principal("user:mallory", "acme", &["mcp:grants.list:call"]);
    for (verb, input) in [
        ("authz.resolve", json!({ "subject": "user:bob" })),
        ("authz.revoke-tokens", json!({ "subject": "user:bob" })),
        ("roles.delete", json!({ "name": "operator" })),
    ] {
        let err = call_authz_tool(&store, &p, "acme", verb, &input)
            .await
            .unwrap_err();
        assert!(
            matches!(err, ToolError::Denied),
            "{verb} must be denied without its cap"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn access_console_verbs_cannot_cross_the_workspace_wall() {
    let store = Store::memory().await.unwrap();
    // A ws-B admin holding every access-console cap, but in workspace `globex`.
    let ws_b = principal(
        "user:carol",
        "globex",
        &[
            "mcp:authz.resolve:call",
            "mcp:authz.revoke-tokens:call",
            "mcp:roles.manage:call",
        ],
    );
    // Calling into "acme" (gate 1 — the caller's ws is globex) is denied for every verb.
    for (verb, input) in [
        ("authz.resolve", json!({ "subject": "user:bob" })),
        ("authz.revoke-tokens", json!({ "subject": "user:bob" })),
        ("roles.delete", json!({ "name": "operator" })),
    ] {
        let err = call_authz_tool(&store, &ws_b, "acme", verb, &input)
            .await
            .unwrap_err();
        assert!(
            matches!(err, ToolError::Denied),
            "ws-B → ws-A {verb} must be denied at the workspace wall"
        );
    }
}
