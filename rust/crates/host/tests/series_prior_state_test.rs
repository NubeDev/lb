//! **Prior-state / upgrade tests for the retention plane** (testing-scope §2 category 6, §3.2 row 1;
//! issue [#108](https://github.com/NubeDev/lb/issues/108) bug #2).
//!
//! Every other test in this crate starts from a bare host, which is exactly why the suite could not
//! see bug #2: a changed retention default was correct on a fresh install and **dead on every
//! existing one**, because the older build's per-network rows sat at a LONGER prefix and won under
//! longest-prefix-wins. The new code reported success; the old data reported nothing.
//!
//! These tests seed what a previous build left on disc (`support/prior_state.rs`), run the
//! convergence the upgrade is supposed to perform through the REAL capability-gated verbs, and
//! assert the NEW behaviour actually governs — proven by real eviction, not by a returned status.
//!
//! Real store (`mem://`), real ingest write→drain, real MCP dispatch. No mocks (testing §0).

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::call_ingest_tool;
use lb_ingest::sample_count;
use lb_store::Store;
use serde_json::json;

#[path = "support/prior_state.rs"]
mod prior_state;
use prior_state::{PriorRetention, PriorSeries};

/// The first sample's ts and the GC's logical `now` — both constants (determinism §3): the retention
/// cutoff is on the ts axis, so nothing here may read a wall clock.
const FIRST_TS_MS: u64 = 1_784_070_000_000;
const NOW_MS: u64 = 1_784_070_999_999;

fn admin(ws: &str) -> Principal {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: "user:test".into(),
        ws: ws.into(),
        role: Role::Member,
        caps: [
            "mcp:series.retention.set:call",
            "mcp:series.retention.list:call",
            "mcp:series.retention.gc:call",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect(),
        iat: 0,
        exp: u64::MAX,
        constraint: None,
        run_id: None,
    };
    verify(&key, &mint(&key, &claims), 1).expect("token verifies")
}

/// Run one real GC pass through the capability-gated verb and report what the count cap evicted.
async fn gc(store: &Store, p: &Principal, ws: &str) -> u64 {
    let pass = call_ingest_tool(
        store,
        p,
        ws,
        "series.retention.gc",
        &json!({"now_ms": NOW_MS}),
    )
    .await
    .expect("gc");
    pass["capped_raw"].as_u64().expect("capped_raw")
}

/// THE HEADLINE (bug #2's shape). A node upgraded from a build that wrote one policy row per
/// network: the newer global `modbus.` cap is set successfully and evicts **nothing**, because the
/// stale `modbus.plant-a.` row is a longer prefix. Only after the upgrade removes the stale row does
/// the new default actually govern.
///
/// A test starting from a bare host asserts the last phase alone and passes against the bug.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_new_global_cap_governs_only_after_the_previous_builds_per_network_row_is_removed() {
    let store = Store::memory().await.unwrap();
    let ws = "nube";
    let p = admin(ws);

    // ── PRIOR STATE: what the previous build left on disc ──────────────────────────────────────
    PriorSeries::in_ws(ws)
        .history("modbus.plant-a.temp", 60, FIRST_TS_MS)
        .seed(&store)
        .await;
    PriorRetention::in_ws(ws)
        .per_network("modbus.", "plant-a")
        .seed(&store)
        .await;

    // ── The new build writes its global default through the real verb, and it SUCCEEDS ─────────
    call_ingest_tool(
        &store,
        &p,
        ws,
        "series.retention.set",
        &json!({"prefix": "modbus.", "raw_for_ms": 0, "max_samples": 10}),
    )
    .await
    .expect("the new global policy is written");

    // ── The bug: success is reported, nothing is governed ──────────────────────────────────────
    assert_eq!(
        gc(&store, &p, ws).await,
        0,
        "the stale longer prefix must still own the series — if this ever returns non-zero the \
         longest-prefix-wins precedence has changed, and every operator's per-network tuning is \
         being silently overridden by a global row"
    );
    assert_eq!(
        sample_count(&store, ws, "modbus.plant-a.temp")
            .await
            .unwrap(),
        60,
        "the newer global cap of 10 evicted NOTHING on a node with history — bug #2"
    );

    // ── The convergence an upgrade must actually perform ───────────────────────────────────────
    call_ingest_tool(
        &store,
        &p,
        ws,
        "series.retention.delete",
        &json!({"prefix": "modbus.plant-a."}),
    )
    .await
    .expect("the migration removes the previous build's per-network row");

    assert_eq!(
        gc(&store, &p, ws).await,
        50,
        "with the stale row gone the global cap must evict the excess — a migration that reports \
         success without deleting the row would leave this at 0"
    );
    assert_eq!(
        sample_count(&store, ws, "modbus.plant-a.temp")
            .await
            .unwrap(),
        10,
        "the NEW default now governs the previous build's own history"
    );
}

/// The other prior-state axis: a policy row whose **stored shape** predates a field. A row written
/// before `max_samples`/`filter` existed must read back at today's defaults (unbounded, no filter) —
/// the closed-struct trap — and must upgrade in place when the new build re-sets it.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_policy_row_written_before_the_cap_existed_reads_back_unbounded_and_upgrades_in_place() {
    let store = Store::memory().await.unwrap();
    let ws = "nube";
    let p = admin(ws);

    PriorSeries::in_ws(ws)
        .history("modbus.plant-b.temp", 60, FIRST_TS_MS)
        .seed(&store)
        .await;
    // Seven days of raw — the only two fields the older release knew about.
    PriorRetention::in_ws(ws)
        .pre_cap_shaped("modbus.", 604_800_000)
        .seed(&store)
        .await;

    let listed = call_ingest_tool(&store, &p, ws, "series.retention.list", &json!({}))
        .await
        .expect("list");
    assert_eq!(listed["policies"][0]["prefix"], "modbus.");
    assert_eq!(
        listed["policies"][0]["raw_for_ms"], 604_800_000u64,
        "the fields the older release DID write must survive the read"
    );
    assert_eq!(
        listed["policies"][0]["max_samples"], 0,
        "a field absent from the stored row must read back as today's default (unbounded), never \
         as a surprise bound applied to an existing operator's data"
    );
    assert!(
        listed["policies"][0].get("filter").is_none(),
        "and an absent filter must stay absent — an older row must not acquire behaviour it never \
         asked for"
    );

    assert_eq!(
        gc(&store, &p, ws).await,
        0,
        "an unbounded prior row evicts nothing on the count axis"
    );
    assert_eq!(
        sample_count(&store, ws, "modbus.plant-b.temp")
            .await
            .unwrap(),
        60
    );

    // The new build re-sets the same prefix, adding the cap and preserving the stored horizon.
    call_ingest_tool(
        &store,
        &p,
        ws,
        "series.retention.set",
        &json!({"prefix": "modbus.", "raw_for_ms": 604_800_000u64, "max_samples": 10}),
    )
    .await
    .expect("set");

    assert_eq!(gc(&store, &p, ws).await, 50);
    assert_eq!(
        sample_count(&store, ws, "modbus.plant-b.temp")
            .await
            .unwrap(),
        10,
        "the upgraded row governs the pre-existing history in place — same prefix, no new row"
    );
}
