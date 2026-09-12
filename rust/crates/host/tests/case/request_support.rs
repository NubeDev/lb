//! The **contractor round trip** — `party.*`, `case.request.*`, the link email, the nudge ladder
//! and the token principal, over a REAL booted `Node` (`docs/scope/insights/case-plane-scope.md`,
//! wave 2).
//!
//! Real store (`mem://`), real bus, real caps, the real `call_tool` MCP bridge, the real outbox
//! relay and the real reminder reactor. The ONE permitted external is the email provider, behind
//! the `EmailProvider` trait — and both shipped non-transport impls are used deliberately:
//! `RecordingEmailProvider` to read what was actually mailed (which is how these tests get a
//! working token: the raw string exists ONLY in the link, exactly as production says it does), and
//! `LoggingEmailProvider` to prove the thing resolved decision 7 exists for — that a node with no
//! mailer yields `delivery: logged` and NEVER `sent`.
//!
//! Mandatory categories, per new verb: **capability-deny** and **workspace-isolation**. Plus a
//! POSITIVE gate test per verb, because a missing `tool_gate.rs` arm is `Denied`, not `NotFound`.
//!
//! **Fixtures only.** Split out of `case_request_test.rs` when that file passed the 400-line
//! FILE-LAYOUT limit. The tests live in the `request_*.rs` siblings, aggregated by
//! `case_suite.rs`; this module holds what they share and asserts nothing itself.

#![allow(dead_code)]

pub use std::sync::Arc;

pub use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
pub use lb_authz::membership_add_raw;
pub use lb_host::{
    call_tool, relay_outbox, EmailTarget, LoggingEmailProvider, Node, RecordingEmailProvider,
};
pub use lb_mcp::ToolError;
pub use serde_json::{json, Value};

pub const RAISE: &str = "mcp:insight.raise:call";
pub const I_GET: &str = "mcp:insight.get:call";
pub const GET: &str = "mcp:case.get:call";
pub const OPEN: &str = "mcp:case.open:call";
pub const WORKFLOW: &str = "mcp:case.workflow:call";
pub const SEND: &str = "mcp:case.request.send:call";
pub const P_UPSERT: &str = "mcp:party.upsert:call";
pub const P_LIST: &str = "mcp:party.list:call";

/// A fully-empowered operator: every cap this suite needs to SEED with.
pub const ALL: &[&str] = &[RAISE, I_GET, GET, OPEN, WORKFLOW, SEND, P_UPSERT, P_LIST];

/// The SLA policy caps, and the two DESTRUCTIVE ones (case-plane scope §Wave 5). Deliberately NOT
/// in [`ALL`]: `request_roster_lifecycle.rs` asserts that the authoring caps buy neither delete, so
/// a fixture that handed them out would erase the distinction for every test in the suite.
pub const POL_SET: &str = "mcp:policy.sla.set:call";
pub const POL_LIST: &str = "mcp:policy.sla.list:call";
pub const P_DELETE: &str = "mcp:party.delete:call";
pub const POL_DELETE: &str = "mcp:policy.sla.delete:call";

/// Everything an admin authoring the plane's configuration holds, INCLUDING the destructive pair.
pub fn admin_caps() -> Vec<&'static str> {
    let mut c = ALL.to_vec();
    c.extend([POL_SET, POL_LIST, P_DELETE, POL_DELETE]);
    c
}

/// The same, WITHOUT the two delete caps — the "may author, may not erase" session.
pub fn author_caps() -> Vec<&'static str> {
    let mut c = ALL.to_vec();
    c.extend([POL_SET, POL_LIST]);
    c
}

/// The `party.upsert` argument, with `active` explicit — the flag is the point of these fixtures.
pub fn party_input(id: &str, active: bool) -> Value {
    json!({
        "id": id,
        "kind": "contractor",
        "name": format!("{id} services"),
        "contact": { "email": format!("{id}@example.com") },
        "sites": ["site-a"],
        "trades": ["mechanical"],
        "default_ask_window_h": 0,
        "active": active,
    })
}

/// The `policy.sla.set` argument, with `active` explicit.
pub fn policy_input(id: &str, active: bool) -> Value {
    json!({
        "id": id,
        "name": format!("policy {id}"),
        "match": {},
        "respond_h": 4,
        "resolve_h": 24,
        "party_window_h": 48,
        "hold_down_days": 14,
        "active": active,
    })
}

/// One hour in the epoch-ms the case plane stores.
pub const HOUR_MS: u64 = 60 * 60 * 1000;

/// A logical "now" well inside the epoch-ms band, so the host's `normalize_ts` passes it through.
pub const T0: u64 = 1_800_000_000_000;

pub fn principal(sub: &str, ws: &str, caps: &[&str]) -> Principal {
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

pub async fn call(
    node: &Arc<Node>,
    p: &Principal,
    ws: &str,
    tool: &str,
    input: Value,
) -> Result<Value, ToolError> {
    let out = call_tool(node, p, ws, tool, &input.to_string()).await?;
    Ok(serde_json::from_str(&out).unwrap_or(Value::Null))
}

/// Grant `cap` to `user` in the DURABLE grant store — what the reminder's fire-time re-resolve
/// reads. The sub is parsed exactly as `grants.assign` parses it, so the row lands under the bare
/// handle the store keys on (wrapping it verbatim hides a dead fire path).
pub async fn grant(store: &lb_store::Store, ws: &str, user: &str, cap: &str) {
    let s = lb_authz::Subject::parse(user).expect("a subject like `user:test`");
    lb_authz::grant_assign(store, ws, &s, cap).await.unwrap();
}

/// Raise one insight; the inline grouping reactor opens its case. Returns the case id.
pub async fn seed_case(node: &Arc<Node>, p: &Principal, ws: &str, key: &str) -> String {
    let raised = call(
        node,
        p,
        ws,
        "insight.raise",
        json!({
            "dedup_key": key,
            "severity": "warning",
            "title": format!("finding {key}"),
            "origin": { "kind": "rule", "ref": "rule:probe" },
            "ts": T0,
        }),
    )
    .await
    .expect("raise ok");
    let insight = raised["id"].as_str().unwrap().to_string();
    let got = call(node, p, ws, "insight.get", json!({ "id": insight }))
        .await
        .expect("get ok");
    got["case_id"]
        .as_str()
        .unwrap_or_else(|| panic!("no case echo: {got}"))
        .to_string()
}

/// Put one party in the roster. `window_h` of 0 means "no opinion" (the policy/default decides).
pub async fn seed_party(node: &Arc<Node>, p: &Principal, ws: &str, id: &str, window_h: u32) {
    call(
        node,
        p,
        ws,
        "party.upsert",
        json!({
            "id": id,
            "kind": "contractor",
            "name": format!("{id} services"),
            "contact": { "email": format!("{id}@example.com") },
            "sites": ["site-a"],
            "trades": ["mechanical"],
            "default_ask_window_h": window_h,
        }),
    )
    .await
    .expect("party.upsert ok");
}

pub async fn seed_roster(node: &Arc<Node>, ws: &str) {
    membership_add_raw(&node.store, ws, "user:test", 1)
        .await
        .expect("test joins");
}

/// Drain the outbox through the REAL relay and the REAL email target, recording what was mailed.
pub async fn drain_recording(node: &Arc<Node>, ws: &str) -> Arc<RecordingEmailProvider> {
    let provider = Arc::new(RecordingEmailProvider::default());
    let target = EmailTarget::new(Box::new(provider.clone()), node.store.clone());
    relay_outbox(&node.store, ws, &target, T0 / 1000)
        .await
        .expect("a relay pass");
    provider
}

/// Drain the outbox through the LOGGING provider — a node with no mailer configured.
pub async fn drain_logging(node: &Arc<Node>, ws: &str) {
    let target = EmailTarget::new(Box::new(LoggingEmailProvider), node.store.clone());
    relay_outbox(&node.store, ws, &target, T0 / 1000)
        .await
        .expect("a relay pass");
}

/// The raw token out of the mail body — the ONLY place it exists (the row keeps a hash). This is
/// also the assertion that the link email actually carries a usable link.
pub fn token_from(body: &str) -> String {
    let at = body
        .find("/r/")
        .unwrap_or_else(|| panic!("no link in the mail: {body}"));
    body[at + 3..]
        .split_whitespace()
        .next()
        .expect("a token after /r/")
        .trim_end_matches(['.', ',', '<', '"'])
        .to_string()
}

/// Send an ask and return `(request_id, raw_token)` — the send, one relay pass, and the token read
/// back out of what was actually mailed.
pub async fn send_ask(
    node: &Arc<Node>,
    p: &Principal,
    ws: &str,
    case_id: &str,
    party: &str,
) -> (String, String) {
    let request = call(
        node,
        p,
        ws,
        "case.request.send",
        json!({ "case_id": case_id, "party_id": party, "ask": "quote", "ts": T0 }),
    )
    .await
    .expect("send ok");
    let id = request["id"].as_str().unwrap().to_string();
    let provider = drain_recording(node, ws).await;
    let sends = provider.sends();
    assert_eq!(sends.len(), 1, "one link email per ask");
    (id, token_from(&sends[0].body))
}

/// The case's history, newest-first page, as raw event rows.
pub async fn events_of(node: &Arc<Node>, p: &Principal, ws: &str, case_id: &str) -> Vec<Value> {
    let page = call(
        node,
        p,
        ws,
        "case.events",
        json!({ "case_id": case_id, "limit": 100 }),
    )
    .await
    .expect("events ok");
    page["items"]
        .as_array()
        .cloned()
        .unwrap_or_else(|| panic!("the event page has no `items`: {page}"))
}

pub fn count_kind(events: &[Value], kind: &str) -> usize {
    events
        .iter()
        .filter(|e| e["kind"].as_str() == Some(kind))
        .count()
}
