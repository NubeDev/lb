//! Crate-private persistence helpers: the per-(ws) sequence counter, journal-entry rows, and the
//! per-(ws, actor[, surface]) stack-state record. Kept in one file because they share the
//! table-name constants and the (de)serialize boilerplate; each is a thin verb over `lb_store`.
//!
//! Journal entries are written through [`lb_store::write`] (immutable once written — only ever
//! created, never mutated). The stack cursor is read-modify-write LWW state. The seq counter uses
//! the store's monotonic `rev` as the sequence source so we never read-modify-write a counter
//! racily (the `rev` bump is atomic server-side).

use lb_store::{read, read_versioned, write, Store};

use crate::error::UndoError;
use crate::model::{JournalEntry, StackState, ENTRY_TABLE, SEQ_ID, SEQ_TABLE, STACK_TABLE};

/// The mutable per-step predicate state: for each touched record (same order as the entry's
/// `touched`), the `rev` the record currently sits at as far as undo is concerned. Updated on every
/// undo/redo so the next inverse op guards against the *actual* current rev — not a stale
/// capture-time rev — across repeated undo↔redo cycles. Kept separate from the immutable
/// [`JournalEntry`] (which stays audit-stable). Stored at `undo_live:{seq}`.
pub(crate) const LIVE_TABLE: &str = "undo_live";

/// Load the live predicate revs for step `seq`, defaulting to the capture-time `after` revs from
/// the entry if none has been written yet (the first undo guards against the original after rev).
pub(crate) async fn load_live_revs(
    store: &Store,
    ws: &str,
    entry: &JournalEntry,
) -> Result<Vec<u64>, UndoError> {
    match read(store, ws, LIVE_TABLE, &entry.seq.to_string()).await? {
        Some(v) => serde_json::from_value(v).map_err(UndoError::codec),
        None => Ok(entry.touched.iter().map(|t| t.expected_after_rev).collect()),
    }
}

/// Persist the live predicate revs for step `seq`.
pub(crate) async fn save_live_revs(
    store: &Store,
    ws: &str,
    seq: u64,
    revs: &[u64],
) -> Result<(), UndoError> {
    let value = serde_json::to_value(revs).map_err(UndoError::codec)?;
    write(store, ws, LIVE_TABLE, &seq.to_string(), &value).await?;
    Ok(())
}

/// The id of a stack-state record: `{actor}` for the default stack, `{actor}:{surface}` for a
/// finer editor-style stack. One actor's surfaces never collide, and a surface key is opaque.
pub(crate) fn stack_id(actor: &str, surface: &str) -> String {
    if surface.is_empty() {
        actor.to_string()
    } else {
        format!("{actor}:{surface}")
    }
}

/// Allocate the next monotonic sequence number for workspace `ws`. We bump a single counter record
/// and use the store's own `rev` as the sequence value — atomic, gap-free, and never racy because
/// the `rev` increment happens server-side inside the UPSERT (no read-modify-write here).
pub(crate) async fn next_seq(store: &Store, ws: &str) -> Result<u64, UndoError> {
    // The value is irrelevant; we only want the rev bump. Each write yields a strictly greater rev.
    write(store, ws, SEQ_TABLE, SEQ_ID, &serde_json::json!({})).await?;
    Ok(read_versioned(store, ws, SEQ_TABLE, SEQ_ID).await?.rev)
}

/// Persist an immutable journal entry at `undo:{seq}`.
pub(crate) async fn save_entry(
    store: &Store,
    ws: &str,
    entry: &JournalEntry,
) -> Result<(), UndoError> {
    let value = serde_json::to_value(entry).map_err(UndoError::codec)?;
    write(store, ws, ENTRY_TABLE, &entry.seq.to_string(), &value).await?;
    Ok(())
}

/// Load journal entry `undo:{seq}` from workspace `ws`.
pub(crate) async fn load_entry(
    store: &Store,
    ws: &str,
    seq: u64,
) -> Result<Option<JournalEntry>, UndoError> {
    match read(store, ws, ENTRY_TABLE, &seq.to_string()).await? {
        Some(v) => Ok(Some(serde_json::from_value(v).map_err(UndoError::codec)?)),
        None => Ok(None),
    }
}

/// Load the stack cursor for (ws, actor, surface), or a fresh empty one if none exists yet.
pub(crate) async fn load_stack(
    store: &Store,
    ws: &str,
    actor: &str,
    surface: &str,
) -> Result<StackState, UndoError> {
    let id = stack_id(actor, surface);
    match read(store, ws, STACK_TABLE, &id).await? {
        Some(v) => Ok(serde_json::from_value(v).map_err(UndoError::codec)?),
        None => Ok(StackState::new(ws, actor, surface)),
    }
}

/// Persist the (mutated) stack cursor.
pub(crate) async fn save_stack(
    store: &Store,
    ws: &str,
    stack: &StackState,
) -> Result<(), UndoError> {
    let id = stack_id(&stack.actor, &stack.surface);
    let value = serde_json::to_value(stack).map_err(UndoError::codec)?;
    write(store, ws, STACK_TABLE, &id, &value).await?;
    Ok(())
}
