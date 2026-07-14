//! The drain bound: one producer's `ingest.write` must not be billed for another producer's staging
//! backlog (drain-backpressure scope).
//!
//! The bug this pins: `ingest.write` used to call `drain_workspace` — which loops `commit_batch`
//! until staging is EMPTY — so a caller pushing ONE sample to a backlogged workspace paid to commit
//! every other producer's staged rows. Measured at pin node-v0.4.5 against a real store: one sample
//! against a 4,671-row backlog took 18.5s; the identical call at backlog 0 took 21ms.
//!
//! **The assertion is STRUCTURAL, not wall-clock**: the call commits at most `COMMIT_BATCH` rows
//! (plus its own). A timing bound is a flake generator on a loaded box (see the `rules_test`
//! under-load note in debugging/) and would not pin WHY the call is fast. Committed-count is the
//! honest, deterministic expression of "the caller pays for its own batch, not the backlog".
//!
//! The backlog is staged through the REAL `lb_ingest::write` path (no hand-rolled rows, no mocks)
//! against a REAL store — testing §0.

use std::sync::Arc;
use std::time::Duration;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{call_ingest_tool, drain_workspace, spawn_ingest_reactors, Node, COMMIT_BATCH};
use lb_ingest::{Qos, Sample};
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

/// Stage `n` rows for `producer` through the real staging append (the cheap path), bypassing the MCP
/// verb so the backlog exists BEFORE the call under test — exactly like a busy producer's rows.
async fn stage_backlog(store: &Store, ws: &str, producer: &str, n: u64) {
    let samples: Vec<Sample> = (1..=n)
        .map(|seq| Sample {
            series: "backlog.series".into(),
            producer: producer.into(),
            ts: 1_784_070_000_000 + seq,
            seq,
            payload: json!(seq),
            labels: Default::default(),
            qos: Qos::BestEffort,
        })
        .collect();
    lb_ingest::write(store, ws, &samples, 0)
        .await
        .expect("stage");
}

/// Count what is still staged — the backlog the caller must NOT have been billed for.
async fn staged_count(store: &Store, ws: &str) -> usize {
    let mut resp = store
        .query_ws(ws, "SELECT count() FROM ingest_staging GROUP ALL", vec![])
        .await
        .expect("count query");
    let rows: Vec<serde_json::Value> = resp.take(0).expect("count rows");
    rows.first().and_then(|r| r["count"].as_u64()).unwrap_or(0) as usize
}

/// THE HEADLINE REGRESSION. A one-sample write against a backlog far larger than one batch must
/// commit only a bounded slice — never the whole backlog.
///
/// Fails against the unbounded drain: that version commits ALL 2000 backlog rows + the sample
/// inside the call, leaving `staged_count == 0` and blowing the ceiling assertion below.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_write_is_not_billed_for_another_producers_backlog() {
    let store = Store::memory().await.unwrap();
    let ws = "acme";
    // A backlog several batches deep, staged by SOMEONE ELSE.
    const BACKLOG: u64 = 2_000;
    stage_backlog(&store, ws, "other:producer", BACKLOG).await;
    assert_eq!(staged_count(&store, ws).await, BACKLOG as usize);

    let p = principal("client:pi-7", ws, &["mcp:ingest.write:call"]);
    let out = call_ingest_tool(
        &store,
        &p,
        ws,
        "ingest.write",
        &json!({ "samples": [{
            "series": "debug.probe", "producer": "p", "ts": 1_784_070_000_000u64,
            "seq": 1, "payload": 1.0, "labels": {}, "qos": "must-deliver"
        }] }),
    )
    .await
    .expect("write");
    assert_eq!(out["accepted"], 1);

    // The caller committed AT MOST its own batch — the rest of the backlog is still staged.
    // This is the property: write cost is O(batch), not O(backlog).
    let remaining = staged_count(&store, ws).await;
    let committed_by_call = (BACKLOG as usize + 1) - remaining;
    assert!(
        committed_by_call <= COMMIT_BATCH,
        "the write committed {committed_by_call} rows — it must commit at most one batch \
         ({COMMIT_BATCH}); it is being billed for the workspace backlog"
    );
    assert!(
        remaining > 0,
        "the whole backlog drained inside the caller's call — the unbounded drain is back"
    );
}

/// The bound must not strand rows: a later drain (the reactor's job in production) still commits
/// everything exactly once. Bounding WHO pays must never change WHAT lands.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_bounded_write_strands_nothing() {
    let store = Store::memory().await.unwrap();
    let ws = "acme";
    const BACKLOG: u64 = 600;
    stage_backlog(&store, ws, "other:producer", BACKLOG).await;

    let p = principal(
        "client:pi-7",
        ws,
        &["mcp:ingest.write:call", "mcp:series.read:call"],
    );
    call_ingest_tool(
        &store,
        &p,
        ws,
        "ingest.write",
        &json!({ "samples": [{
            "series": "debug.probe", "producer": "p", "ts": 1_784_070_000_000u64,
            "seq": 1, "payload": 1.0, "labels": {}, "qos": "must-deliver"
        }] }),
    )
    .await
    .expect("write");

    // Drain the remainder the way the reactor does — off the caller's path.
    drain_workspace(&store, ws).await.expect("drain");
    assert_eq!(staged_count(&store, ws).await, 0, "staging fully drains");

    // Every backlog row landed, exactly once (the `(series, producer, seq)` UPSERT identity).
    let read = call_ingest_tool(
        &store,
        &p,
        ws,
        "series.read",
        &json!({ "series": "backlog.series", "limit": 5_000 }),
    )
    .await
    .expect("read");
    assert_eq!(
        read["samples"].as_array().unwrap().len(),
        BACKLOG as usize,
        "every backlogged sample committed exactly once"
    );

    // A re-drain is a no-op — bounding the caller did not break exactly-once.
    assert_eq!(drain_workspace(&store, ws).await.unwrap().committed, 0);
}

/// The reactor must actually DRAIN — not merely be spawnable.
///
/// The bounded caller-drain is only half the fix: without a driver the backlog would strand forever.
/// So this asserts the OUTCOME (staging reaches 0 with no caller ever draining), never that a spawn
/// function was called. A test asserting a plan is wired never proves it executes — the exact trap
/// the native-boot-respawn fix was written to close.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_ingest_reactor_drains_the_backlog_with_no_caller_involved() {
    let node = Arc::new(Node::boot().await.unwrap());
    let ws = "acme";
    // A backlog several batches deep — more than any single bounded caller-drain could clear.
    stage_backlog(&node.store, ws, "other:producer", 1_000).await;
    assert_eq!(staged_count(&node.store, ws).await, 1_000);

    // Boot the reactor exactly as `node/src/reactors.rs` does. Nobody calls drain after this point.
    spawn_ingest_reactors(
        node.clone(),
        vec![ws.to_string()],
        Duration::from_millis(50),
    );

    // Poll for the OUTCOME rather than sleeping a fixed span (a fixed sleep is either flaky or slow).
    let mut drained = false;
    for _ in 0..100 {
        tokio::time::sleep(Duration::from_millis(50)).await;
        if staged_count(&node.store, ws).await == 0 {
            drained = true;
            break;
        }
    }
    assert!(
        drained,
        "the ingest reactor never drained the backlog — staging still holds {} rows. \
         The commit worker has no driver (the bug this reactor exists to fix).",
        staged_count(&node.store, ws).await
    );
}

/// The reactor is workspace-scoped: it drains ONLY the workspaces it was configured for. The hard
/// wall (rule 6) holds for the background path exactly as for a caller's.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_ingest_reactor_only_drains_its_configured_workspace() {
    let node = Arc::new(Node::boot().await.unwrap());
    stage_backlog(&node.store, "ws-a", "p:a", 300).await;
    stage_backlog(&node.store, "ws-b", "p:b", 300).await;

    // Configured for ws-a ONLY.
    spawn_ingest_reactors(
        node.clone(),
        vec!["ws-a".to_string()],
        Duration::from_millis(50),
    );

    let mut a_drained = false;
    for _ in 0..100 {
        tokio::time::sleep(Duration::from_millis(50)).await;
        if staged_count(&node.store, "ws-a").await == 0 {
            a_drained = true;
            break;
        }
    }
    assert!(a_drained, "the reactor must drain its configured workspace");
    assert_eq!(
        staged_count(&node.store, "ws-b").await,
        300,
        "ws-B's staging must be untouched — the reactor is workspace-scoped, not node-global"
    );
}

/// The round-trip the bounded drain must PRESERVE: a caller's own sample is visible to its very next
/// read over the same bridge, with no explicit drain — even with a backlog ahead of it. This is the
/// property the synchronous drain was deliberately bought for (`tool.rs`), and the one a naive
/// "just delete the drain" fix silently breaks.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_writers_own_sample_reads_back_immediately_despite_a_backlog() {
    let store = Store::memory().await.unwrap();
    let ws = "acme";
    stage_backlog(&store, ws, "other:producer", 900).await;

    let p = principal(
        "client:pi-7",
        ws,
        &["mcp:ingest.write:call", "mcp:series.latest:call"],
    );
    call_ingest_tool(
        &store,
        &p,
        ws,
        "ingest.write",
        &json!({ "samples": [{
            "series": "debug.probe", "producer": "p", "ts": 1_784_070_000_000u64,
            "seq": 1, "payload": 42.5, "labels": {}, "qos": "must-deliver"
        }] }),
    )
    .await
    .expect("write");

    // No explicit drain between the write and the read — the bridge round-trip.
    let latest = call_ingest_tool(
        &store,
        &p,
        ws,
        "series.latest",
        &json!({ "series": "debug.probe" }),
    )
    .await
    .expect("latest");
    assert_eq!(
        latest["sample"]["payload"], 42.5,
        "a writer's own sample must be visible to its very next read over the same bridge, \
         backlog or not"
    );
}
