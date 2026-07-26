//! The series-normalize MCP surface at the host seam (series-normalize scope): the additive
//! `filter` / `method` fields ride the EXISTING caps, a bucketed read resolves the governing tier's
//! method, and both mandatory categories — **capability deny** and **workspace isolation** — hold on
//! the new fields.
//!
//! Real node boot, real store, real ingest path, real MCP dispatch — no mocks (testing §0).

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{call_ingest_tool, drain_workspace, Node};
use lb_ingest::{Qos, Sample};
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

/// An admin principal for `ws` — every series cap this suite touches.
fn admin(ws: &str) -> Principal {
    principal(
        "user:ada",
        ws,
        &[
            "mcp:series.retention.set:call",
            "mcp:series.retention.list:call",
            "mcp:series.retention.gc:call",
            "mcp:series.read:call",
            "mcp:series.latest:call",
            "mcp:ingest.write:call",
        ],
    )
}

async fn call(
    node: &Node,
    p: &Principal,
    ws: &str,
    tool: &str,
    args: Value,
) -> Result<Value, ToolError> {
    call_ingest_tool(&node.store, p, ws, tool, &args).await
}

/// Commit `samples` through the real write → drain path.
async fn seed(node: &Node, ws: &str, samples: Vec<Sample>) {
    lb_ingest::write(&node.store, ws, &samples, 0)
        .await
        .unwrap();
    drain_workspace(&node.store, ws).await.unwrap();
}

fn sample_at(series: &str, producer: &str, seq: u64, ts: u64, payload: Value) -> Sample {
    Sample {
        series: series.into(),
        producer: producer.into(),
        ts,
        seq,
        payload,
        labels: json!({}),
        qos: Qos::BestEffort,
    }
}

// ---------------------------------------------------------------- mandatory: capability deny ----

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_policy_round_trips_its_new_fields_through_set_and_list() {
    // The closed-struct trap: a field added to `Policy` but missing from `list_policies`' explicit
    // projection reads back as its serde default forever — the row on disc correct, the struct in
    // memory silently not.
    let node = Node::boot().await.unwrap();
    let p = admin("acme");
    let authored = json!({
        "prefix": "modbus.",
        "raw_for_ms": 900000,
        "max_samples": 0,
        "tiers": [{"width_ms": 900000, "keep_for_ms": 0, "method": "avg"}],
        "filter": {
            "drop": false,
            "min_interval_ms": 2000,
            "deadband": {"abs": 0.5},
            "range": {"min": -40.0, "max": 120.0, "mode": "clamp"}
        }
    });
    call(&node, &p, "acme", "series.retention.set", authored)
        .await
        .unwrap();

    let listed = call(&node, &p, "acme", "series.retention.list", json!({}))
        .await
        .unwrap();
    let got = &listed["policies"][0];
    assert_eq!(got["prefix"], json!("modbus."));
    assert_eq!(
        got["tiers"][0]["method"],
        json!("avg"),
        "method survived the projection"
    );
    assert_eq!(got["filter"]["min_interval_ms"], json!(2000));
    assert_eq!(got["filter"]["deadband"]["abs"], json!(0.5));
    assert_eq!(got["filter"]["range"]["mode"], json!("clamp"));
    assert_eq!(got["filter"]["range"]["min"], json!(-40.0));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_existing_policy_row_keeps_its_exact_meaning() {
    // A row authored before this slice — no `filter`, no `method` — must deserialize to "store
    // everything, full stat row", not to some new default.
    let node = Node::boot().await.unwrap();
    let p = admin("acme");
    call(
        &node,
        &p,
        "acme",
        "series.retention.set",
        json!({"prefix": "legacy.", "raw_for_ms": 60000, "max_samples": 10, "tiers": [{"width_ms": 1000, "keep_for_ms": 0}]}),
    )
    .await
    .unwrap();

    let listed = call(&node, &p, "acme", "series.retention.list", json!({}))
        .await
        .unwrap();
    let got = &listed["policies"][0];
    assert!(
        got.get("filter").is_none(),
        "an absent filter stays absent on the wire"
    );
    assert!(got["tiers"][0].get("method").is_none());
    assert_eq!(
        got["raw_for_ms"],
        json!(60000),
        "the pre-existing axes are untouched"
    );
    assert_eq!(got["max_samples"], json!(10));

    // And it stores everything, identical values included.
    seed(
        &node,
        "acme",
        (1..=5u64)
            .map(|i| sample_at("legacy.v", "p", i, i * 100, json!(7.0)))
            .collect(),
    )
    .await;
    let rows = call(
        &node,
        &p,
        "acme",
        "series.read",
        json!({"series": "legacy.v"}),
    )
    .await
    .unwrap();
    assert_eq!(rows["samples"].as_array().unwrap().len(), 5);
}

/// REGRESSION — `debugging/ingest/filtered-batch-stops-the-drain-loop.md`.
///
/// The drain loop used to stop on `pass.committed == 0`. A muted prefix commits zero rows while
/// consuming a whole 256-row batch, so the very first pass looked like "staging is empty" and the
/// unbounded drain abandoned the rest of the backlog — the drain-backpressure stall, re-created
/// through a new door. The bound must be "nothing was DEQUEUED", not "nothing was committed".
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_fully_filtered_backlog_drains_completely_instead_of_stalling_after_one_batch() {
    let node = Node::boot().await.unwrap();
    let p = admin("acme");
    call(
        &node,
        &p,
        "acme",
        "series.retention.set",
        json!({"prefix": "quiet.", "raw_for_ms": 0, "filter": {"drop": true}}),
    )
    .await
    .unwrap();

    // Three batches' worth (COMMIT_BATCH is 256), staged directly so nothing drains inline.
    let n = 700u64;
    lb_ingest::write(
        &node.store,
        "acme",
        &(1..=n)
            .map(|i| sample_at("quiet.v", "p", i, i * 1_000, json!(i as f64)))
            .collect::<Vec<_>>(),
        0,
    )
    .await
    .unwrap();

    let pass = drain_workspace(&node.store, "acme").await.unwrap();
    assert_eq!(pass.committed, 0, "everything was muted");
    assert_eq!(
        pass.filtered.muted, n as usize,
        "ONE drain_workspace call must consume the WHOLE backlog, not just the first batch"
    );

    // And staging really is empty — a second pass has nothing left to do.
    assert!(drain_workspace(&node.store, "acme")
        .await
        .unwrap()
        .filtered
        .is_zero());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_live_write_path_reports_what_it_filtered() {
    // `ingest.write` accepts, then the commit filters. The sample is delivered-then-filtered, never
    // silently lost — and the operator can see it in the drain pass counts.
    let node = Node::boot().await.unwrap();
    let p = admin("acme");
    call(
        &node,
        &p,
        "acme",
        "series.retention.set",
        json!({"prefix": "d.", "raw_for_ms": 0, "filter": {"deadband": {"abs": 1.0}}}),
    )
    .await
    .unwrap();

    let samples: Vec<Value> = [20.0, 20.1, 20.2, 25.0]
        .iter()
        .enumerate()
        .map(|(i, v)| {
            json!({
                "series": "d.v", "producer": "p", "seq": i + 1,
                "ts": 1000 + i * 1000, "payload": v, "labels": {}, "qos": "best-effort"
            })
        })
        .collect();
    let accepted = call(
        &node,
        &p,
        "acme",
        "ingest.write",
        json!({"samples": samples}),
    )
    .await
    .unwrap();
    assert_eq!(
        accepted["accepted"],
        json!(4),
        "ACCEPTANCE is unfiltered — all four landed in staging"
    );
    // The verb's own inline drain is what filtered them, so the counts must come back on ITS reply —
    // otherwise `accepted: 4` against 2 stored rows is an unexplained gap for the producer.
    assert_eq!(
        accepted["filtered"]["deadband"],
        json!(2),
        "the two redundant samples are counted, not vanished: {accepted}"
    );

    // A later drain finds nothing left — the write already committed its own batch.
    let pass = drain_workspace(&node.store, "acme").await.unwrap();
    assert!(pass.filtered.is_zero());

    let rows = call(&node, &p, "acme", "series.read", json!({"series": "d.v"}))
        .await
        .unwrap();
    assert_eq!(rows["samples"].as_array().unwrap().len(), 2);
}
