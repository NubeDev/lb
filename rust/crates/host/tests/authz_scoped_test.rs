//! Entity-scoped grants at the host layer: the MCP bridge (`call_authz_tool`) with scoped
//! `grants.assign`, the `authz.check_scoped` / `authz.scope_filter` verbs, the mandatory
//! capability-deny (without `mcp:authz.check_scoped:call` → 403), and workspace isolation.
//! Real store, real resolver, real capability gate — no mocks.

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{call_authz_tool, grants_assign, Scope, Subject};
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

const ADMIN: &[&str] = &[
    "mcp:grants.assign:call",
    "mcp:grants.list:call",
    "mcp:hvac.setpoint:call",
    "mcp:authz.check_scoped:call",
    "mcp:authz.scope_filter:call",
];

fn ids_scope(table: &str, ids: &[&str]) -> serde_json::Value {
    json!({
        "kind": "ids",
        "table": table,
        "ids": ids,
    })
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn scoped_grant_assign_and_check_scoped_over_mcp() {
    let store = Store::memory().await.unwrap();
    let admin = principal("user:alice", "acme", ADMIN);

    // Assign a scoped grant to ana via the MCP bridge.
    call_authz_tool(
        &store,
        &admin,
        "acme",
        "grants.assign",
        &json!({
            "subject": "user:ana",
            "cap": "mcp:hvac.setpoint:call",
            "scope": ids_scope("child", &["leo"]),
        }),
    )
    .await
    .unwrap();

    // Ana calls check_scoped for leo → allowed (she has a scoped grant for leo).
    let ana = principal(
        "user:ana",
        "acme",
        &["mcp:authz.check_scoped:call", "mcp:hvac.setpoint:call"],
    );
    let ok = call_authz_tool(
        &store,
        &ana,
        "acme",
        "authz.check_scoped",
        &json!({
            "cap": "mcp:hvac.setpoint:call",
            "table": "child",
            "id": "leo",
        }),
    )
    .await
    .unwrap();
    assert_eq!(ok["allowed"], json!(true));

    // Ana calls check_scoped for mia → denied (outside her scope).
    let denied = call_authz_tool(
        &store,
        &ana,
        "acme",
        "authz.check_scoped",
        &json!({
            "cap": "mcp:hvac.setpoint:call",
            "table": "child",
            "id": "mia",
        }),
    )
    .await
    .unwrap();
    assert_eq!(denied["allowed"], json!(false));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn check_scoped_for_scoped_principal() {
    let store = Store::memory().await.unwrap();
    let admin = principal("user:alice", "acme", ADMIN);

    // Admin grants ana a scoped cap.
    grants_assign(
        &store,
        &admin,
        "acme",
        &Subject::User("ana".into()),
        "mcp:hvac.setpoint:call",
        &Scope::Ids {
            table: "child".into(),
            ids: vec!["leo".into()],
        },
    )
    .await
    .unwrap();

    // Ana calls check_scoped for leo → allowed.
    let ana = principal(
        "user:ana",
        "acme",
        &["mcp:authz.check_scoped:call", "mcp:hvac.setpoint:call"],
    );
    let ok = call_authz_tool(
        &store,
        &ana,
        "acme",
        "authz.check_scoped",
        &json!({"cap": "mcp:hvac.setpoint:call", "table": "child", "id": "leo"}),
    )
    .await
    .unwrap();
    assert_eq!(ok["allowed"], json!(true));

    // Ana calls check_scoped for mia → denied (outside scope).
    let denied = call_authz_tool(
        &store,
        &ana,
        "acme",
        "authz.check_scoped",
        &json!({"cap": "mcp:hvac.setpoint:call", "table": "child", "id": "mia"}),
    )
    .await
    .unwrap();
    assert_eq!(denied["allowed"], json!(false));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn scope_filter_over_mcp_returns_ids() {
    let store = Store::memory().await.unwrap();
    let admin = principal("user:alice", "acme", ADMIN);

    grants_assign(
        &store,
        &admin,
        "acme",
        &Subject::User("ana".into()),
        "mcp:hvac.setpoint:call",
        &Scope::Ids {
            table: "child".into(),
            ids: vec!["leo".into(), "mia".into()],
        },
    )
    .await
    .unwrap();

    let ana = principal(
        "user:ana",
        "acme",
        &["mcp:authz.scope_filter:call", "mcp:hvac.setpoint:call"],
    );
    let result = call_authz_tool(
        &store,
        &ana,
        "acme",
        "authz.scope_filter",
        &json!({"cap": "mcp:hvac.setpoint:call", "table": "child"}),
    )
    .await
    .unwrap();
    let filter_ids = result["filter"]["ids"].as_array().unwrap();
    assert_eq!(filter_ids.len(), 2);
}

// ── Mandatory: capability-deny ──────────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn denies_check_scoped_without_its_cap() {
    let store = Store::memory().await.unwrap();
    // Ana has the hvac cap but NOT the authz.check_scoped cap.
    let ana = principal("user:ana", "acme", &["mcp:hvac.setpoint:call"]);
    let err = call_authz_tool(
        &store,
        &ana,
        "acme",
        "authz.check_scoped",
        &json!({"cap": "mcp:hvac.setpoint:call", "table": "child", "id": "leo"}),
    )
    .await
    .unwrap_err();
    assert!(matches!(err, ToolError::Denied));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn denies_scope_filter_without_its_cap() {
    let store = Store::memory().await.unwrap();
    let ana = principal("user:ana", "acme", &["mcp:hvac.setpoint:call"]);
    let err = call_authz_tool(
        &store,
        &ana,
        "acme",
        "authz.scope_filter",
        &json!({"cap": "mcp:hvac.setpoint:call", "table": "child"}),
    )
    .await
    .unwrap_err();
    assert!(matches!(err, ToolError::Denied));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn denies_scoped_grant_assign_without_grants_cap() {
    let store = Store::memory().await.unwrap();
    // Ana has the hvac cap but NOT the grants.assign cap.
    let ana = principal("user:ana", "acme", &["mcp:hvac.setpoint:call"]);
    let err = call_authz_tool(
        &store,
        &ana,
        "acme",
        "grants.assign",
        &json!({
            "subject": "user:bob",
            "cap": "mcp:hvac.setpoint:call",
            "scope": ids_scope("child", &["leo"]),
        }),
    )
    .await
    .unwrap_err();
    assert!(matches!(err, ToolError::Denied));
}

// ── Mandatory: workspace isolation ───────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn scoped_checks_never_cross_workspace_wall() {
    let store = Store::memory().await.unwrap();
    let admin_a = principal("user:alice", "acme", ADMIN);

    grants_assign(
        &store,
        &admin_a,
        "acme",
        &Subject::User("ana".into()),
        "mcp:hvac.setpoint:call",
        &Scope::Ids {
            table: "child".into(),
            ids: vec!["leo".into()],
        },
    )
    .await
    .unwrap();

    // Ana in globex (different ws) has no scoped grant — check returns false.
    let ana_b = principal(
        "user:ana",
        "globex",
        &["mcp:authz.check_scoped:call", "mcp:hvac.setpoint:call"],
    );
    let result = call_authz_tool(
        &store,
        &ana_b,
        "globex",
        "authz.check_scoped",
        &json!({"cap": "mcp:hvac.setpoint:call", "table": "child", "id": "leo"}),
    )
    .await
    .unwrap();
    assert_eq!(result["allowed"], json!(false));
}
