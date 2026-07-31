//! The bounded-concurrency pin for `history.list` (history-list-read-cost scope §Testing plan).
//!
//! `list` loads its journal entries concurrently — that is the whole fix — but the fan-out MUST be
//! bounded: a 500-entry stack opening 500 simultaneous store reads trades a latency problem for a
//! contention problem. `list` chunks at 32; this asserts nothing ever exceeds that.
//!
//! **This test lives in its OWN file on purpose.** Cargo compiles each `tests/*.rs` into its own
//! binary, and the in-flight gauge it reads (`lb_undo::peak_in_flight`) is process-global. Sharing
//! a binary with other tests that call `list` would let their concurrency land in this test's
//! reading — a flake that would look like a real bound violation.
//!
//! **Mutation check:** raise `LOAD_CHUNK` in `history.rs` above 32 (or drop the chunking and
//! `join_all` the whole stack) and this goes red at 100 in-flight.

use lb_store::Store;
use lb_undo::{list, peak_in_flight, record_change, reset_in_flight_peak, RecordChange};
use serde_json::json;

/// Enough entries that an unbounded fan-out is unmistakable (100 ≫ the 32 bound).
const ENTRIES: u64 = 100;

/// The bound `history.rs` chunks at. Duplicated deliberately: the test asserts the CONTRACT
/// ("never more than 32 concurrent entry reads"), so importing the constant would let a bad edit
/// move the goalposts and the assertion silently along with it.
const MAX_IN_FLIGHT: usize = 32;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn entry_loads_stay_bounded() {
    let store = Store::memory().await.unwrap();
    let ws = "hist-bound";
    for n in 0..ENTRIES {
        let val = json!({ "n": n });
        record_change(
            &store,
            RecordChange {
                ws,
                actor: "alice",
                surface: "",
                tool: "doc.save",
                trace_id: "t",
                ts: n + 1,
                table: "doc",
                id: &format!("d{n}"),
                new_value: Some(&val),
                depth_cap: Some(500),
            },
        )
        .await
        .expect("record");
    }

    // Zero the high-water mark AFTER seeding (recording a change reads nothing through `list`,
    // but resetting here makes the reading unambiguously about the one call below).
    reset_in_flight_peak();
    let out = list(&store, ws, "alice", "").await.unwrap();
    assert_eq!(out.items.len(), ENTRIES as usize, "all entries listed");

    let peak = peak_in_flight();
    assert!(
        peak <= MAX_IN_FLIGHT,
        "list held {peak} concurrent entry reads on a {ENTRIES}-entry stack — the fan-out must \
         stay bounded at {MAX_IN_FLIGHT}, or a long journal stampedes the store"
    );
    assert!(
        peak > 1,
        "list held only {peak} entry read at a time — the loads are still serial, which is the \
         very bug this scope removed"
    );
}
