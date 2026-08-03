//! The schedule payload's **named window** (relative-time-range scope, step 7) through the REAL
//! MCP bridge (`lb_host::call_tool`) over a booted `Node` — real store, real caps wall, no mocks.
//! A reminder action carrying `{"range":{"from":…}}` — the ONE named-window form — is validated at
//! SAVE time (a bad expression fails with a human watching, never at 03:00), and an update is
//! judged identically. The removed legacy `{"preset":…}` key is REFUSED, naming its replacement.
//! The fire-time resolution itself (concrete ISO days injected into the payload) and the fire-time
//! `preset` refusal are pinned by the unit tests in `reminder/range.rs`.

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

const CAPS: &[&str] = &[
    "mcp:reminder.create:call",
    "mcp:reminder.update:call",
    "mcp:reminder.get:call",
];

/// Mon 2024-01-01 00:00 UTC.
const NOW: u64 = 1_704_067_200;

async fn create(
    node: &Arc<Node>,
    p: &Principal,
    ws: &str,
    id: &str,
    action: Value,
) -> Result<Value, ToolError> {
    let out = call_tool(
        node,
        p,
        ws,
        "reminder.create",
        &json!({ "id": id, "schedule": "0 8 * * 1", "action": action, "ts": NOW }).to_string(),
    )
    .await?;
    Ok(serde_json::from_str(&out).unwrap())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_named_window_is_validated_at_save_time() {
    let ws = "rem-range";
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let p = principal("user:tara", ws, CAPS);

    // A good `range` saves — the MCP-TOOL form.
    create(
        &node,
        &p,
        ws,
        "monthly",
        json!({ "kind": "mcp-tool", "tool": "report.export",
                "args": { "reportId": "energy", "range": { "from": "last-month" } } }),
    )
    .await
    .expect("a resolvable range saves");

    // …and the OUTBOX form (the shipped report schedule's shape), with a tz.
    create(
        &node,
        &p,
        ws,
        "weekly",
        json!({ "kind": "outbox", "target": "report", "action": "render",
                "payload": json!({ "reportId": "energy",
                                    "range": { "from": "last-7-days", "tz": "Australia/Sydney" } }).to_string() }),
    )
    .await
    .expect("a resolvable outbox range saves");

    // The REMOVED `preset` key is refused, naming the dead key and its replacement — never a
    // silent ignore that would leave a reminder looking configured while it mails a fallback
    // window nightly. (The legacy preset vocabulary was dropped by decision: not in production.)
    let err = create(
        &node,
        &p,
        ws,
        "preset-refused",
        json!({ "kind": "outbox", "target": "report", "action": "render",
                "payload": json!({ "reportId": "energy", "preset": "last-7-days" }).to_string() }),
    )
    .await
    .unwrap_err();
    let ToolError::BadInput(ref msg) = err else {
        panic!("expected BadInput, got {err:?}")
    };
    assert!(msg.contains("preset"), "names the dead key: {msg}");
    assert!(msg.contains("range:"), "names the replacement: {msg}");
    let out = call_tool(
        &node,
        &p,
        ws,
        "reminder.get",
        &json!({ "id": "preset-refused" }).to_string(),
    )
    .await
    .unwrap();
    let got: Value = serde_json::from_str(&out).unwrap();
    assert!(
        got["reminder"].is_null(),
        "a refused preset create stores nothing"
    );

    // A BAD range expression is refused at SAVE, naming the token — and nothing is stored.
    let err = create(
        &node,
        &p,
        ws,
        "broken",
        json!({ "kind": "mcp-tool", "tool": "report.export",
                "args": { "range": { "from": "last-fortnight" } } }),
    )
    .await
    .unwrap_err();
    assert!(
        matches!(err, ToolError::BadInput(ref m) if m.contains("last-fortnight")),
        "a bad range must refuse the save naming the token, got {err:?}"
    );
    let out = call_tool(
        &node,
        &p,
        ws,
        "reminder.get",
        &json!({ "id": "broken" }).to_string(),
    )
    .await
    .unwrap();
    let got: Value = serde_json::from_str(&out).unwrap();
    assert!(got["reminder"].is_null(), "a refused create stores nothing");

    // The MCP-TOOL carrier is judged identically — the refusal keys on the payload KEY, not on the
    // action kind or the tool name (rule 10).
    let err = create(
        &node,
        &p,
        ws,
        "badpreset",
        json!({ "kind": "mcp-tool", "tool": "report.export",
                "args": { "reportId": "energy", "preset": "last-7-days" } }),
    )
    .await
    .unwrap_err();
    assert!(matches!(err, ToolError::BadInput(ref m) if m.contains("preset")));

    // An UPDATE swapping in a bad window is judged the same as create — the stored action survives.
    let err = call_tool(
        &node,
        &p,
        ws,
        "reminder.update",
        &json!({ "id": "monthly", "ts": NOW + 60,
                 "action": { "kind": "mcp-tool", "tool": "report.export",
                             "args": { "range": { "from": "this-month", "to": "now" } } } })
        .to_string(),
    )
    .await
    .unwrap_err();
    assert!(
        matches!(err, ToolError::BadInput(ref m) if m.contains("this-month")),
        "a range token with a `to` refuses the update, got {err:?}"
    );
    let out = call_tool(
        &node,
        &p,
        ws,
        "reminder.get",
        &json!({ "id": "monthly" }).to_string(),
    )
    .await
    .unwrap();
    let got: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(
        got["reminder"]["action"]["args"]["range"]["from"], "last-month",
        "a refused update leaves the stored window untouched"
    );
}
