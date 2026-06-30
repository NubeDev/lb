//! Delete a record at `<table>:<id>` within a workspace's namespace.
//!
//! The namespace is selected from `ws` before the delete, so an erase can only touch its own
//! workspace (README §7) — the same hard wall as [`write`](crate::write). Idempotent: deleting a
//! record that is already absent is a no-op success (the caller is left in the "it is gone"
//! state it asked for). The caller is expected to have passed `caps::check` first — this is the
//! raw store verb, not the authorization point.
//!
//! Unlike [`write`](crate::write), a delete does NOT bump `rev` (there is no record left to carry
//! a revision); an undo of a delete restores the prior value via the journaled write path.

use crate::open::{Store, StoreError};

/// Erase `table:id` from workspace `ws`. No-op (still `Ok`) if the record is already absent.
pub async fn delete(store: &Store, ws: &str, table: &str, id: &str) -> Result<(), StoreError> {
    let db = store.use_ws(ws).await?;
    db.query("DELETE type::thing($tb, $id) RETURN NONE")
        .bind(("tb", table.to_string()))
        .bind(("id", id.to_string()))
        .await?
        .check()?;
    Ok(())
}
