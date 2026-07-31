//! `history.list` and `history.compensations` — the read side of the stack, for a UI affordance
//! (`docs/scope/undo/undo-scope.md` MCP surface). Reads only; the live stack is **state**, returned
//! as a list (not a stream — the stack is state, not motion).

use std::sync::atomic::{AtomicUsize, Ordering};

use lb_store::Store;
use serde::{Deserialize, Serialize};

use crate::error::UndoError;
use crate::model::{Class, JournalEntry};
use crate::persist::{load_entry, load_stack};

/// One row of the history list, for the UI: the step plus whether it is undoable now and any
/// compensation it offers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryItem {
    pub seq: u64,
    pub tool: String,
    pub class: Class,
    /// True if `undo()` would attempt this step (it is reversible). False = greyed
    /// "external — not undoable".
    pub undoable: bool,
    /// True if this step is currently on the redo side (already undone).
    pub redoable: bool,
    pub ts: u64,
}

/// The `history.list` result: the rows, plus the two gate booleans a button-gating caller wants
/// (history-list-read-cost scope). `items` is byte-identical in shape and order to what `list`
/// returned before the flags existed; `can_undo`/`can_redo` are ADDITIVE, computed server-side over
/// the full stack where the data already is — so a UI that only enables two toolbar buttons reads
/// two fields instead of folding N items.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryList {
    pub items: Vec<HistoryItem>,
    /// Some entry on the undo side is actually reversible — `undo()` would attempt something.
    pub can_undo: bool,
    /// Something has been undone and can be replayed.
    pub can_redo: bool,
}

/// How many journal entries are loaded concurrently. Bounded so a long stack cannot open one store
/// read per entry all at once (a 500-entry journal must not fan 500 simultaneous reads); large
/// enough that a typical 100-entry surface collapses to ~3 waves instead of 100 round-trips.
const LOAD_CHUNK: usize = 32;

/// Live and peak count of concurrent `load_entry` calls made by [`list`] — the observable seam for
/// the bounded-concurrency assertion (a 500-entry stack must never open 500 simultaneous store
/// reads). Two relaxed atomic ops per entry load, next to a store round-trip: unmeasurable. The
/// integration tests link the crate rather than compiling it with `cfg(test)`, so this cannot be
/// test-only; nothing in the product path reads it.
static IN_FLIGHT: AtomicUsize = AtomicUsize::new(0);
static PEAK_IN_FLIGHT: AtomicUsize = AtomicUsize::new(0);

/// The highest number of `load_entry` calls [`list`] ever had in flight at once. Test seam — see
/// [`IN_FLIGHT`]. Reset with [`reset_in_flight_peak`].
pub fn peak_in_flight() -> usize {
    PEAK_IN_FLIGHT.load(Ordering::Relaxed)
}

/// Zero the [`peak_in_flight`] high-water mark. Test seam — see [`IN_FLIGHT`].
pub fn reset_in_flight_peak() {
    PEAK_IN_FLIGHT.store(0, Ordering::Relaxed);
}

/// List the actor's stack newest-first: undoable steps then (already-undone) redoable steps, plus
/// the two gate flags.
///
/// The entry reads run CONCURRENTLY in bounded chunks. They used to run one-at-a-time, which made
/// this the slowest read verb on a dashboard open and made it grow with every edit: measured at
/// 432–437 ms on a 99-entry surface (≈99 × the ~4.3 ms unit read) versus 8–13 ms for every other
/// list verb. Order is reassembled from the seq order, never from completion order.
pub async fn list(
    store: &Store,
    ws: &str,
    actor: &str,
    surface: &str,
) -> Result<HistoryList, UndoError> {
    let stack = load_stack(store, ws, actor, surface).await?;
    // Both sides newest-first — the exact order the items are emitted in.
    let undo_seqs: Vec<u64> = stack.undoable.iter().rev().copied().collect();
    let redo_seqs: Vec<u64> = stack.redoable.iter().rev().copied().collect();

    let undo_entries = load_entries(store, ws, &undo_seqs).await?;
    let redo_entries = load_entries(store, ws, &redo_seqs).await?;

    let mut items = Vec::with_capacity(undo_entries.len() + redo_entries.len());
    // A missing entry is SKIPPED, exactly as the serial loop did (a pruned journal row must not
    // fail the read).
    for e in undo_entries.iter().flatten() {
        items.push(to_item(e, true, false));
    }
    for e in redo_entries.iter().flatten() {
        items.push(to_item(e, false, true));
    }

    // The same predicates every consumer folds today, computed here: `can_undo` needs a REVERSIBLE
    // entry on the undo side (a stack of purely non-undoable steps must gate the button OFF, which
    // is why this reads `undoable`, not "the undo side is non-empty"); `can_redo` is simply whether
    // anything has been undone.
    let can_undo = items.iter().any(|i| i.undoable);
    let can_redo = !stack.redoable.is_empty();
    Ok(HistoryList {
        items,
        can_undo,
        can_redo,
    })
}

/// Load `seqs` concurrently in bounded chunks, returning results POSITIONALLY (index i ↔ seqs[i]),
/// so the caller reassembles the list in seq order rather than completion order. `None` marks an
/// entry that is no longer present.
async fn load_entries(
    store: &Store,
    ws: &str,
    seqs: &[u64],
) -> Result<Vec<Option<JournalEntry>>, UndoError> {
    let mut out = Vec::with_capacity(seqs.len());
    for chunk in seqs.chunks(LOAD_CHUNK) {
        let loaded = futures::future::join_all(chunk.iter().map(|&seq| async move {
            let n = IN_FLIGHT.fetch_add(1, Ordering::Relaxed) + 1;
            PEAK_IN_FLIGHT.fetch_max(n, Ordering::Relaxed);
            let r = load_entry(store, ws, seq).await;
            IN_FLIGHT.fetch_sub(1, Ordering::Relaxed);
            r
        }))
        .await;
        // Propagate a real store error rather than silently dropping the entry — only a MISSING
        // entry (`Ok(None)`) is skippable.
        for r in loaded {
            out.push(r?);
        }
    }
    Ok(out)
}

/// The compensating tool a non-undoable step offers, if any (`Class::Compensable`). Empty for a
/// reversible or plainly-irreversible step.
pub async fn compensations(store: &Store, ws: &str, seq: u64) -> Result<Option<String>, UndoError> {
    let entry = load_entry(store, ws, seq)
        .await?
        .ok_or(UndoError::NoSuchStep)?;
    Ok(match entry.class {
        Class::Compensable { compensation_tool } => Some(compensation_tool),
        _ => None,
    })
}

fn to_item(e: &JournalEntry, on_undo_side: bool, on_redo_side: bool) -> HistoryItem {
    HistoryItem {
        seq: e.seq,
        tool: e.tool.clone(),
        class: e.class.clone(),
        undoable: on_undo_side && e.class.is_undoable(),
        redoable: on_redo_side,
        ts: e.ts,
    }
}
