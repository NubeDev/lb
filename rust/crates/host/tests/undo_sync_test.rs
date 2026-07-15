//! The undo scope's §2.3 sync case: **the conditional restore is enforced at the node where it
//! applies, not where it was captured** — an offline-captured undo that arrives stale is REFUSED,
//! never merged last-writer-wins.
//!
//! ## What this models, and what it deliberately does not
//!
//! Two REAL nodes with two REAL, independent stores (rule #9). "The edge captured an undo while
//! offline; the hub's copy moved on; the undo now applies at the hub" is modelled by capturing the
//! step against the edge's store and applying the restore against the HUB's store — which is
//! exactly what "the predicate travels with the operation" means: the same expected-`rev` predicate,
//! evaluated against a different authoritative store, must refuse.
//!
//! ### The transport is carried by the test, and that is the point
//!
//! There is no journal replication in the product today: `ChannelSync` (`host/src/sync.rs`) is the
//! only cross-node sync and it mirrors inbox `Item`s only — no doc replication, no journal
//! replication. So the test itself carries the edge's REAL journal records to the hub's store
//! (`carry_journal` — a real `scan` + real `write`, no hand-built rows), then runs the REAL
//! `apply_undo` against the hub. Nothing about undo is re-implemented: the transport is stubbed,
//! the mechanism under test is not (testing §0 — the sanctioned "external you cannot run locally"
//! shape, here a replication layer that does not exist yet).
//!
//! Without this carry the hub has no journal, `apply_undo` returns `Empty`, and the test passes
//! **vacuously without ever evaluating a predicate** — proving nothing. That is precisely the trap
//! this file exists to avoid, so the refusal assertion below is `Stale` and NEVER `Empty`.

use lb_host::{Node, Role as NodeRole};
use lb_store::{read, scan, write, MAX_SCAN_LIMIT};
use lb_undo::{apply_undo, record_change, RecordChange, UndoError};
use serde_json::json;

const DOC: &str = "doc";
/// The undo journal's real tables (`undo` events, `undo_stack` cursor, `undo_seq` counter,
/// `undo_live` companions) — what a journal-replicating sync would have to carry.
const JOURNAL_TABLES: [&str; 4] = ["undo", "undo_stack", "undo_seq", "undo_live"];

/// Two real nodes, two real independent stores — an edge and a hub.
async fn edge_and_hub() -> (Node, Node) {
    let edge = Node::boot_as(NodeRole::Edge).await.expect("edge boots");
    let hub = Node::boot_as(NodeRole::Hub).await.expect("hub boots");
    (edge, hub)
}

/// Carry the edge's REAL journal records into the hub's store — the re-sync the product does not
/// have yet. Real `scan` reads, real `write`s; the rows are the edge's own, never fabricated.
async fn carry_journal(edge: &Node, hub: &Node, ws: &str) {
    for table in JOURNAL_TABLES {
        let page = scan(&edge.store, ws, table, MAX_SCAN_LIMIT, None)
            .await
            .expect("scan the edge's journal table");
        for row in page.rows {
            // `scan` hands back the storage envelope (`{rev, data}`) under a QUALIFIED id
            // (`undo:1`), while `write` takes the inner value under a BARE id and stamps its own
            // `rev`. Unwrap both, or the hub gets double-wrapped rows under mangled keys that
            // `load_stack` cannot read — which is an `Empty` stack and a vacuous test.
            let id = row.id.strip_prefix(&format!("{table}:")).unwrap_or(&row.id);
            let value = row.data.get("data").unwrap_or(&row.data);
            write(&hub.store, ws, table, id, value)
                .await
                .expect("carry the row to the hub");
        }
    }
}

/// THE load-bearing case (§2.3): the edge captures an undo against the value it saw; meanwhile the
/// hub's copy changes. Applied at the hub, the restore is REFUSED — its expected `rev` no longer
/// matches — and the hub's newer value survives untouched. No silent LWW clobber.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_offline_captured_undo_is_refused_at_the_hub_when_its_copy_moved_on() {
    let (edge, hub) = edge_and_hub().await;
    let ws = "undo-sync-refused";

    // Both nodes start from the same state (the edge's last sync point).
    write(&edge.store, ws, DOC, "d1", &json!({"title": "draft"}))
        .await
        .unwrap();
    write(&hub.store, ws, DOC, "d1", &json!({"title": "draft"}))
        .await
        .unwrap();

    // OFFLINE: the edge makes a tracked change. Its journal records the before-image + the `rev`
    // the record had on the EDGE — the predicate that will travel with the undo.
    record_change(
        &edge.store,
        RecordChange {
            ws,
            actor: "user:ada",
            surface: "",
            tool: "doc.rename",
            trace_id: "t",
            ts: 1,
            table: DOC,
            id: "d1",
            new_value: Some(&json!({"title": "edge-v1"})),
            depth_cap: None,
        },
    )
    .await
    .unwrap();

    // MEANWHILE: the hub's authoritative copy moves on independently. Two writes, so the hub's
    // live `rev` is unambiguously PAST the `rev` the edge's step expects — the divergence the
    // predicate must catch. (One write would leave the hub at the same rev the step expects and
    // the predicate would match by coincidence, applying the undo: a false green.)
    write(&hub.store, ws, DOC, "d1", &json!({"title": "hub-edit-1"}))
        .await
        .unwrap();
    write(&hub.store, ws, DOC, "d1", &json!({"title": "hub-moved-on"}))
        .await
        .unwrap();

    // RE-SYNC: the edge's journal reaches the hub, carrying its predicate with it.
    carry_journal(&edge, &hub, ws).await;

    // The undo now applies AT THE HUB — and is refused, because the hub's live `rev` is not the one
    // the step expects. The predicate is enforced where it APPLIES, not where it was captured.
    let err = apply_undo(&hub.store, ws, "user:ada", "")
        .await
        .unwrap_err();
    assert!(
        matches!(err, UndoError::Stale),
        "the hub must REFUSE on the stale predicate. `Empty` here would mean the journal never \
         arrived and no predicate was evaluated — a vacuous pass. Got {err:?}"
    );

    // The hub's newer value survives — nothing was clobbered.
    assert_eq!(
        read(&hub.store, ws, DOC, "d1").await.unwrap(),
        Some(json!({"title": "hub-moved-on"})),
        "a refused undo must never overwrite the authoritative copy"
    );
}

/// The CONTROL for the case above: same carried journal, same hub — but the hub's copy did NOT
/// move, so the identical undo APPLIES there. This is what proves the refusal above is caused by
/// the intervening write and not merely by the undo arriving at a different node's store.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_same_undo_applies_at_the_hub_when_its_copy_did_not_move() {
    let (edge, hub) = edge_and_hub().await;
    let ws = "undo-sync-applies";

    write(&edge.store, ws, DOC, "d1", &json!({"title": "draft"}))
        .await
        .unwrap();
    write(&hub.store, ws, DOC, "d1", &json!({"title": "draft"}))
        .await
        .unwrap();
    record_change(
        &edge.store,
        RecordChange {
            ws,
            actor: "user:ada",
            surface: "",
            tool: "doc.rename",
            trace_id: "t",
            ts: 1,
            table: DOC,
            id: "d1",
            new_value: Some(&json!({"title": "edge-v1"})),
            depth_cap: None,
        },
    )
    .await
    .unwrap();

    // The edge's tracked write also lands on the hub (the doc sync that would precede re-sync),
    // leaving the hub's copy at exactly the state the step expects.
    write(&hub.store, ws, DOC, "d1", &json!({"title": "edge-v1"}))
        .await
        .unwrap();
    carry_journal(&edge, &hub, ws).await;

    // Nobody else wrote at the hub: the predicate holds, so the restore APPLIES there.
    apply_undo(&hub.store, ws, "user:ada", "")
        .await
        .expect("an unraced undo applies at the hub");
    assert_eq!(
        read(&hub.store, ws, DOC, "d1").await.unwrap(),
        Some(json!({"title": "draft"})),
        "the carried undo restored the before-image at the hub"
    );
}

/// Idempotence at the apply point (§2.3's other half): a restore that already landed does not
/// re-apply on a second delivery. The first undo consumes the step and bumps the record's `rev`;
/// a repeat finds nothing to undo rather than restoring twice.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_replayed_undo_does_not_restore_twice() {
    let (edge, _hub) = edge_and_hub().await;
    let ws = "undo-sync-idempotent";

    write(&edge.store, ws, DOC, "d1", &json!({"title": "draft"}))
        .await
        .unwrap();
    record_change(
        &edge.store,
        RecordChange {
            ws,
            actor: "user:ada",
            surface: "",
            tool: "doc.rename",
            trace_id: "t",
            ts: 1,
            table: DOC,
            id: "d1",
            new_value: Some(&json!({"title": "edge-v1"})),
            depth_cap: None,
        },
    )
    .await
    .unwrap();

    apply_undo(&edge.store, ws, "user:ada", "").await.unwrap();
    let after_first = read(&edge.store, ws, DOC, "d1").await.unwrap();

    // A second delivery of the same undo: the step is already on the redo side — nothing to undo.
    let err = apply_undo(&edge.store, ws, "user:ada", "")
        .await
        .unwrap_err();
    assert!(
        matches!(err, UndoError::Empty(_)),
        "a replayed undo finds an empty stack, not a second restore; got {err:?}"
    );
    assert_eq!(
        read(&edge.store, ws, DOC, "d1").await.unwrap(),
        after_first,
        "the replay left the record exactly as the first undo did"
    );
}
