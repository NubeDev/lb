//! The prefs surface through the REAL MCP bridge (`lb_host::call_tool`) — the same entry the
//! gateway's `POST /mcp/call` forwards. Proves the grant-free utility tier dispatches WITHOUT a cap,
//! the gated verbs round-trip end to end, and a gated verb is denied opaquely without its grant.

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{call_tool, Node};
use lb_mcp::ToolError;
use serde_json::{json, Value};
use std::sync::Arc;

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

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn format_and_convert_are_grant_free() {
    let ws = "prefs-util";
    let node = Arc::new(Node::boot().await.unwrap());
    // A caller with NO capabilities at all can still use the utility tier (pure math, no tenant data).
    let nobody = principal("user:eve", ws, &[]);

    // convert.unit: 0 °C -> 32 °F (affine).
    let out = call_tool(
        &node,
        &nobody,
        ws,
        "convert.unit",
        &json!({ "value": 0.0, "from": "celsius", "to": "fahrenheit" }).to_string(),
    )
    .await
    .expect("convert.unit is grant-free");
    let v: Value = serde_json::from_str(&out).unwrap();
    assert!((v["value"].as_f64().unwrap() - 32.0).abs() < 1e-6);

    // format.quantity: 12 m/s wind, es number format -> "43,2 km/h".
    let prefs = json!({
        "language": "es", "timezone": "Europe/Madrid", "date_style": "eu", "time_style": "h24",
        "first_day_of_week": "monday", "number_format": "comma_dot", "unit_system": "metric",
        "unit_overrides": {}
    });
    let out = call_tool(
        &node,
        &nobody,
        ws,
        "format.quantity",
        &json!({ "value": 12.0, "from_unit": "meter_per_second", "dimension": "speed", "prefs": prefs })
            .to_string(),
    )
    .await
    .expect("format.quantity is grant-free");
    let v: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["text"], "43,2 km/h");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn prefs_set_then_resolve_round_trips_through_the_bridge() {
    let ws = "prefs-rt";
    let node = Arc::new(Node::boot().await.unwrap());
    let ada = principal(
        "user:ada",
        ws,
        &["mcp:prefs.set:call", "mcp:prefs.resolve:call"],
    );

    call_tool(
        &node,
        &ada,
        ws,
        "prefs.set",
        &json!({ "patch": { "language": "es", "unit_system": "imperial" } }).to_string(),
    )
    .await
    .expect("prefs.set with the grant");

    let out = call_tool(&node, &ada, ws, "prefs.resolve", "{}")
        .await
        .expect("prefs.resolve with the grant");
    let v: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["resolved"]["language"], "es");
    assert_eq!(v["resolved"]["unit_system"], "imperial");
    // an unset axis falls back to the built-in.
    assert_eq!(v["resolved"]["timezone"], "UTC");

    // A self-scoped request override previews without writing the record.
    let out = call_tool(
        &node,
        &ada,
        ws,
        "prefs.resolve",
        &json!({ "override": { "language": "en" } }).to_string(),
    )
    .await
    .unwrap();
    let v: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["resolved"]["language"], "en");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn prefs_set_default_denied_without_admin_cap_via_bridge() {
    let ws = "prefs-admin";
    let node = Arc::new(Node::boot().await.unwrap());
    // Holds set/resolve but NOT the admin set_default cap.
    let member = principal("user:bob", ws, &["mcp:prefs.set:call"]);
    let err = call_tool(
        &node,
        &member,
        ws,
        "prefs.set_default",
        &json!({ "patch": { "unit_system": "imperial" } }).to_string(),
    )
    .await
    .expect_err("set_default without the admin cap is denied");
    assert!(
        matches!(err, ToolError::Denied),
        "opaque denial, got {err:?}"
    );
}
