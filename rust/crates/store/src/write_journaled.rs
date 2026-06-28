//! Write a domain change AND its undo before-image journal entry in ONE transaction — the
//! atomic-before-image seam the undo journal rides (`docs/scope/undo/undo-scope.md` "Intent").
//!
//! This is the undo analogue of [`write_tx`](crate::write_tx) (which pairs a change with an outbox
//! effect). Here the second record is a **journal entry** the caller has already built from the
//! record's prior state. Capturing the before-image and committing the change in the *same*
//! transaction is the whole correctness point: the journal can never be out of step with the data
//! (no orphan entry, no un-journaled change). A forced failure of either statement rolls back both.
//!
//! The change is written with the **same monotonic `rev` bump** as [`write`](crate::write), so the
//! `after` rev the caller records in the journal entry is the rev this write produces. The caller
//! reads `rev` back via [`read_versioned`](crate::read_versioned) after the write to learn it (the
//! transaction does not return it — keeping the seam shaped like `write_tx`).
//!
//! Workspace-namespaced (README §7): both records land in `ws`'s namespace, selected first.
//! Raw verb — `caps::check` is the caller's job.

use serde_json::Value;

use crate::open::{Store, StoreError};
use crate::record::FIRST_REV;

/// The journal entry to commit alongside the change. `table`/`id` address the journal row; `value`
/// is the fully-built entry (before-image, expected revs, group, kind, …) the undo crate owns the
/// shape of. The journal row is a plain record (its own `rev` bumps like any write).
pub struct JournalWrite<'a> {
    pub table: &'a str,
    pub id: &'a str,
    pub value: &'a Value,
}

/// Atomically (1) upsert the change at `change_table:change_id` — bumping its monotonic `rev` —
/// and (2) write the `journal` entry, in one transaction. Either both commit or neither does.
pub async fn write_journaled(
    store: &Store,
    ws: &str,
    change_table: &str,
    change_id: &str,
    change: &Value,
    journal: &JournalWrite<'_>,
) -> Result<(), StoreError> {
    let db = store.use_ws(ws).await?;
    db.query(
        "BEGIN TRANSACTION;
         UPSERT type::thing($ct, $cid) CONTENT { \
            data: $cdata, \
            rev: (type::thing($ct, $cid).rev ?? ($first - 1)) + 1 \
         } RETURN NONE;
         UPSERT type::thing($jt, $jid) CONTENT { \
            data: $jdata, \
            rev: (type::thing($jt, $jid).rev ?? ($first - 1)) + 1 \
         } RETURN NONE;
         COMMIT TRANSACTION;",
    )
    .bind(("ct", change_table.to_string()))
    .bind(("cid", change_id.to_string()))
    .bind(("cdata", change.clone()))
    .bind(("jt", journal.table.to_string()))
    .bind(("jid", journal.id.to_string()))
    .bind(("jdata", journal.value.clone()))
    .bind(("first", FIRST_REV))
    .await?
    .check()?;
    Ok(())
}
