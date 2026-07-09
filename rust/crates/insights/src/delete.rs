//! `delete` — hard-delete an insight and cascade its occurrence ring (insights umbrella scope).
//!
//! The destructive counterpart to the `open → acked → resolved` lifecycle: `resolve` closes an
//! insight but keeps the durable record + its evidence; `delete` erases both. Removing the parent
//! WITHOUT its occurrences would strand the ring rows (`insight_occ` filtered by `insight_id`) —
//! orphan evidence a later scan would surface with no parent. So the delete is a cascade: the ring
//! rows first, then the parent record. Idempotent — deleting an already-gone insight is `Ok`.
//!
//! No auth here (the host gates `mcp:insight.delete:call` first). The parent's `count`/`first_ts`/
//! `last_ts` are gone with the record; there is no "lifetime history" to preserve past a delete.

use lb_store::{delete as store_delete, Store};
use serde_json::Value;

use crate::error::InsightsError;
use crate::insight::OCC_TABLE;
use crate::insight_id::record_id;
use crate::occurrence::TABLE as OCC_ROW_TABLE;

/// Delete insight `id` in workspace `ws`, cascading its occurrence ring. Idempotent. The ring rows
/// (`insight_occ` filtered by `insight_id`) are erased first so no orphan evidence outlives the
/// parent, then the parent record (`insight:<id>`) itself.
// SCOPE: docs/scope/insights/insight-delete-scope.md §"Verb surface" (insight.delete)
pub async fn delete(store: &Store, ws: &str, id: &str) -> Result<(), InsightsError> {
    // Cascade the ring first — a bulk delete filtered by parent (the same `insight_id` field the
    // ring read filters on). Occurrence rows are flat (`capped_insert`, no `data` envelope), so a
    // direct `DELETE ... WHERE insight_id = $iid` clears the whole ring in one statement.
    store
        .query_ws(
            ws,
            "DELETE FROM type::table($tb) WHERE insight_id = $iid",
            vec![
                ("tb".into(), Value::String(OCC_ROW_TABLE.to_string())),
                ("iid".into(), Value::String(id.to_string())),
            ],
        )
        .await?;
    // Then the parent record. `store::delete` is a no-op (still Ok) if it's already gone.
    store_delete(store, ws, OCC_TABLE, &record_id(id)).await?;
    Ok(())
}
