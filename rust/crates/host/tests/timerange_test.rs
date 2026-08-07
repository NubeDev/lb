//! `time.range.resolve` through the REAL MCP bridge (`lb_host::call_tool`) over a booted `Node` —
//! real caps wall, no mocks (relative-time-range scope, testing plan). Mandatory categories:
//!
//!   - HAPPY PATH (the positive control): a fresh subject HOLDING `mcp:time.range.resolve:call`
//!     resolves a window and gets `{fromMs,toMs,fromIso,toIso}`.
//!   - CAPABILITY DENY: the same verb without the cap is refused OPAQUELY at the caps wall — a
//!     fresh subject (not the suite's usual test@nube), so no residue grant can fake a pass.
//!   - MALFORMED INPUT: a bad token / a range token with `to` / an empty `from` / a bad tz are
//!     loud `BadInput`s NAMING the offender — nothing defaults silently.
//!
//! Workspace isolation for the FIELD this verb serves (`Dashboard.time`) lives in
//! `dashboard_test.rs::workspace_isolation` — the verb itself is pure compute over the caller's
//! own expression and reads no per-workspace state.

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

/// 2026-07-29 10:30:00 UTC.
const NOW_MS: i64 = 1_785_321_000_000;

async fn resolve(
    node: &Arc<Node>,
    p: &Principal,
    ws: &str,
    args: Value,
) -> Result<Value, ToolError> {
    let out = call_tool(node, p, ws, "time.range.resolve", &args.to_string()).await?;
    Ok(serde_json::from_str(&out).unwrap())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn resolve_with_the_grant_and_denied_without_it() {
    let ws = "ws-timerange";
    let node = Arc::new(Node::boot().await.expect("node boots"));

    // POSITIVE CONTROL — a fresh subject holding exactly the one cap resolves a window.
    let tara = principal("user:tara", ws, &["mcp:time.range.resolve:call"]);
    let out = resolve(
        &node,
        &tara,
        ws,
        json!({ "from": "last-7-days", "now": NOW_MS }),
    )
    .await
    .expect("the grant admits the call");
    assert_eq!(out["fromIso"], "2026-07-22");
    assert_eq!(out["toMs"], NOW_MS);
    assert_eq!(
        out["toMs"].as_i64().unwrap() - out["fromMs"].as_i64().unwrap(),
        7 * 86_400_000,
        "seven whole days ending now"
    );

    // A timezone rides through: "today" in Sydney is Sydney's day, exclusive `to`.
    let out = resolve(
        &node,
        &tara,
        ws,
        json!({ "from": "today", "tz": "Australia/Sydney", "now": 1_785_357_000_000i64 }),
    )
    .await
    .expect("tz form resolves");
    assert_eq!(out["fromIso"], "2026-07-30");
    assert_eq!(out["toIso"], "2026-07-31");

    // CAPABILITY DENY — a FRESH subject with an unrelated cap is refused OPAQUELY at the wall.
    let nocap = principal("user:nocap", ws, &["mcp:series.read:call"]);
    let err = resolve(
        &node,
        &nocap,
        ws,
        json!({ "from": "last-7-days", "now": NOW_MS }),
    )
    .await
    .unwrap_err();
    assert!(
        matches!(err, ToolError::Denied),
        "no grant ⇒ opaque deny, got {err:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn malformed_input_is_refused_naming_the_bad_token() {
    let ws = "ws-timerange-bad";
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let tara = principal("user:tara", ws, &["mcp:time.range.resolve:call"]);

    // An unknown token is named, with the legal set beside it.
    let err = resolve(
        &node,
        &tara,
        ws,
        json!({ "from": "last-fortnight", "now": NOW_MS }),
    )
    .await
    .unwrap_err();
    let ToolError::BadInput(msg) = err else {
        panic!("expected BadInput")
    };
    assert!(msg.contains("last-fortnight"), "names the token: {msg}");
    assert!(msg.contains("yesterday"), "names the legal set: {msg}");

    // A range token with a `to` is a shape refusal, not a silent ignore.
    let err = resolve(
        &node,
        &tara,
        ws,
        json!({ "from": "this-month", "to": "now", "now": NOW_MS }),
    )
    .await
    .unwrap_err();
    assert!(
        matches!(err, ToolError::BadInput(ref m) if m.contains("this-month")),
        "got {err:?}"
    );

    // An empty `from` and a bad tz are loud too.
    let err = resolve(&node, &tara, ws, json!({ "from": "", "now": NOW_MS }))
        .await
        .unwrap_err();
    assert!(matches!(err, ToolError::BadInput(ref m) if m.contains("empty")));
    let err = resolve(
        &node,
        &tara,
        ws,
        json!({ "from": "today", "tz": "Mars/Olympus", "now": NOW_MS }),
    )
    .await
    .unwrap_err();
    assert!(matches!(err, ToolError::BadInput(ref m) if m.contains("Mars/Olympus")));
}
