//! The conditional restore — the correctness core (`docs/scope/undo/undo-scope.md`).
//!
//! A restore writes a set of records back to a target state, **only if** every one of them still
//! has the `rev` the step expects (no intervening writer). The predicate and the write happen in
//! ONE transaction so the check cannot go stale between testing and writing: each UPSERT is guarded
//! by `WHERE`/`IF` on the live `rev`, and if any guard fails the whole transaction is rolled back
//! and we report [`UndoError::Stale`] — safe-by-refusal, never a forced clobber.
//!
//! This is the seam that makes undo safe across sync: the predicate travels *with* the operation
//! and is enforced wherever the transaction actually applies (the authoritative node for shared
//! data). An offline-captured restore that arrives stale is refused at the apply point, not merged.

use lb_store::{read_versioned, Store};
use serde_json::Value;

use crate::error::UndoError;

/// One record to conditionally restore: write `target` iff the live `rev` equals `expected_rev`.
pub(crate) struct Restore {
    pub table: String,
    pub id: String,
    /// The value to write, or `None` to delete the record (restore-to-absent).
    pub target: Option<Value>,
    /// The `rev` the live record must currently have for the restore to apply.
    /// [`lb_store::Versioned::ABSENT_REV`] (0) means "must still be absent".
    pub expected_rev: u64,
}

/// Apply all restores atomically iff every predicate holds. Returns the new `rev` each record was
/// left at (in the same order as `restores`) so the caller can record the predicate for the
/// *inverse* operation (redo after undo, undo after redo). Returns `Err(Stale)` and writes nothing
/// if any record's current `rev` differs from its `expected_rev`.
pub(crate) async fn restore_all(
    store: &Store,
    ws: &str,
    restores: &[Restore],
) -> Result<Vec<u64>, UndoError> {
    // Pre-check every predicate first (cheap, read-only). This gives an early, clean refusal and
    // keeps the transaction below simple. The transaction re-asserts each predicate so a writer
    // racing between pre-check and commit is still caught (the transaction is the source of truth).
    for r in restores {
        let live = read_versioned(store, ws, &r.table, &r.id).await?;
        if live.rev != r.expected_rev {
            return Err(UndoError::Stale);
        }
    }

    // Build one transaction that re-asserts each predicate and applies the write. We use an
    // explicit guard: THROW inside the transaction rolls it back, so any mismatch aborts the whole
    // restore. A deleted target is modelled as removing the row.
    let mut q = String::from("BEGIN TRANSACTION;\n");
    let mut binds: Vec<(String, Value)> = Vec::new();
    for (i, r) in restores.iter().enumerate() {
        let tb = format!("t{i}");
        let id = format!("i{i}");
        let ex = format!("e{i}");
        binds.push((tb.clone(), Value::String(r.table.clone())));
        binds.push((id.clone(), Value::String(r.id.clone())));
        binds.push((ex.clone(), Value::from(r.expected_rev)));
        // Re-assert the predicate; THROW (and thus roll back) on mismatch.
        q.push_str(&format!(
            "IF (type::thing(${tb}, ${id}).rev ?? 0) != ${ex} {{ THROW 'stale' }};\n"
        ));
        match &r.target {
            Some(v) => {
                let d = format!("d{i}");
                binds.push((d.clone(), v.clone()));
                q.push_str(&format!(
                    "UPSERT type::thing(${tb}, ${id}) CONTENT {{ data: ${d}, \
                     rev: (type::thing(${tb}, ${id}).rev ?? 0) + 1 }} RETURN NONE;\n"
                ));
            }
            None => {
                q.push_str(&format!("DELETE type::thing(${tb}, ${id});\n"));
            }
        }
    }
    q.push_str("COMMIT TRANSACTION;");

    match store.query_ws(ws, &q, binds).await {
        Ok(_) => {
            // Report the rev each record was left at: a written record is now `expected_rev + 1`;
            // a deleted record is back to absent (rev 0). This is the predicate the inverse op
            // must assert next.
            let produced = restores
                .iter()
                .map(|r| match r.target {
                    Some(_) => r.expected_rev + 1,
                    None => 0,
                })
                .collect();
            Ok(produced)
        }
        Err(e) => {
            // A thrown 'stale' surfaces as a backend error; map it to the typed refusal. Any other
            // backend error propagates as-is.
            let msg = format!("{e}");
            if msg.contains("stale") {
                Err(UndoError::Stale)
            } else {
                Err(UndoError::Store(e))
            }
        }
    }
}
