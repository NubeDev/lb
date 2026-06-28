//! Integration tests for the undo journal — exercised against a REAL in-memory store (rule #9: no
//! mocks). These tests are the proof of the scope's load-bearing claims
//! (`docs/scope/undo/undo-scope.md` "Testing plan"):
//!   - reversible round-trip (undo restores, redo re-applies);
//!   - the conditional restore refuses a STALE undo (intervening writer) — no silent clobber;
//!   - workspace isolation of the journal;
//!   - the irreversible boundary (not-undoable refused; compensable surfaces a compensation);
//!   - redo truncation on a new `do`;
//!   - atomicity of capture (before-image + change in one tx);
//!   - the classification `max`-composition unit.

use lb_store::{read, read_versioned, write, Store};
use lb_undo::{
    apply_redo, apply_undo, classify, compensations, list, record_change, record_irreversible,
    Class, RecordChange, RecordIrreversible, UndoError,
};
use serde_json::json;

const DOC: &str = "doc";

fn change<'a>(
    ws: &'a str,
    actor: &'a str,
    tool: &'a str,
    id: &'a str,
    val: &'a serde_json::Value,
) -> RecordChange<'a> {
    RecordChange {
        ws,
        actor,
        surface: "",
        tool,
        trace_id: "t",
        ts: 1,
        table: DOC,
        id,
        new_value: Some(val),
    }
}

#[tokio::test]
async fn undo_restores_before_image_and_redo_reapplies() {
    let store = Store::memory().await.unwrap();
    let ws = "undo-roundtrip";

    // Seed an initial value (a real prior write), then a tracked rename.
    write(&store, ws, DOC, "d1", &json!({"title": "draft"}))
        .await
        .unwrap();
    record_change(
        &store,
        change(ws, "alice", "doc.rename", "d1", &json!({"title": "v1"})),
    )
    .await
    .unwrap();
    assert_eq!(
        read(&store, ws, DOC, "d1").await.unwrap(),
        Some(json!({"title": "v1"}))
    );

    // Undo → back to "draft".
    apply_undo(&store, ws, "alice", "").await.unwrap();
    assert_eq!(
        read(&store, ws, DOC, "d1").await.unwrap(),
        Some(json!({"title": "draft"}))
    );

    // Redo → forward to "v1".
    apply_redo(&store, ws, "alice", "").await.unwrap();
    assert_eq!(
        read(&store, ws, DOC, "d1").await.unwrap(),
        Some(json!({"title": "v1"}))
    );

    // Undo again → "draft" (the round-trip is exact and repeatable).
    apply_undo(&store, ws, "alice", "").await.unwrap();
    assert_eq!(
        read(&store, ws, DOC, "d1").await.unwrap(),
        Some(json!({"title": "draft"}))
    );
}

#[tokio::test]
async fn undo_of_a_create_deletes_back_to_absence() {
    let store = Store::memory().await.unwrap();
    let ws = "undo-create";

    // No prior record → this is a create.
    record_change(
        &store,
        change(ws, "alice", "doc.new", "fresh", &json!({"title": "new"})),
    )
    .await
    .unwrap();
    assert!(read(&store, ws, DOC, "fresh").await.unwrap().is_some());

    apply_undo(&store, ws, "alice", "").await.unwrap();
    assert_eq!(
        read(&store, ws, DOC, "fresh").await.unwrap(),
        None,
        "undo of a create restores absence"
    );

    // Redo brings it back.
    apply_redo(&store, ws, "alice", "").await.unwrap();
    assert_eq!(
        read(&store, ws, DOC, "fresh").await.unwrap(),
        Some(json!({"title": "new"}))
    );
}

#[tokio::test]
async fn a_stale_undo_is_refused_not_clobbered() {
    let store = Store::memory().await.unwrap();
    let ws = "undo-stale";

    write(&store, ws, DOC, "d1", &json!({"title": "draft"}))
        .await
        .unwrap();
    record_change(
        &store,
        change(ws, "alice", "doc.rename", "d1", &json!({"title": "v1"})),
    )
    .await
    .unwrap();

    // An intervening writer (a collaborator) changes the doc AFTER the tracked step.
    write(
        &store,
        ws,
        DOC,
        "d1",
        &json!({"title": "collaborator-edit"}),
    )
    .await
    .unwrap();

    // Undo must REFUSE — the record changed since the step.
    let err = apply_undo(&store, ws, "alice", "").await.unwrap_err();
    assert!(
        matches!(err, UndoError::Stale),
        "expected Stale, got {err:?}"
    );

    // The collaborator's edit is intact — nothing was clobbered.
    assert_eq!(
        read(&store, ws, DOC, "d1").await.unwrap(),
        Some(json!({"title": "collaborator-edit"})),
        "the intervening write must survive a refused undo"
    );
}

#[tokio::test]
async fn the_journal_is_walled_per_workspace() {
    let store = Store::memory().await.unwrap();

    write(&store, "ws-a", DOC, "d1", &json!({"title": "a-draft"}))
        .await
        .unwrap();
    record_change(
        &store,
        change(
            "ws-a",
            "alice",
            "doc.rename",
            "d1",
            &json!({"title": "a-v1"}),
        ),
    )
    .await
    .unwrap();

    // ws-B's actor sees nothing of ws-A's stack.
    let b_history = list(&store, "ws-b", "alice", "").await.unwrap();
    assert!(b_history.is_empty(), "ws-B must not see ws-A's journal");

    // And a ws-B undo finds nothing to undo (its own empty stack).
    let err = apply_undo(&store, "ws-b", "alice", "").await.unwrap_err();
    assert!(matches!(err, UndoError::Empty(_)));

    // ws-A's record is untouched by the ws-B attempt.
    assert_eq!(
        read(&store, "ws-a", DOC, "d1").await.unwrap(),
        Some(json!({"title": "a-v1"}))
    );
}

#[tokio::test]
async fn an_irreversible_step_is_refused() {
    let store = Store::memory().await.unwrap();
    let ws = "undo-irreversible";

    record_irreversible(
        &store,
        RecordIrreversible {
            ws,
            actor: "alice",
            surface: "",
            tool: "workflow.open_pr",
            trace_id: "t",
            ts: 1,
            class: Class::Irreversible,
            group: None,
        },
    )
    .await
    .unwrap();

    // history shows it, greyed (not undoable).
    let history = list(&store, ws, "alice", "").await.unwrap();
    assert_eq!(history.len(), 1);
    assert!(!history[0].undoable, "an irreversible step is greyed");

    // undo refuses it.
    let err = apply_undo(&store, ws, "alice", "").await.unwrap_err();
    assert!(matches!(err, UndoError::NotUndoable { .. }), "got {err:?}");
}

#[tokio::test]
async fn a_compensable_step_surfaces_its_compensation() {
    let store = Store::memory().await.unwrap();
    let ws = "undo-compensable";

    let seq = record_irreversible(
        &store,
        RecordIrreversible {
            ws,
            actor: "alice",
            surface: "",
            tool: "workflow.open_pr",
            trace_id: "t",
            ts: 1,
            class: Class::Compensable {
                compensation_tool: "workflow.close_pr".into(),
            },
            group: None,
        },
    )
    .await
    .unwrap();

    // undo refuses but names the compensation.
    let err = apply_undo(&store, ws, "alice", "").await.unwrap_err();
    match err {
        UndoError::NotUndoable { compensation_tool } => {
            assert_eq!(compensation_tool.as_deref(), Some("workflow.close_pr"));
        }
        other => panic!("expected NotUndoable with compensation, got {other:?}"),
    }

    // history.compensations surfaces it for a UI affordance.
    assert_eq!(
        compensations(&store, ws, seq).await.unwrap().as_deref(),
        Some("workflow.close_pr")
    );
}

#[tokio::test]
async fn a_new_do_truncates_the_redo_stack() {
    let store = Store::memory().await.unwrap();
    let ws = "undo-truncate";

    write(&store, ws, DOC, "d1", &json!({"v": 0}))
        .await
        .unwrap();
    record_change(
        &store,
        change(ws, "alice", "doc.set", "d1", &json!({"v": 1})),
    )
    .await
    .unwrap();
    apply_undo(&store, ws, "alice", "").await.unwrap(); // d1 back to v0, step is redoable

    // A new do truncates the redo future.
    record_change(
        &store,
        change(ws, "alice", "doc.set", "d1", &json!({"v": 2})),
    )
    .await
    .unwrap();

    // redo now finds nothing.
    let err = apply_redo(&store, ws, "alice", "").await.unwrap_err();
    assert!(
        matches!(err, UndoError::Empty(_)),
        "redo truncated by new do; got {err:?}"
    );
}

#[tokio::test]
async fn capture_bumps_rev_so_predicate_has_a_token() {
    // The before-image + change land together and the change's rev is recorded for the predicate.
    let store = Store::memory().await.unwrap();
    let ws = "undo-rev";

    write(&store, ws, DOC, "d1", &json!({"v": 1}))
        .await
        .unwrap(); // rev 1
    record_change(
        &store,
        change(ws, "alice", "doc.set", "d1", &json!({"v": 2})),
    )
    .await
    .unwrap();
    assert_eq!(
        read_versioned(&store, ws, DOC, "d1").await.unwrap().rev,
        2,
        "the tracked write bumped rev to 2"
    );
}

#[test]
fn classification_is_max_over_parts() {
    // Pure state → reversible.
    assert_eq!(classify(false, None), Class::Reversible);
    // Reached outbox → irreversible (derived, not trusted).
    assert_eq!(classify(true, None), Class::Irreversible);
    // Reached outbox + declared compensation → compensable (adds a handle; never downgrades).
    assert_eq!(
        classify(true, Some("workflow.close_pr")),
        Class::Compensable {
            compensation_tool: "workflow.close_pr".into()
        }
    );
    // A spurious compensation on a pure-state action does NOT make it irreversible.
    assert_eq!(classify(false, Some("whatever")), Class::Reversible);

    // The combine() composition: any irreversible/compensable part dominates reversible.
    assert_eq!(
        Class::Reversible.combine(Class::Reversible),
        Class::Reversible
    );
    assert_eq!(
        Class::Reversible.combine(Class::Irreversible),
        Class::Irreversible
    );
    let comp = Class::Compensable {
        compensation_tool: "c".into(),
    };
    assert_eq!(Class::Reversible.combine(comp.clone()), comp);
}
