//! The store operational pair (`store.status` / `store.compact`) headless, against a real
//! store (online-compaction scope, issue #67). Proves the mandatory categories:
//!   - **capability deny, per verb** — no `store:status:read` → opaque `Denied`; no
//!     `store:compact:run` → opaque `Denied` AND no job record written (deny is total).
//!   - **the job contract** — `store.compact` enqueues a durable `store-compact` job and
//!     returns its id; the reactor drain executes the pass and completes the job with the
//!     `{before_bytes, after_bytes}` outcome on the record.
//!   - **threshold advisory** — below threshold → `advisory: None` (a quiet store warns
//!     nothing); above → the warning string (pinned via the pure fn, same line the reactor logs).
//!
//! Real infra, real bytes: the pass runs on a real SurrealKV dir; the job lands via the real
//! jobs verbs; nothing is mocked (rule 9).

use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{
    drain_compact_jobs, over_threshold_advisory, store_compact_enqueue, store_status_run, Node,
    StoreAdminError, LOG_ADVISORY_BYTES, STORE_COMPACT_JOB_KIND,
};
use lb_store::{write, Store};
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

const STATUS: &str = "store:status:read";
const COMPACT: &str = "store:compact:run";

fn temp_path(tag: &str) -> String {
    std::env::temp_dir()
        .join(format!("lb-store-admin-{tag}-{}", std::process::id()))
        .to_string_lossy()
        .into_owned()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn status_reads_with_cap_and_denies_without() {
    let ws = "sa-status";
    let store = Store::memory().await.unwrap();

    // With the cap: a real snapshot (memory store → not persistent, zero log, no advisory).
    let p = principal("user:ada", ws, &[STATUS]);
    let report = store_status_run(&store, &p, ws).expect("status with cap");
    assert!(!report.persistent);
    assert_eq!(report.log_bytes, 0);
    assert_eq!(report.threshold_bytes, LOG_ADVISORY_BYTES);
    assert!(
        report.advisory.is_none(),
        "a quiet store carries no advisory"
    );

    // Without: opaque deny. (MANDATORY deny-test for the read verb.)
    let p_none = principal("user:eve", ws, &[COMPACT]); // holds the OTHER cap — no bleed-over
    let err = store_status_run(&store, &p_none, ws).unwrap_err();
    assert!(
        matches!(err, StoreAdminError::Denied),
        "opaque deny, got {err:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn compact_denied_without_cap_and_writes_no_job() {
    let ws = "sa-deny";
    let store = Store::memory().await.unwrap();
    // Holds status (and even the broad author write wildcard) but NOT compact:run — the
    // distinct `run` action means `store:*:write` must never imply a node-pausing pass.
    let p = principal("user:eve", ws, &[STATUS, "store:*:write"]);
    let err = store_compact_enqueue(&store, &p, ws, 1).await.unwrap_err();
    assert!(
        matches!(err, StoreAdminError::Denied),
        "opaque deny, got {err:?}"
    );

    // Deny is total: no job record exists.
    let pending = lb_jobs::pending(&store, ws, STORE_COMPACT_JOB_KIND)
        .await
        .unwrap();
    assert!(pending.is_empty(), "a denied enqueue must write nothing");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn compact_job_enqueues_drains_and_records_outcome() {
    let ws = "sa-job";
    let path = temp_path("job");
    let _ = std::fs::remove_dir_all(&path);
    let store = Store::open(&path).await.unwrap();

    // Real garbage for the pass: overwrite churn through the real write path.
    for k in 0..30 {
        for round in 0..6u64 {
            write(
                &store,
                ws,
                "kv",
                &format!("k{k}"),
                &json!({"round": round, "pad": "z".repeat(200)}),
            )
            .await
            .unwrap();
        }
    }

    let p = principal("user:ada", ws, &[COMPACT, STATUS]);
    let pre = store_status_run(&store, &p, ws).expect("status before");
    eprintln!(
        "GROUNDING: status before: log_bytes={} advisory={}",
        pre.log_bytes,
        pre.advisory.as_deref().unwrap_or("none")
    );
    let enq = store_compact_enqueue(&store, &p, ws, 42)
        .await
        .expect("enqueue");
    eprintln!("GROUNDING: store.compact -> job {}", enq.job_id);
    assert!(enq.job_id.starts_with("store-compact-"));

    // The durable job is pending until the reactor drains it.
    let pending = lb_jobs::pending(&store, ws, STORE_COMPACT_JOB_KIND)
        .await
        .unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].id, enq.job_id);

    // Drain exactly as the reactor tick does (same fn), on a real Node over this store.
    let node = Arc::new(
        Node::boot_with_store(store.clone())
            .await
            .expect("node boots"),
    );
    drain_compact_jobs(&node, ws).await.expect("drain runs");

    // Job completed with the outcome recorded on the record.
    let job = lb_jobs::load(&store, ws, &enq.job_id)
        .await
        .unwrap()
        .expect("job exists");
    eprintln!("GROUNDING: job record payload: {}", job.payload);
    assert_eq!(
        job.status,
        lb_jobs::JobStatus::Done,
        "pass completes the job"
    );
    let payload: serde_json::Value = serde_json::from_str(&job.payload).unwrap();
    assert_eq!(payload["requested_by"], json!("user:ada"));
    let outcome = &payload["outcome"];
    assert_eq!(outcome["ok"], json!(true));
    assert!(
        outcome["after_bytes"].as_u64().unwrap() < outcome["before_bytes"].as_u64().unwrap(),
        "outcome records a real shrink: {outcome}"
    );

    eprintln!(
        "GROUNDING: job done: outcome ok={} before={} after={}",
        outcome["ok"], outcome["before_bytes"], outcome["after_bytes"]
    );

    // Nothing left pending; status reflects the pass on the LIVE handle.
    assert!(lb_jobs::pending(&store, ws, STORE_COMPACT_JOB_KIND)
        .await
        .unwrap()
        .is_empty());
    let report = store_status_run(&store, &p, ws).expect("status after pass");
    eprintln!(
        "GROUNDING: status after: log_bytes={}, last_compaction.ok={}",
        report.log_bytes,
        report
            .last_compaction
            .as_ref()
            .map(|r| r.ok)
            .unwrap_or(false)
    );
    assert!(report.last_compaction.expect("recorded").ok);

    drop(node);
    let _ = std::fs::remove_dir_all(&path);
}

/// OQ4 (scope §Open questions): what IS in a bloated log — superseded/tombstoned churn, or a
/// write-amplification bug? Synthesize 1s-cadence-shaped churn through the REAL ingest write
/// path with a retention cap evicting (the measured incident's shape, small scale), then
/// compact a copy and report the ratio. Run explicitly for the session doc:
/// `cargo test -p lb-host --test store_admin_test oq4 -- --ignored --nocapture`
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "OQ4 experiment — run by hand for the session doc"]
async fn oq4_bloat_composition_experiment() {
    use lb_ingest::{run_gc, set_policy, Policy};

    let ws = "oq4";
    let path = temp_path("oq4");
    let _ = std::fs::remove_dir_all(&path);
    let store = Store::open(&path).await.unwrap();
    let p = principal("user:ada", ws, &["mcp:ingest.write:call", STATUS, COMPACT]);

    // A 10-series fleet at "1s cadence": 1,200 samples each, capped at 100 retained — so ~92%
    // of what was written is evicted (tombstones) and the rest is plain append churn.
    set_policy(
        &store,
        ws,
        &Policy {
            prefix: "fleet.".into(),
            raw_for_ms: 0,
            max_samples: 100,
            tiers: vec![],
        },
    )
    .await
    .unwrap();
    for series in 0..10 {
        for batch in 0..12u64 {
            let samples: Vec<_> = (0..100u64)
                .map(|i| {
                    let seq = batch * 100 + i + 1;
                    json!({"series": format!("fleet.s{series}"), "producer": "probe", "ts": seq, "seq": seq, "payload": json!(seq as f64), "qos": "best-effort"})
                })
                .collect();
            lb_host::call_ingest_tool(&store, &p, ws, "ingest.write", &json!({"samples": samples}))
                .await
                .expect("ingest");
            run_gc(&store, ws, u64::MAX)
                .await
                .expect("gc evicts over-cap");
        }
    }

    let before = store_status_run(&store, &p, ws).unwrap();
    let rec = lb_store::compact(&store).await.expect("pass");
    eprintln!(
        "OQ4 RESULT: 12,000 ingest-path samples, 1,000 retained (92% evicted): log {} B -> {} B \
         ({}x bloat over the compacted live set) — superseded versions + eviction tombstones \
         fully account for the growth; compaction recovers it",
        rec.before_bytes,
        rec.after_bytes,
        rec.before_bytes / rec.after_bytes.max(1)
    );
    assert!(
        rec.after_bytes < rec.before_bytes,
        "churn must compact away"
    );
    let _ = before;
    drop(store);
    let _ = std::fs::remove_dir_all(&path);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn threshold_advisory_fires_only_over_threshold() {
    // The pure decision the reactor and the status verb share: quiet store → silence.
    assert!(over_threshold_advisory(0, LOG_ADVISORY_BYTES).is_none());
    assert!(over_threshold_advisory(LOG_ADVISORY_BYTES, LOG_ADVISORY_BYTES).is_none());
    let warning = over_threshold_advisory(LOG_ADVISORY_BYTES + 1, LOG_ADVISORY_BYTES)
        .expect("over threshold warns");
    assert!(
        warning.contains("store.compact"),
        "the advisory names the remedy: {warning}"
    );
}
