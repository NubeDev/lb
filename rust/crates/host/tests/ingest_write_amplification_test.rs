//! Ingest write amplification: the caller path must cost ~ONE commit-log append per sample, not
//! three (compaction-write-availability scope, lever 1).
//!
//! Why this matters: the engine under the store is append-only, so the commit log is the physical
//! consequence — and the log is simultaneously the node's RSS high-water mark (~1.4× log bytes for
//! key-dense samples) and the input to the stop-the-world compaction pass whose ~94 s pause blocked
//! `ingest.write` on RC-6. Fewer writes per sample stretches all three out together.
//!
//! The assertions are STRUCTURAL — record writes, not log bytes. See
//! `the_staged_path_writes_three_records_per_sample_and_the_direct_path_one` for why a byte meter
//! was tried and rejected. No mocks and no fixtures: real nodes, the real `ingest_write` path, and
//! the real stage-then-`commit_batch` path (testing §0).

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{ingest_write, Node};
use lb_ingest::{commit_batch, Qos, Sample, STAGING_TABLE};
use lb_store::Store;

const WS: &str = "nube";

fn principal(sub: &str) -> Principal {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: sub.into(),
        ws: WS.into(),
        role: Role::Member,
        caps: vec!["mcp:ingest.write:call".into()],
        iat: 0,
        exp: u64::MAX,
        constraint: None,
        run_id: None,
    };
    let token = mint(&key, &claims);
    verify(&key, &token, 1).expect("token verifies")
}

fn samples(n: usize) -> Vec<Sample> {
    (0..n)
        .map(|i| Sample {
            series: "meter.kwh".into(),
            producer: String::new(),
            ts: 1_700_000_000_000 + i as u64,
            seq: i as u64,
            // A realistic modbus-shaped reading, not a bare integer: the amplification ratio is a
            // property of the record's size, so a toy payload would flatter the result.
            payload: serde_json::json!({ "v": 240.5 + i as f64, "unit": "kWh", "q": "good" }),
            labels: serde_json::json!({}),
            qos: Qos::MustDeliver,
        })
        .collect()
}

async fn staged_rows(store: &Store) -> usize {
    let mut resp = store
        .query_ws(
            WS,
            &format!("SELECT count() FROM {STAGING_TABLE} GROUP ALL"),
            vec![],
        )
        .await
        .expect("count staging");
    let rows: Vec<serde_json::Value> = resp.take(0).expect("decode");
    rows.first()
        .and_then(|r| r.get("count"))
        .and_then(|c| c.as_u64())
        .unwrap_or(0) as usize
}

async fn series_rows(store: &Store) -> usize {
    lb_ingest::read(store, WS, "meter.kwh", None, None)
        .await
        .expect("read series")
        .len()
}

/// The direct path: `ingest_write` into a drained workspace never touches staging, and the samples
/// are committed — not merely accepted — by the time the call returns.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_batch_into_empty_staging_commits_without_staging_it() {
    let node = Node::boot().await.expect("node");
    let p = principal("ext:modbus");

    let accepted = ingest_write(&node.store, &p, WS, samples(10))
        .await
        .expect("write");
    assert_eq!(accepted, 10);

    assert_eq!(
        staged_rows(&node.store).await,
        0,
        "nothing was staged — no staging row and therefore no tombstone to write later"
    );
    let got = lb_ingest::read(&node.store, WS, "meter.kwh", None, None)
        .await
        .expect("read");
    assert_eq!(
        got.len(),
        10,
        "the samples are COMMITTED when the call returns, not pending a drain"
    );
}

/// The staged path is still there, and still the one taken whenever something is queued ahead: a
/// backlog must commit in order, never be jumped. This is the guard on the direct path's precondition
/// — the relief staging exists for (bursts, offline re-appends, crash recovery) is untouched, because
/// in every one of those cases staging is by definition not empty.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_batch_arriving_behind_a_backlog_still_stages() {
    let node = Node::boot().await.expect("node");
    let p = principal("ext:modbus");

    // A backlog nobody has drained yet, put there through the real staging path.
    lb_ingest::write(&node.store, WS, &samples(5), 0)
        .await
        .expect("seed backlog");
    assert_eq!(staged_rows(&node.store).await, 5);

    ingest_write(&node.store, &p, WS, samples(10))
        .await
        .expect("write");

    assert_eq!(
        staged_rows(&node.store).await,
        15,
        "the incoming batch queued behind the backlog instead of committing past it"
    );
}

/// The quantity, counted in RECORD WRITES rather than log bytes.
///
/// This is the assertion that says the direct path costs ~one commit-log append per sample where the
/// staged path costs three, and it is deliberately structural. Measuring the commit log in bytes was
/// tried first and abandoned: surrealkv flushes asynchronously, and the surrealdb index-builder leak
/// documented in `store/compact.rs` means the engine handle is never fully shut down inside a test
/// process, so the on-disk size at any moment is a function of machine load, not of what was
/// written. The same 2400-sample run measured 667,665 B and 1,647,313 B on the same code minutes
/// apart. A meter that moves 2.5× under load cannot pin anything, and a green test built on it would
/// be worse than no test.
///
/// What IS deterministic is the record-level bookkeeping, which is where the amplification lives:
/// the staged path creates a staging row per sample and then deletes it (a tombstone — on an
/// append-only engine a delete is a write), on top of the `series` row both paths write. Three
/// record writes; the direct path writes one. This test pins each of those three events.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_staged_path_writes_three_records_per_sample_and_the_direct_path_one() {
    const N: usize = 40;

    // The staged path, step by step.
    {
        let node = Node::boot().await.expect("node");
        let mut stamped = samples(N);
        for s in &mut stamped {
            s.producer = "ext:modbus".into();
        }

        // WRITE 1 of 3 — a durable staging row per sample.
        lb_ingest::write(&node.store, WS, &stamped, 0)
            .await
            .expect("stage");
        assert_eq!(
            staged_rows(&node.store).await,
            N,
            "one staging row per sample"
        );
        assert_eq!(
            series_rows(&node.store).await,
            0,
            "and nothing committed yet — the sample is stored, then stored again"
        );

        // WRITES 2 and 3 — the series row, and the staging row's tombstone, in one transaction.
        let pass = commit_batch(&node.store, WS, 256).await.expect("commit");
        assert_eq!(pass.committed, N);
        assert_eq!(series_rows(&node.store).await, N, "the series row: write 2");
        assert_eq!(
            staged_rows(&node.store).await,
            0,
            "the staging row is gone — deleted, which on an append-only engine is write 3"
        );
    }

    // The direct path: write 1 only, and it is the one that keeps the data.
    {
        let node = Node::boot().await.expect("node");
        let p = principal("ext:modbus");

        ingest_write(&node.store, &p, WS, samples(N))
            .await
            .expect("direct write");

        assert_eq!(
            series_rows(&node.store).await,
            N,
            "the series row: the only write"
        );
        assert_eq!(
            staged_rows(&node.store).await,
            0,
            "no staging row was ever created, so none had to be tombstoned"
        );
    }
}

/// A large push is several bounded transactions, not one enormous one.
///
/// The staged drain has always committed in `COMMIT_BATCH`-sized transactions, on the stated grounds
/// that a single transaction must stay bounded; the direct path inherits that rule via
/// `lb_ingest::DIRECT_COMMIT_BATCH` rather than quietly building a statement per sample for a push
/// of any size. This pins the outcome — every sample of an oversized push lands exactly once — which
/// is what makes chunking free to do.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_oversized_push_commits_every_sample_across_several_transactions() {
    let n = lb_ingest::DIRECT_COMMIT_BATCH * 3 + 7;
    let node = Node::boot().await.expect("node");
    let p = principal("ext:modbus");

    let accepted = ingest_write(&node.store, &p, WS, samples(n))
        .await
        .expect("direct write");

    assert_eq!(accepted, n);
    assert_eq!(
        series_rows(&node.store).await,
        n,
        "no sample fell between chunks"
    );
    assert_eq!(staged_rows(&node.store).await, 0);

    // Exactly-once holds ACROSS the chunk boundaries: the identical push re-applies to the same
    // `[series, producer, seq]` keys and creates nothing new. This is the property that makes a
    // multi-transaction push safe to retry after a failure part-way through it.
    ingest_write(&node.store, &p, WS, samples(n))
        .await
        .expect("re-push");
    assert_eq!(
        series_rows(&node.store).await,
        n,
        "a re-push double-counts nothing"
    );
}
