//! Core ingest round-trip + the resolved-design invariants (ingest scope): commit → typed read, the
//! exactly-once UPSERT key on a producer's replay, and the two-producer collision (BOTH survive).

use lb_ingest::{commit_direct, latest, read, Qos, Sample};
use lb_store::Store;

fn sample(series: &str, producer: &str, seq: u64, payload: serde_json::Value, qos: Qos) -> Sample {
    Sample {
        series: series.into(),
        producer: producer.into(),
        ts: seq, // logical ts; not wall-clock (determinism §3)
        seq,
        payload,
        labels: serde_json::json!({}),
        qos,
    }
}

/// Like [`sample`], but with `ts` set INDEPENDENTLY of `seq`.
///
/// `sample` above ties `ts: seq`, so the two axes can never disagree there — which is why no test in
/// this file could catch an ordering bug between them. The whole point of a producer restart is that
/// they DO disagree: seq goes backwards while the clock goes forwards.
fn sample_at(
    series: &str,
    producer: &str,
    seq: u64,
    ts: u64,
    payload: serde_json::Value,
) -> Sample {
    Sample {
        series: series.into(),
        producer: producer.into(),
        ts,
        seq,
        payload,
        labels: serde_json::json!({}),
        qos: Qos::BestEffort,
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn write_commit_read_round_trips_typed() {
    let store = Store::memory().await.unwrap();
    let samples = vec![
        sample("cpu", "pi-7", 1, serde_json::json!(61.4), Qos::BestEffort),
        sample(
            "cpu",
            "pi-7",
            2,
            serde_json::json!({"v": 62, "ok": true}),
            Qos::BestEffort,
        ),
    ];
    let pass = commit_direct(&store, "nube", &samples).await.unwrap();
    assert_eq!(pass.committed, 2);

    let got = read(&store, "nube", "cpu", None, None).await.unwrap();
    assert_eq!(got.len(), 2);
    // Typed payloads preserved (scalar stays a number; structured stays a nested object).
    assert_eq!(got[0].payload, serde_json::json!(61.4));
    assert_eq!(got[1].payload, serde_json::json!({"v": 62, "ok": true}));

    let last = latest(&store, "nube", "cpu").await.unwrap().unwrap();
    assert_eq!(last.seq, 2);
}

/// A producer whose in-memory `seq` restarts at 0 (any restarted process) must not pin
/// `series.latest` to its PRE-restart sample.
///
/// `latest` ordered by `seq DESC` across the whole series, but `seq` is monotonic per
/// `(series, producer)` ONLY — across producers those are two unrelated scales. So a new stream's
/// seq=0,1,2… lost to the old stream's seq=807 forever: a live meter read a stale value for hours
/// while fresh samples landed underneath it. `ts` is the only axis the streams share.
///
/// Found live in ems, not by a test: a FRESH series has no prior epoch, so no green e2e run could
/// reproduce it — only a series that outlives a sidecar restart, i.e. every real one.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn latest_follows_wall_clock_across_a_producer_restart() {
    let store = Store::memory().await.unwrap();

    // Epoch 1 ran a long time: its seq climbed high.
    let old = vec![
        sample_at(
            "v",
            "ext:modbus/net@1",
            806,
            1_000,
            serde_json::json!(230.0),
        ),
        sample_at(
            "v",
            "ext:modbus/net@1",
            807,
            1_001,
            serde_json::json!(230.9),
        ),
    ];
    commit_direct(&store, "nube", &old).await.unwrap();
    assert_eq!(latest(&store, "nube", "v").await.unwrap().unwrap().seq, 807);

    // Epoch 2: the process restarted — seq resets to 0, but the clock moved FORWARD.
    let new = vec![
        sample_at("v", "ext:modbus/net@2", 0, 9_000, serde_json::json!(239.8)),
        sample_at("v", "ext:modbus/net@2", 1, 9_001, serde_json::json!(240.1)),
    ];
    commit_direct(&store, "nube", &new).await.unwrap();

    let last = latest(&store, "nube", "v").await.unwrap().unwrap();
    assert_eq!(
        last.payload,
        serde_json::json!(240.1),
        "latest must follow the clock, not the (per-producer) seq — got seq={} producer={} ts={}",
        last.seq,
        last.producer,
        last.ts
    );
    assert_eq!(last.producer, "ext:modbus/net@2");
    assert_eq!(
        last.seq, 1,
        "seq still breaks ties WITHIN the newest stream"
    );

    // Both epochs' rows survive — a restart is not data loss (the two-producer guarantee).
    assert_eq!(
        read(&store, "nube", "v", None, None).await.unwrap().len(),
        4
    );
}

/// Within ONE producer batching several samples onto one `ts`, `seq` must still order them.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn latest_breaks_a_ts_tie_by_seq() {
    let store = Store::memory().await.unwrap();
    let s = vec![
        sample_at("v", "p", 1, 5_000, serde_json::json!("first")),
        sample_at("v", "p", 2, 5_000, serde_json::json!("second")),
    ];
    commit_direct(&store, "nube", &s).await.unwrap();
    let last = latest(&store, "nube", "v").await.unwrap().unwrap();
    assert_eq!(last.payload, serde_json::json!("second"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn commit_is_idempotent_on_a_producers_replay() {
    // A producer that lost an ack and re-pushes must not double-count: the UPSERT key is
    // [series, producer, seq], so the second commit lands on the same row.
    let store = Store::memory().await.unwrap();
    let s = vec![sample("m", "p", 5, serde_json::json!(1), Qos::MustDeliver)];
    commit_direct(&store, "nube", &s).await.unwrap();
    // Replay (a producer reconnecting after a lost ack): same sample again.
    commit_direct(&store, "nube", &s).await.unwrap();

    let got = read(&store, "nube", "m", None, None).await.unwrap();
    assert_eq!(got.len(), 1, "a replayed sample commits exactly once");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn two_producers_same_seq_both_survive() {
    // The resolved dedup identity is (series, producer, seq) — NOT (series, seq). Producer-A and
    // producer-B both writing seq=5 to ONE series must BOTH survive.
    let store = Store::memory().await.unwrap();
    let s = vec![
        sample(
            "shared",
            "prod-a",
            5,
            serde_json::json!("a"),
            Qos::MustDeliver,
        ),
        sample(
            "shared",
            "prod-b",
            5,
            serde_json::json!("b"),
            Qos::MustDeliver,
        ),
    ];
    commit_direct(&store, "nube", &s).await.unwrap();

    let got = read(&store, "nube", "shared", None, None).await.unwrap();
    assert_eq!(got.len(), 2, "both producers' seq=5 must coexist");
    let payloads: Vec<_> = got.iter().map(|s| s.payload.clone()).collect();
    assert!(payloads.contains(&serde_json::json!("a")));
    assert!(payloads.contains(&serde_json::json!("b")));
}
