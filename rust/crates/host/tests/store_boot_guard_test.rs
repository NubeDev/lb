//! `store.status` after the boot memory guard (boot-memory-guard scope slice 3), and the budget
//! driver's re-seed from the persisted record.
//!
//! Mandatory categories:
//!   - **capability deny (re-assert)** — `store.status` gained a field, not a door: without
//!     `store:status:read` the call is still an opaque `Denied`, and with it the new `skipped`
//!     field is served.
//!   - **workspace isolation** — n/a *by construction* and asserted as such: the verb stats files
//!     below the namespace wall and reads no record as any principal, which is exactly why the
//!     isolation suites are unmodified by this scope. The deny test below is the gate that applies.
//!
//! Real store on a real path; the record is the one a real boot pass wrote (rule 9).

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{store_status_run, BudgetDriver, StoreAdminError};
use lb_store::{write, OpenOptions, Store};

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

const STATUS: &str = "store:status:read";
const COMPACT: &str = "store:compact:run";

fn temp_case(tag: &str) -> (std::path::PathBuf, String) {
    let parent =
        std::env::temp_dir().join(format!("lb-host-bootguard-{tag}-{}", lb_store::new_ulid()));
    let store = parent.join("store");
    std::fs::create_dir_all(&parent).unwrap();
    let s = store.to_string_lossy().into_owned();
    (parent, s)
}

async fn seed(store: &Store, ws: &str) {
    for k in 0..16u64 {
        write(
            store,
            ws,
            "kv",
            &format!("k{k}"),
            &serde_json::json!({ "pad": "x".repeat(512) }),
        )
        .await
        .unwrap();
    }
}

/// A node that skipped its boot pass says so through `store.status` — the one MCP call that answers
/// "has this node stopped compacting, and why". And the gate is unchanged: no cap ⇒ opaque deny.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn status_serves_the_skip_reason_and_still_denies_without_the_cap() {
    let ws = "bg-status";
    let (parent, path) = temp_case("status");
    {
        let store = Store::open(&path).await.unwrap();
        seed(&store, ws).await;
    }

    // Boot with less headroom than the pass needs: it is declined, and the node opens anyway.
    let probe = Store::open_with(
        &path,
        &OpenOptions::default()
            .with_available_ram(Some(u64::MAX))
            .allow_unguarded(true),
    )
    .await
    .unwrap();
    let log_bytes = lb_store::status(&probe).log_bytes;
    drop(probe);

    let store = Store::open_with(
        &path,
        &OpenOptions::default().with_available_ram(Some(log_bytes * 3 / 2)),
    )
    .await
    .expect("a skip never stops the node opening");

    let p = principal("user:ada", ws, &[STATUS]);
    let report = store_status_run(&store, &p, ws).expect("status with cap");
    assert!(report.persistent);
    let rec = report
        .last_compaction
        .expect("boot always records what it decided");
    let reason = rec.skipped.expect("…including that it skipped, and why");
    assert!(
        reason.contains("available RAM"),
        "the served reason is the operator's whole diagnostic: {reason}"
    );
    assert!(rec.error.is_none(), "a skip is a decision, not a failure");

    // MANDATORY deny re-assert: the new field did not open a new door.
    let p_none = principal("user:eve", ws, &[COMPACT]); // holds the OTHER cap — no bleed-over
    let err = store_status_run(&store, &p_none, ws).unwrap_err();
    assert!(
        matches!(err, StoreAdminError::Denied),
        "opaque deny, got {err:?}"
    );

    drop(store);
    std::fs::remove_dir_all(&parent).ok();
}

/// **Grounding run for `skills/store-compact/SKILL.md`** — prints the real boot-guard flow an
/// operator drives: a normal boot, a skipped boot (record + status), a refused open, and the
/// override. `#[ignore]`d because it exists to be read, not to gate CI (the
/// `compaction_pause_measure_test` precedent):
///
/// ```text
/// cargo test -p lb-host --test store_boot_guard_test -- --ignored --nocapture
/// ```
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "grounding run for the skill doc; prints, asserts little"]
async fn grounding_boot_guard_flow() {
    let ws = "bg-live";
    let (parent, path) = temp_case("live");
    {
        let store = Store::open(&path).await.unwrap();
        seed(&store, ws).await;
    }
    let p = principal("user:ada", ws, &[STATUS]);

    let store = Store::open(&path).await.unwrap();
    let r = store_status_run(&store, &p, ws).unwrap();
    println!(
        "LIVE boot(normal): log_bytes={} last_compaction={:?}",
        r.log_bytes, r.last_compaction
    );
    println!(
        "LIVE persisted record: {:?}",
        lb_store::last_persisted_compaction(&store)
    );
    println!(
        "LIVE sidecar path: {}",
        parent.join("last-compaction.json").display()
    );
    let log_bytes = r.log_bytes;
    drop(store);

    let store = Store::open_with(
        &path,
        &OpenOptions::default().with_available_ram(Some(log_bytes * 3 / 2)),
    )
    .await
    .unwrap();
    let r = store_status_run(&store, &p, ws).unwrap();
    println!(
        "LIVE boot(skipped): log_bytes={} skipped={:?}",
        r.log_bytes,
        r.last_compaction.as_ref().and_then(|c| c.skipped.clone())
    );
    drop(store);

    match Store::open_with(
        &path,
        &OpenOptions::default().with_available_ram(Some(1024)),
    )
    .await
    {
        Ok(_) => println!("LIVE boot(refused): UNEXPECTEDLY OPENED"),
        Err(e) => println!("LIVE boot(refused): {e}"),
    }

    let store = Store::open_with(
        &path,
        &OpenOptions::default()
            .with_available_ram(Some(1024))
            .allow_unguarded(true),
    )
    .await
    .unwrap();
    println!(
        "LIVE boot(override): opened, log_bytes={}",
        lb_store::status(&store).log_bytes
    );
    drop(store);
    std::fs::remove_dir_all(&parent).ok();
}

/// The `#122` budget driver re-seeds its unproductive suspension from the persisted record, so a
/// restart no longer forgets that compaction has stopped paying here — and a *skipped* pass never
/// teaches it anything, because a skip measured nothing.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn budget_driver_re_seeds_from_the_persisted_record_but_never_from_a_skip() {
    let ws = "bg-driver";
    let (parent, path) = temp_case("driver");
    {
        let store = Store::open(&path).await.unwrap();
        seed(&store, ws).await;
    }
    // A real boot pass persists a real record beside the store.
    let store = Store::open(&path).await.unwrap();
    let persisted = lb_store::last_persisted_compaction(&store).expect("a real pass was persisted");
    assert!(persisted.ok && persisted.skipped.is_none());

    // The driver's judgement over that record is the SAME judgement boot's precondition makes —
    // one definition, re-exported (`lb_host::is_productive` is `lb_store`'s).
    let mut driver = BudgetDriver::new(lb_host::budget_marks(Some(1024)));
    driver.note_pass(&persisted);
    assert_eq!(
        driver.is_suspended(),
        !lb_host::is_productive(persisted.before_bytes, persisted.after_bytes),
        "the re-seed reproduces the runtime convergence condition exactly"
    );

    // A skipped record must never move the driver: concluding "unproductive" from a skip would
    // suspend automatic passes on precisely the RAM-bound node that needs them most.
    let mut skipped = persisted.clone();
    skipped.ok = false;
    skipped.skipped = Some("no headroom".into());
    skipped.after_bytes = skipped.before_bytes; // "reclaimed nothing" — but it never ran
    let mut driver = BudgetDriver::new(lb_host::budget_marks(Some(1024)));
    driver.note_pass(&skipped);
    assert!(
        !driver.is_suspended(),
        "a skip is not evidence that compaction has stopped paying"
    );

    drop(store);
    std::fs::remove_dir_all(&parent).ok();
}
