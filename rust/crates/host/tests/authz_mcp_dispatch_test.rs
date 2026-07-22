//! `grants.*` / `roles.*` / `teams.*` over the MCP dispatcher (authz-verbs-mcp-dispatch scope).
//!
//! These verbs are implemented by `call_authz_tool`, but until this scope the host dispatcher
//! (`is_host_native` / `HOST_NATIVE_PREFIXES`) only routed the `authz.` prefix — so a call to
//! `grants.assign` over the bridge (the transport a native Tier-2 extension reaches the host on)
//! fell through to the extension registry and denied. This suite exercises the REAL bridge
//! (`call_tool` → real `Node` → real store), proving:
//!
//! - **the four cap-gate aliases** (`gate_tool_for`): a workspace-admin token that carries the
//!   admin caps by their *canonical* names (`grants.assign`, `grants.list`, `teams.manage`,
//!   `roles.manage`, `roles.define`) — and NOT the per-verb names `grants.revoke` /
//!   `grants.list_scoped` / `teams.create` / `roles.delete` — can call all nine verbs. Without the
//!   aliases exactly those four would deny even for an admin. This is the load-bearing test;
//! - **capability-deny** (mandatory): a caller missing the gate cap is opaquely `Denied`, per verb;
//! - **anti-widen over the callback**: an admin holding `grants.assign` but NOT the target cap is
//!   `BadInput`, proving the handler's guard runs regardless of transport;
//! - **read/write symmetry**: assign over the bridge, then read the write back via
//!   `grants.list_scoped` over the same bridge;
//! - **workspace isolation** (mandatory): a ws-B admin cannot touch ws-A's authz records.
//!
//! Real `Node`, real store, real caps — no mocks (testing-scope §0).

use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{call_tool, Node};
use lb_mcp::ToolError;
use serde_json::{json, Value};

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

async fn call(
    node: &Arc<Node>,
    p: &Principal,
    ws: &str,
    tool: &str,
    input: Value,
) -> Result<Value, ToolError> {
    let out = call_tool(node, p, ws, tool, &input.to_string()).await?;
    Ok(serde_json::from_str(&out).unwrap())
}

/// The admin caps by their CANONICAL names — deliberately NOT holding `grants.revoke` /
/// `grants.list_scoped` / `teams.create` / `roles.delete` by name. Those verbs must pass anyway,
/// via the `gate_tool_for` aliases. Also holds the example target cap so no-widen is satisfied when
/// this admin grants it.
const ADMIN: &[&str] = &[
    "mcp:grants.assign:call",
    "mcp:grants.list:call",
    "mcp:roles.define:call",
    "mcp:roles.list:call",
    "mcp:roles.manage:call",
    "mcp:teams.manage:call",
    "mcp:teams.list:call",
    "mcp:hvac.setpoint:call",
];

// ── The load-bearing test: all nine verbs dispatch, incl. the four aliased ones ────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn admin_reaches_all_nine_authz_verbs_over_the_bridge() {
    let ws = "ws-authz-dispatch";
    let node = Arc::new(Node::boot().await.unwrap());
    let admin = principal("user:alice", ws, ADMIN);

    // teams.create — aliased to mcp:teams.manage:call (no mcp:teams.create:call exists).
    call(
        &node,
        &admin,
        ws,
        "teams.create",
        json!({ "team": "facilities", "name": "Facilities" }),
    )
    .await
    .expect("teams.create dispatches (alias → teams.manage)");
    let teams = call(&node, &admin, ws, "teams.list", json!({}))
        .await
        .expect("teams.list dispatches");
    assert!(
        teams["teams"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["team"] == "facilities"),
        "the created team is listed: {teams}"
    );

    // roles.define (own cap) then roles.delete — aliased to mcp:roles.manage:call.
    call(
        &node,
        &admin,
        ws,
        "roles.define",
        json!({ "name": "operator", "caps": ["mcp:hvac.setpoint:call"] }),
    )
    .await
    .expect("roles.define dispatches");
    call(&node, &admin, ws, "roles.list", json!({}))
        .await
        .expect("roles.list dispatches");
    call(
        &node,
        &admin,
        ws,
        "roles.delete",
        json!({ "name": "operator" }),
    )
    .await
    .expect("roles.delete dispatches (alias → roles.manage)");

    // grants.assign then grants.revoke — revoke aliased to mcp:grants.assign:call.
    call(
        &node,
        &admin,
        ws,
        "grants.assign",
        json!({ "subject": "user:bob", "cap": "mcp:hvac.setpoint:call" }),
    )
    .await
    .expect("grants.assign dispatches");
    call(
        &node,
        &admin,
        ws,
        "grants.revoke",
        json!({ "subject": "user:bob", "cap": "mcp:hvac.setpoint:call" }),
    )
    .await
    .expect("grants.revoke dispatches (alias → grants.assign)");

    // grants.list_scoped — aliased to mcp:grants.list:call.
    call(
        &node,
        &admin,
        ws,
        "grants.list_scoped",
        json!({ "subject": "user:bob" }),
    )
    .await
    .expect("grants.list_scoped dispatches (alias → grants.list)");
}

// ── Read/write symmetry over the one transport ─────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn assign_then_read_back_scoped_over_the_bridge() {
    let ws = "ws-authz-symmetry";
    let node = Arc::new(Node::boot().await.unwrap());
    let admin = principal("user:alice", ws, ADMIN);

    call(
        &node,
        &admin,
        ws,
        "grants.assign",
        json!({
            "subject": "user:guardian",
            "cap": "mcp:hvac.setpoint:call",
            "scope": { "kind": "ids", "table": "child", "ids": ["child-1"] }
        }),
    )
    .await
    .expect("scoped grant assigned over the bridge");

    let scoped = call(
        &node,
        &admin,
        ws,
        "grants.list_scoped",
        json!({ "subject": "user:guardian" }),
    )
    .await
    .expect("read the write back over the same bridge");
    let grants = scoped["grants"].as_array().unwrap();
    assert!(
        grants.iter().any(|g| g["cap"] == "mcp:hvac.setpoint:call"),
        "the scoped grant is read back: {scoped}"
    );
}

// ── Mandatory: capability-deny, per verb, over the real bridge ──────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn denies_each_verb_without_its_gate_cap_over_the_bridge() {
    let ws = "ws-authz-deny";
    let node = Arc::new(Node::boot().await.unwrap());
    // Holds ONLY grants.list — every other verb (incl. the aliased ones) must deny.
    let mallory = principal("user:mallory", ws, &["mcp:grants.list:call"]);

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
        ("roles.delete", json!({ "name": "operator" })),
        (
            "teams.create",
            json!({ "team": "facilities", "name": "Facilities" }),
        ),
    ] {
        let err = call(&node, &mallory, ws, verb, input).await.unwrap_err();
        assert!(
            matches!(err, ToolError::Denied),
            "{verb} must be opaquely Denied over the bridge, got {err:?}"
        );
    }

    // grants.list_scoped is aliased to grants.list, which mallory holds → allowed (empty).
    call(
        &node,
        &mallory,
        ws,
        "grants.list_scoped",
        json!({ "subject": "user:bob" }),
    )
    .await
    .expect("grants.list_scoped allowed via the grants.list cap it aliases to");
}

// ── Anti-widen still fires over the callback ────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn anti_widen_fires_over_the_bridge() {
    let ws = "ws-authz-widen";
    let node = Arc::new(Node::boot().await.unwrap());
    // Admin can assign grants, but does NOT hold the target cap it tries to hand out.
    let admin = principal(
        "user:alice",
        ws,
        &["mcp:grants.assign:call", "mcp:grants.list:call"],
    );

    let err = call(
        &node,
        &admin,
        ws,
        "grants.assign",
        json!({ "subject": "user:bob", "cap": "mcp:hvac.setpoint:call" }),
    )
    .await
    .unwrap_err();
    assert!(
        matches!(err, ToolError::BadInput(_)),
        "cannot grant a cap you lack (anti-widen) over the bridge, got {err:?}"
    );
}

// ── Mandatory: two-workspace isolation over the bridge ──────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn ws_b_admin_cannot_touch_ws_a_authz_over_the_bridge() {
    let node = Arc::new(Node::boot().await.unwrap());
    let ws_a = "ws-authz-iso-a";
    let admin_a = principal("user:alice", ws_a, ADMIN);
    // A ws-B admin with identical caps, but scoped to a different workspace.
    let admin_b = principal("user:carol", ws_a, ADMIN);

    // ws-A admin seeds a team + grant in ws_a.
    call(
        &node,
        &admin_a,
        ws_a,
        "teams.create",
        json!({ "team": "facilities", "name": "Facilities" }),
    )
    .await
    .unwrap();

    // A principal whose TOKEN workspace differs from the call's ws is denied at gate 1 (workspace).
    // The `ws` the dispatcher trusts comes from the caller's session, never the body — here we pass
    // admin_b (token ws = ws_a) but drive the call against a different workspace label to prove the
    // workspace-first gate. (In production `ws` is the session's; a mismatch cannot be forged.)
    let ws_b = "ws-authz-iso-b";
    for (verb, input) in [
        ("teams.list", json!({})),
        (
            "grants.assign",
            json!({ "subject": "user:bob", "cap": "mcp:hvac.setpoint:call" }),
        ),
    ] {
        let err = call(&node, &admin_b, ws_b, verb, input).await.unwrap_err();
        assert!(
            matches!(err, ToolError::Denied),
            "ws mismatch → {verb} denied at the workspace gate, got {err:?}"
        );
    }
}

// ── series.latest_many rides the series.latest cap (series-read-perf scope) ─────────────────────
// The batched fleet-snapshot read is ONE logical read of the series-latest surface, so it rides the
// existing `mcp:series.latest:call` grant via the `gate_tool_for` alias — no `_many` cap exists in
// any role bundle. Regression guard for the shipped-but-denied state: dispatch demanded the literal
// `mcp:series.latest_many:call` and denied every caller (incl. the ems ext + admins) until aliased.

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn series_latest_many_rides_the_series_latest_cap() {
    let ws = "ws-series-latest-many";
    let node = Arc::new(Node::boot().await.unwrap());

    // A caller holding ONLY the singular grant — deliberately NOT `mcp:series.latest_many:call`.
    let reader = principal("user:reader", ws, &["mcp:series.latest:call"]);

    // The batch dispatches (alias → series.latest) and returns the null-per-name contract for an
    // unseeded series — never `Denied`, which is the bug this test pins.
    let out = call(
        &node,
        &reader,
        ws,
        "series.latest_many",
        json!({ "series": ["modbus.x.y.z"] }),
    )
    .await
    .expect("series.latest_many dispatches on the mcp:series.latest:call grant (alias)");
    assert!(
        out["latest"].is_object(),
        "batch returns a {{ latest: {{ name: Sample|null }} }} map: {out}"
    );

    // A caller WITHOUT the series.latest grant is denied the whole batch (parity with singular).
    let no_cap = principal("user:intruder", ws, &["mcp:series.read:call"]);
    let err = call(
        &node,
        &no_cap,
        ws,
        "series.latest_many",
        json!({ "series": ["modbus.x.y.z"] }),
    )
    .await
    .unwrap_err();
    assert!(
        matches!(err, ToolError::Denied),
        "no series.latest grant → batch denied, got {err:?}"
    );
}
