//! The store disk budget, slice 1 — config → marks → `store.status` (disk-budget scope, issue
//! #122). Proves the acceptance property: **an unset budget is today's behaviour exactly** (the
//! flat 256 MiB advisory, no marks), and a set budget derives soft/hard marks from it.
//!
//! Real infra, real bytes (rule 9): the byte-level assertions run against a real SurrealKV store on
//! a real temp dir with real records written through the real write path — no mocks, no fakes.

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{
    budget_marks, over_threshold_advisory, store_status_run, store_status_run_with_budget,
    StoreAdminError, HARD_MARK_PCT, LOG_ADVISORY_BYTES, SOFT_MARK_PCT,
};
use lb_store::{write, Store};

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
        .join(format!("lb-store-budget-{tag}-{}", std::process::id()))
        .to_string_lossy()
        .into_owned()
}

/// Seed real records into a real on-disk store so `log_bytes` is a measured number, not a stub.
async fn seeded_store(path: &str) -> Store {
    let _ = std::fs::remove_dir_all(path);
    let store = Store::open(path).await.unwrap();
    for k in 0..40 {
        write(
            &store,
            "bud-ws",
            "kv",
            &format!("k{k}"),
            &serde_json::json!({ "k": k, "pad": "x".repeat(512) }),
        )
        .await
        .unwrap();
    }
    store
}

/// No budget ⇒ **exactly** today's shape: the flat 256 MiB advisory threshold and no marks at all.
/// This is the property the whole slice is judged on — it is what makes slice 1 purely additive.
#[test]
fn no_budget_keeps_the_flat_advisory_and_has_no_marks() {
    let m = budget_marks(None);
    assert_eq!(m.threshold_bytes, LOG_ADVISORY_BYTES);
    assert_eq!(m.budget_bytes, None);
    assert_eq!(m.soft_mark_bytes, None, "unbudgeted ⇒ no soft mark");
    assert_eq!(m.hard_mark_bytes, None, "unbudgeted ⇒ no hard mark");
    assert_eq!(m.headroom_bytes(1_000), None, "no ceiling ⇒ no headroom");
}

/// A budget ⇒ the marks are percentages of it, and the advisory threshold *is* the soft mark, so
/// the operator is warned at the same point the node starts acting.
#[test]
fn marks_derive_from_the_budget_when_set() {
    let budget = 4 * 1024 * 1024 * 1024u64; // 4 GiB — the Pi-on-an-8-GB-card case in the scope.
    let m = budget_marks(Some(budget));
    assert_eq!(m.budget_bytes, Some(budget));
    assert_eq!(m.soft_mark_bytes, Some(budget * SOFT_MARK_PCT / 100));
    assert_eq!(m.hard_mark_bytes, Some(budget * HARD_MARK_PCT / 100));
    assert_eq!(m.threshold_bytes, m.soft_mark_bytes.unwrap());
    assert!(m.soft_mark_bytes < m.hard_mark_bytes);
    assert!(
        m.hard_mark_bytes.unwrap() < budget,
        "the hard mark leaves room for the remedy to run"
    );

    // Headroom is the ceiling minus the usage, saturating past the budget (never an underflow).
    assert_eq!(m.headroom_bytes(0), Some(budget));
    assert_eq!(m.headroom_bytes(budget + 1), Some(0));

    // A small budget makes the threshold SMALLER than the old constant — the whole point: an
    // operator on an SD card is warned long before 256 MiB.
    let small = budget_marks(Some(64 * 1024 * 1024));
    assert!(small.threshold_bytes < LOG_ADVISORY_BYTES);

    // And a multi-exabyte allowance must not overflow the percentage arithmetic.
    let huge = budget_marks(Some(u64::MAX));
    assert!(huge.soft_mark_bytes.unwrap() < huge.hard_mark_bytes.unwrap());
}

/// `store.status` on a REAL store: unbudgeted reports today's threshold and no budget/headroom;
/// budgeted reports the derived threshold plus the measured headroom. Same store, same bytes.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn status_reports_budget_and_headroom_against_a_real_store() {
    let path = temp_path("status");
    let store = seeded_store(&path).await;
    let p = principal("user:ada", "bud-ws", &[STATUS]);

    // Unbudgeted — byte-for-byte today's report.
    let plain = store_status_run(&store, &p, "bud-ws").expect("status with cap");
    assert!(plain.persistent);
    assert!(plain.log_bytes > 0, "real writes ⇒ measurable log");
    assert_eq!(plain.threshold_bytes, LOG_ADVISORY_BYTES);
    assert_eq!(plain.budget_bytes, None);
    assert_eq!(plain.headroom_bytes, None);
    assert!(plain.advisory.is_none(), "a small store warns nothing");
    // Free disk is honestly absent on this build (no filesystem-stat dependency) — never guessed.
    assert_eq!(plain.free_disk_bytes, None);

    // Budgeted at 1 MiB: the same real log is now measured against an allowance.
    let budget = 1024 * 1024u64;
    let rep = store_status_run_with_budget(&store, &p, "bud-ws", Some(budget)).unwrap();
    assert_eq!(rep.log_bytes, plain.log_bytes, "same store, same bytes");
    assert_eq!(rep.budget_bytes, Some(budget));
    assert_eq!(rep.threshold_bytes, budget * SOFT_MARK_PCT / 100);
    assert_eq!(
        rep.headroom_bytes,
        Some(budget.saturating_sub(rep.log_bytes))
    );

    // A budget the real log already exceeds ⇒ the advisory fires, and headroom floors at zero.
    let tiny = rep.log_bytes / 2;
    let over = store_status_run_with_budget(&store, &p, "bud-ws", Some(tiny)).unwrap();
    let warning = over
        .advisory
        .expect("over the derived soft mark ⇒ advisory");
    assert!(
        warning.contains("store.compact"),
        "names the remedy: {warning}"
    );
    assert_eq!(over.headroom_bytes, Some(0));

    // MANDATORY deny: the budgeted read is the same gated verb — no `store:status:read` ⇒ Denied.
    let eve = principal("user:eve", "bud-ws", &[COMPACT]);
    let err = store_status_run_with_budget(&store, &eve, "bud-ws", Some(budget)).unwrap_err();
    assert!(
        matches!(err, StoreAdminError::Denied),
        "opaque deny, got {err:?}"
    );

    drop(store);
    let _ = std::fs::remove_dir_all(&path);
}

/// The advisory decision is unchanged — it just takes the derived threshold now.
#[test]
fn advisory_fires_only_over_the_derived_threshold() {
    let m = budget_marks(Some(1000));
    assert_eq!(m.threshold_bytes, 800);
    assert!(over_threshold_advisory(800, m.threshold_bytes).is_none());
    assert!(over_threshold_advisory(801, m.threshold_bytes).is_some());
}
