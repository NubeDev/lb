//! Source-unit provenance end to end through `viz.query`, against a REAL node — no mocks. A
//! producer declares a `unit` label on ingest; the resolved frame's numeric fields carry that unit,
//! so the frame says what its numbers are IN.
//!
//! This is the gap that made lb's (correct, uom-backed) converter unreachable from a chart: nothing
//! on the data path carried a `from_unit`. `series_meta` had two columns, `series.list` returned
//! bare strings, and the viz `Field` had no unit at all — its own header promised "SI/base units"
//! while providing no way to declare which.
//!
//! What each test proves:
//!   - `a_declared_unit_reaches_the_frame` — the happy path, ingest label → frame field unit.
//!   - `an_undeclared_series_carries_no_unit` — absent means unknown; today's behaviour, unchanged.
//!   - `the_time_column_never_carries_a_unit` — a unit on an instant is meaningless.
//!   - `a_sql_target_carries_no_unit` — no series identity ⇒ no guess.
//!   - `values_are_not_converted` — the unit is PROVENANCE; the numbers stay canonical.

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{call_tool, Node};
use serde_json::{json, Value};
use std::sync::Arc;

const VIZ: &str = "mcp:viz.query:call";
const READ: &str = "mcp:series.read:call";
const QUERY: &str = "mcp:store.query:call";
const WRITE: &str = "mcp:ingest.write:call";

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

/// Seed samples through the REAL ingest write+drain path, optionally declaring a `unit` label.
async fn seed(
    node: &Arc<Node>,
    p: &Principal,
    ws: &str,
    series: &str,
    values: &[f64],
    labels: Value,
) {
    let samples: Vec<Value> = values
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let seq = (i + 1) as u64;
            json!({
                "series": series, "producer": "seed", "ts": seq * 1000, "seq": seq,
                "payload": v, "labels": labels, "qos": "best-effort"
            })
        })
        .collect();
    call_tool(
        node,
        p,
        ws,
        "ingest.write",
        &json!({ "samples": samples }).to_string(),
    )
    .await
    .expect("seed ingest");
}

async fn frames(node: &Arc<Node>, p: &Principal, ws: &str, panel: Value) -> Vec<Value> {
    let out = call_tool(
        node,
        p,
        ws,
        "viz.query",
        &json!({ "panel": panel, "now": 1 }).to_string(),
    )
    .await
    .expect("viz.query");
    let v: Value = serde_json::from_str(&out).expect("json");
    v["frames"].as_array().cloned().unwrap_or_default()
}

fn series_panel(series: &str) -> Value {
    json!({ "sources": [{ "refId": "A", "tool": "series.read", "args": { "series": series } }] })
}

/// The unit of the first field named `name`, if any.
fn unit_of<'a>(frame: &'a Value, name: &str) -> Option<&'a str> {
    frame["fields"]
        .as_array()?
        .iter()
        .find(|f| f["name"] == name)?
        .get("unit")?
        .as_str()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_declared_unit_reaches_the_frame() {
    let ws = "viz-unit";
    let node = Arc::new(Node::boot().await.unwrap());
    let p = principal("user:test", ws, &[VIZ, READ, WRITE]);
    seed(
        &node,
        &p,
        ws,
        "meter.main.power",
        &[1.0, 2.0, 3.0],
        json!({ "unit": "kilowatt" }),
    )
    .await;

    let f = frames(&node, &p, ws, series_panel("meter.main.power")).await;
    let frame = f.first().expect("one frame");
    assert_eq!(
        unit_of(frame, "payload"),
        Some("kilowatt"),
        "the value column carries the series' declared unit: {frame}"
    );
}

/// Absent means unknown — the ordinary case for every series that predates the column.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_undeclared_series_carries_no_unit() {
    let ws = "viz-unit-none";
    let node = Arc::new(Node::boot().await.unwrap());
    let p = principal("user:test", ws, &[VIZ, READ, WRITE]);
    seed(&node, &p, ws, "legacy.cpu", &[1.0, 2.0], json!({})).await;

    let f = frames(&node, &p, ws, series_panel("legacy.cpu")).await;
    let frame = f.first().expect("one frame");
    assert_eq!(unit_of(frame, "payload"), None);
}

/// A unit on an instant is meaningless — the time column must never be stamped.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_time_column_never_carries_a_unit() {
    let ws = "viz-unit-time";
    let node = Arc::new(Node::boot().await.unwrap());
    let p = principal("user:test", ws, &[VIZ, READ, WRITE]);
    seed(
        &node,
        &p,
        ws,
        "ahu.temp",
        &[20.0, 21.0],
        json!({ "unit": "celsius" }),
    )
    .await;

    let f = frames(&node, &p, ws, series_panel("ahu.temp")).await;
    let frame = f.first().expect("one frame");
    for field in frame["fields"].as_array().expect("fields") {
        if field["type"] == "time" {
            assert!(
                field.get("unit").and_then(Value::as_str).is_none(),
                "a time field must carry no unit: {field}"
            );
        }
    }
}

/// A `store.query` target names no series, so there is no provenance to look up — and the host must
/// not invent one. An absent unit is the honest answer.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_sql_target_carries_no_unit() {
    let ws = "viz-unit-sql";
    let node = Arc::new(Node::boot().await.unwrap());
    let p = principal("user:test", ws, &[VIZ, QUERY, WRITE, READ]);
    seed(
        &node,
        &p,
        ws,
        "sql.series",
        &[7.0],
        json!({ "unit": "volt" }),
    )
    .await;

    let panel = json!({
        "sources": [{
            "refId": "A", "tool": "store.query",
            "args": { "sql": "SELECT seq, payload FROM series ORDER BY seq" }
        }]
    });
    let f = frames(&node, &p, ws, panel).await;
    let frame = f.first().expect("one frame");
    assert_eq!(
        unit_of(frame, "payload"),
        None,
        "a SQL target has no series identity; the host must not guess: {frame}"
    );
}

/// **The unit is provenance, not a display decision.** Values stay canonical — converting in the
/// resolver would bake one viewer's prefs into shared (and cached) data.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn values_are_not_converted() {
    let ws = "viz-unit-raw";
    let node = Arc::new(Node::boot().await.unwrap());
    let p = principal("user:test", ws, &[VIZ, READ, WRITE]);
    seed(
        &node,
        &p,
        ws,
        "room.temp",
        &[21.5],
        json!({ "unit": "celsius" }),
    )
    .await;

    let f = frames(&node, &p, ws, series_panel("room.temp")).await;
    let frame = f.first().expect("one frame");
    let values = frame["fields"]
        .as_array()
        .expect("fields")
        .iter()
        .find(|x| x["name"] == "payload")
        .expect("payload field")["values"]
        .as_array()
        .expect("values")
        .clone();
    assert_eq!(
        values,
        vec![json!(21.5)],
        "the value is untouched — 21.5 stays 21.5, never 70.7°F"
    );
    assert_eq!(unit_of(frame, "payload"), Some("celsius"));
}
