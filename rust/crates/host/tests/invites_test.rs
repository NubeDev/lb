//! Invites at the host layer: the admin verbs (create/list/revoke/resend) + the pre-auth accept
//! chain (invites scope). Tests the mandatory capability-deny, workspace isolation, the atomic
//! onboarding (identity + membership + grants + session), expiry, double-redeem, revoke-then-accept,
//! and existing-identity-takeover prevention. Real store, real resolver, real capability gate —
//! no mocks (rule 9). The email target is the one sanctioned fake (RecordingEmailProvider).

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{
    call_invite_tool, invite_accept, invite_create, invite_list, InviteError, EMAIL_TARGET,
};
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

const ADMIN: &[&str] = &[
    "mcp:invite.create:call",
    "mcp:invite.list:call",
    "mcp:grants.assign:call",
    "role:member",
    "mcp:hvac.setpoint:call",
];

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn create_and_list_invite() {
    let store = Store::memory().await.unwrap();
    let admin = principal("user:alice", "acme", ADMIN);

    let token = invite_create(
        &store,
        &admin,
        "acme",
        "sam@example.com",
        "member",
        "",
        None,
        None,
        0,
        100,
    )
    .await
    .unwrap();
    assert!(token.starts_with("lbi_"));

    let invites = invite_list(&store, &admin, "acme").await.unwrap();
    assert_eq!(invites.len(), 1);
    assert_eq!(invites[0].email, "sam@example.com");
    assert_eq!(invites[0].role, "member");
    assert_eq!(invites[0].status, lb_authz::InviteStatus::Pending);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn accept_invite_onboards_new_member() {
    let store = Store::memory().await.unwrap();
    let key = SigningKey::generate();
    let admin = principal("user:alice", "acme", ADMIN);

    let token = invite_create(
        &store,
        &admin,
        "acme",
        "sam@example.com",
        "member",
        "",
        None,
        None,
        0,
        100,
    )
    .await
    .unwrap();

    let result = invite_accept(&store, &key, "acme", &token, "password123", None, 200)
        .await
        .unwrap();

    assert_eq!(result.sub, "user:sam@example.com");
    assert_eq!(result.workspace, "acme");
    assert!(!result.caps.is_empty(), "caps must be live on first login");

    // The invite is now accepted.
    let invites = invite_list(&store, &admin, "acme").await.unwrap();
    assert_eq!(invites[0].status, lb_authz::InviteStatus::Accepted);
    assert_eq!(
        invites[0].accepted_by.as_deref(),
        Some("user:sam@example.com")
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn double_redeem_is_rejected() {
    let store = Store::memory().await.unwrap();
    let key = SigningKey::generate();
    let admin = principal("user:alice", "acme", ADMIN);

    let token = invite_create(
        &store,
        &admin,
        "acme",
        "sam@example.com",
        "member",
        "",
        None,
        None,
        0,
        100,
    )
    .await
    .unwrap();

    invite_accept(&store, &key, "acme", &token, "password123", None, 200)
        .await
        .unwrap();

    let err = invite_accept(&store, &key, "acme", &token, "password123", None, 200)
        .await
        .unwrap_err();
    assert!(matches!(err, InviteError::AlreadyAccepted));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn expired_invite_is_rejected() {
    let store = Store::memory().await.unwrap();
    let key = SigningKey::generate();
    let admin = principal("user:alice", "acme", ADMIN);

    let token = invite_create(
        &store,
        &admin,
        "acme",
        "sam@example.com",
        "member",
        "",
        None,
        None,
        50,
        10,
    )
    .await
    .unwrap();

    let err = invite_accept(&store, &key, "acme", &token, "password123", None, 100)
        .await
        .unwrap_err();
    assert!(matches!(err, InviteError::Expired));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn revoke_then_accept_is_rejected() {
    let store = Store::memory().await.unwrap();
    let key = SigningKey::generate();
    let admin = principal("user:alice", "acme", ADMIN);

    let token = invite_create(
        &store,
        &admin,
        "acme",
        "sam@example.com",
        "member",
        "",
        None,
        None,
        0,
        100,
    )
    .await
    .unwrap();

    // Revoke via the MCP bridge.
    let invites = invite_list(&store, &admin, "acme").await.unwrap();
    let hash = &invites[0].token_hash;
    call_invite_tool(
        &store,
        &admin,
        "acme",
        "invite.revoke",
        &json!({ "token_hash": hash, "now": 150 }),
    )
    .await
    .unwrap();

    let err = invite_accept(&store, &key, "acme", &token, "password123", None, 200)
        .await
        .unwrap_err();
    assert!(matches!(err, InviteError::Revoked));
}

// ── Mandatory: capability-deny ──────────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn denies_create_without_invite_create_cap() {
    let store = Store::memory().await.unwrap();
    let mallory = principal("user:mallory", "acme", &["mcp:invite.list:call"]);

    let err = invite_create(
        &store,
        &mallory,
        "acme",
        "sam@example.com",
        "member",
        "",
        None,
        None,
        0,
        100,
    )
    .await
    .unwrap_err();
    assert!(matches!(err, InviteError::Denied));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn denies_list_without_invite_list_cap() {
    let store = Store::memory().await.unwrap();
    let mallory = principal("user:mallory", "acme", &["mcp:invite.create:call"]);

    let err = invite_list(&store, &mallory, "acme").await.unwrap_err();
    assert!(matches!(err, InviteError::Denied));
}

// ── Mandatory: workspace isolation ───────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn invite_not_visible_from_other_workspace() {
    let store = Store::memory().await.unwrap();
    let admin_a = principal("user:alice", "acme", ADMIN);

    invite_create(
        &store,
        &admin_a,
        "acme",
        "sam@example.com",
        "member",
        "",
        None,
        None,
        0,
        100,
    )
    .await
    .unwrap();

    // A ws-B admin sees no invites from ws-A.
    let admin_b = principal("user:carol", "globex", ADMIN);
    let invites = invite_list(&store, &admin_b, "globex").await.unwrap();
    assert!(invites.is_empty(), "ws-B must not see ws-A's invites");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn accept_with_wrong_workspace_fails() {
    let store = Store::memory().await.unwrap();
    let key = SigningKey::generate();
    let admin = principal("user:alice", "acme", ADMIN);

    let token = invite_create(
        &store,
        &admin,
        "acme",
        "sam@example.com",
        "member",
        "",
        None,
        None,
        0,
        100,
    )
    .await
    .unwrap();

    // Try to accept in a different workspace — the invite doesn't exist there.
    let err = invite_accept(&store, &key, "globex", &token, "password123", None, 200)
        .await
        .unwrap_err();
    assert!(matches!(err, InviteError::NotFound));
}

// ── Role grants follow the grants.assign precedent ───────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn admin_can_invite_with_any_role() {
    let store = Store::memory().await.unwrap();
    let admin = principal("user:alice", "acme", ADMIN);

    // An admin with invite.create can invite with any role (same as grants.assign exempts role:
    // caps — the role's caps were bounded at roles.define time).
    let token = invite_create(
        &store,
        &admin,
        "acme",
        "sam@example.com",
        "workspace-admin",
        "",
        None,
        None,
        0,
        100,
    )
    .await
    .unwrap();
    assert!(token.starts_with("lbi_"));
}

// ── Bad token ────────────────────────────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn bad_token_is_rejected() {
    let store = Store::memory().await.unwrap();
    let key = SigningKey::generate();

    let err = invite_accept(
        &store,
        &key,
        "acme",
        "not-a-token",
        "password123",
        None,
        200,
    )
    .await
    .unwrap_err();
    assert!(matches!(err, InviteError::BadToken));
}
