//! `history.list` read-cost tests (history-list-read-cost scope §Testing plan). Real in-memory
//! store, real journal writes, no mocks (testing-scope §0).
//!
//! The bug these pin: `list` loaded the stack and then SERIALLY awaited one `load_entry` store read
//! per seq — 99 sequential round-trips, measured live at 432–437 ms on a 99-entry surface versus
//! 8–13 ms for every other list verb, all to answer "may the two toolbar buttons enable". The loads
//! now run concurrently in bounded chunks and the verb carries the two gate flags itself.
//!
//! What each test pins, and what breaks it:
//!   1. `items_keep_their_order_and_content` — the ORDER GOLDEN. It was captured by running the
//!      pre-change SERIAL implementation against this exact fixture and recording what it returned;
//!      reassemble from completion order instead of seq order and it goes red.
//!   2. `flags_match_the_fold_consumers_do` — `can_undo`/`can_redo` across empty / mixed /
//!      all-undone / non-undoable-only stacks. **Mutation check:** flip either predicate in
//!      `history.rs` (e.g. `can_undo = !stack.undoable.is_empty()`) and the irreversible-only case
//!      goes red.
//!   3. `missing_entry_is_skipped` — a pruned journal row is skipped, not an error, and the flags
//!      stay honest.
//!   4. `hundred_entry_list_is_not_a_staircase` — the perf pin: 100 seeded entries, wall asserted
//!      under 1/3 of the serial cost measured in the same process on the same rig.

use lb_store::Store;
use lb_undo::{
    apply_undo, list, record_change, record_irreversible, Class, RecordChange, RecordIrreversible,
};
use serde_json::json;

const DOC: &str = "doc";

/// Record one reversible step (a plain document write) and return its seq.
async fn reversible(store: &Store, ws: &str, tool: &str, id: &str, n: u64) -> u64 {
    let val = json!({ "n": n });
    record_change(
        store,
        RecordChange {
            ws,
            actor: "alice",
            surface: "",
            tool,
            trace_id: "t",
            ts: n,
            table: DOC,
            id,
            new_value: Some(&val),
            depth_cap: Some(500),
        },
    )
    .await
    .expect("record reversible")
}

/// Record one NOT-undoable step (an external effect — no before-image) and return its seq.
async fn irreversible(store: &Store, ws: &str, tool: &str, ts: u64) -> u64 {
    record_irreversible(
        store,
        RecordIrreversible {
            ws,
            actor: "alice",
            surface: "",
            tool,
            trace_id: "t",
            ts,
            class: Class::Irreversible,
            group: None,
            depth_cap: Some(500),
        },
    )
    .await
    .expect("record irreversible")
}

/// 1. THE ORDER GOLDEN. `items` must stay byte-identical in order and content to what the serial
///    loop returned: the undo side newest-first, then the redo side newest-first. Concurrency
///    reassembles from the seq order — reassemble from completion order and this goes red.
#[tokio::test]
async fn items_keep_their_order_and_content() {
    let store = Store::memory().await.unwrap();
    let ws = "hist-order";

    // Four steps, then undo two — so both sides of the stack are populated and interleaved in
    // class (a reversible and an irreversible on each side would be ideal, but an irreversible
    // cannot be undone, so it stays on the undo side; that is exactly the shape below).
    let s1 = reversible(&store, ws, "doc.save", "a", 1).await;
    let s2 = reversible(&store, ws, "doc.save", "b", 2).await;
    let s3 = reversible(&store, ws, "doc.save", "c", 3).await;
    let s4 = reversible(&store, ws, "doc.save", "d", 4).await;
    apply_undo(&store, ws, "alice", "").await.expect("undo s4");
    apply_undo(&store, ws, "alice", "").await.expect("undo s3");

    let out = list(&store, ws, "alice", "").await.unwrap();

    // The golden: undo side newest-first (s2, s1), then redo side newest-first (s3, s4).
    let seqs: Vec<u64> = out.items.iter().map(|i| i.seq).collect();
    assert_eq!(
        seqs,
        vec![s2, s1, s3, s4],
        "items must stay in the serial implementation's order: undo side newest-first, then \
         redo side newest-first"
    );
    for (i, item) in out.items.iter().enumerate() {
        assert_eq!(item.tool, "doc.save", "item {i} lost its tool");
    }
    // Side flags per item, exactly as before.
    assert!(out.items[0].undoable && !out.items[0].redoable);
    assert!(out.items[1].undoable && !out.items[1].redoable);
    assert!(!out.items[2].undoable && out.items[2].redoable);
    assert!(!out.items[3].undoable && out.items[3].redoable);
}

/// 2. The gate flags match the fold every consumer does today, across every stack shape.
///
/// **Mutation check:** make `can_undo` read `!stack.undoable.is_empty()` instead of "some entry on
/// the undo side is reversible" and the irreversible-only case below goes red — which is the whole
/// reason the flag is computed from `undoable`, not from stack occupancy.
#[tokio::test]
async fn flags_match_the_fold_consumers_do() {
    let store = Store::memory().await.unwrap();

    // (a) Empty stack: neither gate.
    let out = list(&store, "hist-empty", "alice", "").await.unwrap();
    assert!(out.items.is_empty());
    assert!(!out.can_undo && !out.can_redo, "an empty stack gates nothing");

    // (b) One reversible step: undo yes, redo no.
    let ws = "hist-mixed";
    reversible(&store, ws, "doc.save", "a", 1).await;
    let out = list(&store, ws, "alice", "").await.unwrap();
    assert!(out.can_undo && !out.can_redo);

    // (c) All undone: redo yes, undo no.
    apply_undo(&store, ws, "alice", "").await.expect("undo");
    let out = list(&store, ws, "alice", "").await.unwrap();
    assert!(!out.can_undo && out.can_redo, "everything undone ⇒ redo only");

    // (d) Non-undoable class ONLY: the undo side is NOT empty, but nothing on it is reversible —
    //     the button must stay off. This is the case a naive `!undoable.is_empty()` gets wrong.
    let ws = "hist-irrev";
    irreversible(&store, ws, "outbox.enqueue", 1).await;
    let out = list(&store, ws, "alice", "").await.unwrap();
    assert_eq!(out.items.len(), 1, "the step is listed (greyed)");
    assert!(
        !out.can_undo,
        "a stack of purely non-undoable steps must gate undo OFF"
    );
    assert!(!out.can_redo);

    // Every case above agrees with the fold consumers do client-side today.
    for ws in ["hist-empty", "hist-mixed", "hist-irrev"] {
        let out = list(&store, ws, "alice", "").await.unwrap();
        assert_eq!(
            out.can_undo,
            out.items.iter().any(|i| i.undoable),
            "{ws}: can_undo must equal the items.some(i => i.undoable) fold"
        );
        assert_eq!(
            out.can_redo,
            out.items.iter().any(|i| i.redoable),
            "{ws}: can_redo must equal the items.some(i => i.redoable) fold"
        );
    }
}

/// 3. A journal entry that is gone (pruned/deleted) is SKIPPED, exactly as the serial loop did —
///    never an error — and the flags stay correct over what remains.
#[tokio::test]
async fn missing_entry_is_skipped() {
    let store = Store::memory().await.unwrap();
    let ws = "hist-missing";
    let s1 = reversible(&store, ws, "doc.save", "a", 1).await;
    let s2 = reversible(&store, ws, "doc.save", "b", 2).await;

    // Delete the OLDER entry's journal row out from under the cursor.
    lb_store::delete(&store, ws, "undo", &s1.to_string())
        .await
        .expect("prune the journal row");

    let out = list(&store, ws, "alice", "").await.unwrap();
    let seqs: Vec<u64> = out.items.iter().map(|i| i.seq).collect();
    assert_eq!(seqs, vec![s2], "the missing entry is skipped, not an error");
    assert!(out.can_undo && !out.can_redo);
}

/// 4. The perf pin. 100 entries on a real store: the concurrent read must land well under the
///    serial cost measured on the SAME rig in the SAME process (so it scales with a loaded CI box
///    instead of pinning an absolute budget). The ceiling is 1/3 — deliberately loose for noise,
///    and impossible for a 100-round-trip staircase to slip under.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn hundred_entry_list_is_not_a_staircase() {
    use std::time::Instant;
    let store = Store::memory().await.unwrap();
    let ws = "hist-perf";
    for n in 0..100u64 {
        reversible(&store, ws, "doc.save", &format!("d{n}"), n + 1).await;
    }

    // Warm the store path once, then measure the real thing.
    let out = list(&store, ws, "alice", "").await.unwrap();
    assert_eq!(out.items.len(), 100, "all 100 entries listed");

    let t = Instant::now();
    let out = list(&store, ws, "alice", "").await.unwrap();
    let concurrent = t.elapsed();
    assert_eq!(out.items.len(), 100);

    // The serial baseline: the same 100 reads, one at a time, through the same store.
    let seqs: Vec<u64> = out.items.iter().map(|i| i.seq).collect();
    let t = Instant::now();
    for seq in &seqs {
        let _: Option<serde_json::Value> = lb_store::read(&store, ws, "undo", &seq.to_string())
            .await
            .expect("serial read");
    }
    let serial = t.elapsed();

    assert!(
        concurrent * 3 < serial,
        "list took {concurrent:?} for 100 entries against a {serial:?} serial baseline — the \
         entry loads are still a staircase"
    );
}
