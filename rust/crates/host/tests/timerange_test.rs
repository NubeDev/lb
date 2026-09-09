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
//!   - THE TIMEZONE DEFAULT: an omitted `tz` follows the CALLER's resolved `prefs.timezone` rather
//!     than silently meaning UTC; an explicit `tz` still wins; a caller with no preference still
//!     gets UTC.
//!
//! Workspace isolation for the FIELD this verb serves (`Dashboard.time`) lives in
//! `dashboard_test.rs::workspace_isolation`. The resolution is pure compute over the caller's own
//! expression; the ONE per-caller read is the prefs chain that defaults an omitted `tz`.

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

/// **The timezone default.** `resolve_range` has always done DST-correct calendar snapping and has
/// always taken a `tz` — it just never learned who was asking, so "today" meant "today in
/// Greenwich" for everyone. Meanwhile `prefs.timezone` was resolved by the node and read in exactly
/// ONE place (`format.datetime`). This wires the two together.
///
/// The instant chosen is 2026-07-30T00:30:00Z — half past midnight UTC, which is already 10:30am on
/// the 30th in Sydney. UTC and Sydney therefore disagree about what "today" is at that moment,
/// which is what makes the assertion meaningful rather than a coincidence.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_omitted_tz_follows_the_callers_prefs() {
    let ws = "ws-tz-default";
    let node = Arc::new(Node::boot().await.expect("node boots"));
    const AT: i64 = 1_785_371_400_000; // 2026-07-30T00:30:00Z == 2026-07-30T10:30+10:00

    let caps = &[
        "mcp:time.range.resolve:call",
        "mcp:prefs.set:call",
        "mcp:prefs.resolve:call",
    ];
    let sydney = principal("user:sydney", ws, caps);

    // Before declaring a preference: UTC, exactly as this verb has always behaved.
    let out = resolve(&node, &sydney, ws, json!({ "from": "today", "now": AT }))
        .await
        .expect("resolves with no tz");
    assert_eq!(
        out["fromIso"], "2026-07-30",
        "no preference ⇒ UTC (today's behaviour, unchanged)"
    );

    // Declare a timezone preference, then ask the same question the same way.
    call_tool(
        &node,
        &sydney,
        ws,
        "prefs.set",
        &json!({ "patch": { "timezone": "Australia/Sydney" } }).to_string(),
    )
    .await
    .expect("prefs.set");

    let out = resolve(&node, &sydney, ws, json!({ "from": "today", "now": AT }))
        .await
        .expect("resolves with no tz");
    assert_eq!(
        out["fromIso"], "2026-07-30",
        "Sydney's day, snapped in Sydney"
    );
    assert_eq!(out["toIso"], "2026-07-31", "exclusive end, Sydney's day");
    // The window must START at Sydney midnight (14:00 UTC the previous day), NOT UTC midnight.
    assert_eq!(
        out["fromMs"].as_i64().unwrap(),
        1_785_333_600_000,
        "2026-07-29T14:00:00Z is 2026-07-30T00:00+10:00 — the caller's midnight, not Greenwich's"
    );

    // An EXPLICIT tz still wins over the preference — a caller that names a zone means it.
    let out = resolve(
        &node,
        &sydney,
        ws,
        json!({ "from": "today", "tz": "UTC", "now": AT }),
    )
    .await
    .expect("explicit tz resolves");
    assert_eq!(
        out["fromMs"].as_i64().unwrap(),
        1_785_369_600_000,
        "explicit UTC overrides the Sydney preference"
    );

    // A DIFFERENT caller in the same workspace is unaffected — the default is per-CALLER, not
    // per-workspace, so one person's preference cannot move another person's window.
    let other = principal("user:other", ws, caps);
    let out = resolve(&node, &other, ws, json!({ "from": "today", "now": AT }))
        .await
        .expect("resolves for a second caller");
    assert_eq!(
        out["fromMs"].as_i64().unwrap(),
        1_785_369_600_000,
        "a colleague's timezone preference must not move my window"
    );
}
