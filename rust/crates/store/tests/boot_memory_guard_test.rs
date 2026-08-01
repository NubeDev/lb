//! The boot memory guard (boot-memory-guard scope, issue #128): boot compaction becomes
//! conditional, a hopeless open is refused instead of OOMing the machine, and the pass record
//! survives the restart at which it matters.
//!
//! **No mocks (rule 9).** Every store here is a real SurrealKV store on a real temp path, written
//! through the real write path. The only thing injected is the *number* the guard compares against
//! (`OpenOptions::with_available_ram`) — feeding a real integer to a real pure function is not a
//! fake backend, and it is what lets the gigabyte-scale judgements be tested without seeding 617 MB
//! per case (the scope's testing plan says so explicitly).

use std::sync::{Arc, Mutex};

use lb_store::{read, write, OpenOptions, Store, StoreError};

/// A fresh on-disk store path inside its own parent directory — the sidecar record lives at
/// `<store dir>/../last-compaction.json`, so each case owns the whole parent.
fn temp_case(tag: &str) -> (std::path::PathBuf, String) {
    let parent = std::env::temp_dir().join(format!("lb-bootguard-{tag}-{}", lb_store::new_ulid()));
    let store = parent.join("store");
    std::fs::create_dir_all(&parent).unwrap();
    let s = store.to_string_lossy().into_owned();
    (parent, s)
}

/// Seed real records through the real write path, so the store on disk is a real one.
async fn seed(store: &Store, rounds: u64) {
    for round in 0..rounds {
        for k in 0..16u64 {
            write(
                store,
                "guard",
                "kv",
                &format!("k{k}"),
                &serde_json::json!({ "round": round, "pad": "x".repeat(512) }),
            )
            .await
            .unwrap();
        }
    }
}

/// The commit-log size of the store at `path`, measured with the same `log_stats` the guards use,
/// by reading it off a live handle (opened with the guards disabled so the measurement itself never
/// runs a pass or trips a refusal).
async fn log_bytes_of(path: &str) -> u64 {
    let opts = OpenOptions::default()
        .with_available_ram(Some(u64::MAX))
        .allow_unguarded(true);
    let store = Store::open_with(path, &opts).await.unwrap();
    let n = lb_store::status(&store).log_bytes;
    drop(store);
    n
}

/// An "available RAM" figure that declines the boot PASS (log > 0.5x) while leaving the OPEN
/// permitted (log <= 1.0x) — the incident box's own situation, and the scope's expected outcome.
fn skip_but_open(log_bytes: u64) -> Option<u64> {
    Some(log_bytes * 3 / 2)
}

/// Collect this test's `tracing` output so "the skip is LOUD" is asserted on the real log line,
/// not merely on the returned record.
#[derive(Clone, Default)]
struct LogSink(Arc<Mutex<Vec<u8>>>);

impl LogSink {
    fn text(&self) -> String {
        String::from_utf8_lossy(&self.0.lock().unwrap()).into_owned()
    }
}

impl std::io::Write for LogSink {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for LogSink {
    type Writer = LogSink;
    fn make_writer(&'a self) -> LogSink {
        self.clone()
    }
}

/// A boot pass the machine cannot afford is SKIPPED — loudly, with every number in the line — and
/// the node still opens and serves. Reverting the precondition makes this fail: today's `open` ran
/// the pass unconditionally and recorded `skipped: None`.
// `current_thread`: a scoped `tracing` subscriber is thread-local, so the assertion "the warn line
// actually reached a subscriber" only means something when the caller's thread is the one awaiting.
// (`open_with` carries that dispatcher onto the blocking pass thread — that is what is under test.)
#[tokio::test]
async fn skip_is_loud_and_recorded_and_the_node_still_opens() {
    let (parent, path) = temp_case("skip-loud");
    {
        let store = Store::open(&path).await.unwrap();
        seed(&store, 3).await;
    }

    // Pick a RAM figure from the real measured log: enough that the open is allowed, too little for
    // the pass (the incident box's exact position).
    let log_bytes = log_bytes_of(&path).await;
    let avail = skip_but_open(log_bytes).unwrap();

    let sink = LogSink::default();
    let subscriber = tracing_subscriber::fmt()
        .with_writer(sink.clone())
        .with_max_level(tracing::Level::WARN)
        .finish();

    let store = {
        let _guard = tracing::subscriber::set_default(subscriber);
        let opts = OpenOptions::default().with_available_ram(Some(avail));
        Store::open_with(&path, &opts).await.expect("still opens")
    };

    let snap = lb_store::status(&store);
    let rec = snap
        .last_compaction
        .expect("a boot record is always produced");
    let rec_log_bytes = rec.before_bytes;
    let reason = rec.skipped.expect("the pass was skipped, and says why");
    assert!(!rec.ok, "a skip did not compact anything");
    assert!(rec.error.is_none(), "a skip is a decision, not a failure");
    assert!(
        reason.contains("available RAM") && reason.contains(&avail.to_string()),
        "the reason names the numbers: {reason}"
    );

    let logged = sink.text();
    assert!(
        logged.contains("SKIPPING the boot compaction pass"),
        "the skip must be loud at WARN, got: {logged}"
    );
    assert!(
        logged.contains(&avail.to_string()) && logged.contains(&rec_log_bytes.to_string()),
        "the warn line carries all the numbers: {logged}"
    );

    // And the node is fully usable on the uncompacted log — a degraded boot, not a broken one.
    let v: Option<serde_json::Value> = read(&store, "guard", "kv", "k3").await.unwrap();
    assert_eq!(v.unwrap()["round"], 2);
    drop(store);
    std::fs::remove_dir_all(&parent).ok();
}

/// A store that provably cannot be replayed is REFUSED with both numbers — and the refusal touched
/// nothing, so the override opens the very same directory successfully.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn guard_refuses_and_the_override_still_opens() {
    let (parent, path) = temp_case("wont-fit");
    {
        let store = Store::open(&path).await.unwrap();
        seed(&store, 2).await;
    }
    let before = lb_store::status(&Store::open(&path).await.unwrap()).log_bytes;
    assert!(before > 0, "a real log exists to judge");

    // 1 KiB of available RAM: the log is larger, so the open must be refused.
    let refused = match Store::open_with(
        &path,
        &OpenOptions::default().with_available_ram(Some(1024)),
    )
    .await
    {
        Ok(_) => panic!("the guard must refuse a log larger than available RAM"),
        Err(e) => e,
    };
    match &refused {
        StoreError::WontFit {
            path: p,
            log_bytes,
            available_ram,
        } => {
            assert_eq!(p, &path);
            assert!(*log_bytes > 0 && *available_ram == 1024);
            let msg = refused.to_string();
            assert!(msg.contains(&log_bytes.to_string()) && msg.contains("1024"));
            assert!(
                msg.contains("LB_STORE_OPEN_UNGUARDED=1") && msg.contains("swap"),
                "the diagnostic names the override and the remedies: {msg}"
            );
        }
        other => panic!("expected WontFit, got {other:?}"),
    }

    // Nothing was opened or damaged: the same directory opens with the guard disabled, and every
    // seeded record is still there.
    let opts = OpenOptions::default()
        .with_available_ram(Some(1024))
        .allow_unguarded(true);
    let store = Store::open_with(&path, &opts)
        .await
        .expect("override opens");
    let v: Option<serde_json::Value> = read(&store, "guard", "kv", "k7").await.unwrap();
    assert_eq!(v.unwrap()["round"], 1);
    drop(store);
    std::fs::remove_dir_all(&parent).ok();
}

/// An unmeasurable machine gets today's behaviour exactly: the pass runs, the open proceeds.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unmeasurable_ram_fails_open() {
    let (parent, path) = temp_case("fail-open");
    {
        let store = Store::open(&path).await.unwrap();
        seed(&store, 2).await;
    }
    // `available_ram: None` is what a machine with no readable /proc/meminfo yields.
    let store = Store::open_with(&path, &OpenOptions::default().with_available_ram(None))
        .await
        .expect("no measurement ⇒ no guard");
    let rec = lb_store::status(&store).last_compaction.unwrap();
    assert!(
        rec.skipped.is_none(),
        "nothing to decide from ⇒ run the pass"
    );
    drop(store);
    std::fs::remove_dir_all(&parent).ok();
}

/// The pass record survives the restart at which it matters, and a corrupt sidecar degrades to
/// "no information" rather than to a wrong decision.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn persisted_record_roundtrips_and_degrades() {
    let (parent, path) = temp_case("persist");
    {
        let store = Store::open(&path).await.unwrap();
        seed(&store, 3).await;
    }
    // A real boot pass runs here and persists its record beside the store.
    let store = Store::open(&path).await.unwrap();
    let boot = lb_store::status(&store).last_compaction.unwrap();
    assert!(boot.ok && boot.skipped.is_none());
    drop(store);

    let sidecar = parent.join("last-compaction.json");
    assert!(sidecar.exists(), "the record is a SIBLING of the store dir");
    // Re-open: this boot runs its own pass, persists it, and the sidecar is what it reports.
    let store = Store::open(&path).await.unwrap();
    let this_boot = lb_store::status(&store).last_compaction.unwrap();
    let persisted = lb_store::last_persisted_compaction(&store).expect("persisted");
    assert_eq!(persisted.before_bytes, this_boot.before_bytes);
    assert_eq!(persisted.after_bytes, this_boot.after_bytes);
    assert_eq!(persisted.at_epoch_ms, this_boot.at_epoch_ms);
    drop(store);

    // Corrupt it: the next open must behave exactly as it did before the file existed.
    std::fs::write(&sidecar, b"{ truncated").unwrap();
    let store = Store::open(&path).await.unwrap();
    let rec = lb_store::status(&store).last_compaction.unwrap();
    assert!(
        rec.skipped.is_none() && rec.ok,
        "an unreadable record means no information, never 'do not compact'"
    );
    assert!(
        lb_store::last_persisted_compaction(&store).is_some(),
        "and it is rewritten"
    );
    drop(store);
    std::fs::remove_dir_all(&parent).ok();
}

/// The sidecar is outside the disk budget's arithmetic: `log_bytes` is byte-identical with and
/// without it, so `#122`'s marks can never be moved by this file.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sidecar_is_outside_the_budget_arithmetic() {
    let (parent, path) = temp_case("outside-budget");
    let store = Store::open(&path).await.unwrap();
    seed(&store, 2).await;

    // ONE live handle, measured twice: between the two reads the only thing that changes on disk is
    // the sidecar, so any difference could only come from counting it. (Re-opening between the
    // reads would not isolate it — the engine appends its own shutdown bytes at close.)
    let sidecar = parent.join("last-compaction.json");
    std::fs::remove_file(&sidecar).ok();
    let without = lb_store::status(&store).log_bytes;
    std::fs::write(&sidecar, vec![b'x'; 64 * 1024]).unwrap();
    let with = lb_store::status(&store).log_bytes;
    assert!(without > 0, "there is a real log to measure");
    assert_eq!(with, without, "the sidecar contributes zero budget bytes");

    drop(store);
    std::fs::remove_dir_all(&parent).ok();
}

/// **The P0.** A skipped pass still completes a pending `.merge/` FIRST. If it did not, the merge
/// would be applied by the next *writing* session, which silently loses every write that session
/// makes (debugging/store/compaction-merge-eats-next-sessions-writes.md).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn merge_completion_survives_a_skipped_pass() {
    let (parent, path) = temp_case("merge-p0");
    {
        let store = Store::open(&path).await.unwrap();
        // One round only: with no superseded versions to drop, the merge output is ~the log, so the
        // post-merge log is still big enough for the headroom precondition to decline a pass —
        // which is the situation this case needs (a skip WITH a merge pending).
        seed(&store, 1).await;
    }

    // Reproduce a genuinely interrupted run: compact directly with the engine and stop before the
    // merge is applied. This leaves a REAL pending `.merge/`, exactly as a crash would.
    let dir = std::path::Path::new(&path).to_path_buf();
    tokio::task::spawn_blocking(move || {
        let mut opts = surrealkv::Options::new();
        opts.dir = dir.clone();
        opts.disk_persistence = true;
        opts.enable_versions = false;
        opts.max_segment_size = 1 << 29;
        opts.max_value_threshold = 64;
        let s = surrealkv::Store::new(opts).unwrap();
        s.compact().unwrap();
        s.close().unwrap();
    })
    .await
    .unwrap();
    assert!(
        std::path::Path::new(&path).join(".merge").exists(),
        "a pending merge is staged"
    );

    // Boot with too little headroom for a pass: the PASS is skipped, but the merge must still be
    // completed first.
    // Measured off the segment files directly: opening the store to measure would apply the very
    // merge this case needs left pending.
    let clog_bytes: u64 = std::fs::read_dir(std::path::Path::new(&path).join("clog"))
        .map(|rd| {
            rd.flatten()
                .filter_map(|e| e.metadata().ok())
                .map(|m| m.len())
                .sum()
        })
        .unwrap_or(0);
    assert!(clog_bytes > 0, "there is a real log");
    let avail = Some(clog_bytes * 11 / 10);
    let store = Store::open_with(&path, &OpenOptions::default().with_available_ram(avail))
        .await
        .unwrap();
    assert!(
        lb_store::status(&store)
            .last_compaction
            .unwrap()
            .skipped
            .is_some(),
        "the pass was skipped (that is the point of this case)"
    );
    assert!(
        !std::path::Path::new(&path).join(".merge").exists(),
        "the pending merge was completed BEFORE the skip decision (P0)"
    );

    // The real property: writes made by this session are not eaten at close.
    write(
        &store,
        "guard",
        "kv",
        "after-merge",
        &serde_json::json!({ "kept": true }),
    )
    .await
    .unwrap();
    drop(store);

    let store = Store::open(&path).await.unwrap();
    let v: Option<serde_json::Value> = read(&store, "guard", "kv", "after-merge").await.unwrap();
    assert_eq!(
        v.expect("the write survived the merge-applying boot")["kept"],
        true
    );
    // …and so did the older live set.
    let old: Option<serde_json::Value> = read(&store, "guard", "kv", "k9").await.unwrap();
    assert_eq!(
        old.expect("the pre-merge live set survived too")["round"],
        0
    );
    drop(store);
    std::fs::remove_dir_all(&parent).ok();
}
