//! `delete_occurrence` — hard-delete one row from an insight's occurrence ring
//! (insight-occurrences-scope.md).
//!
//! The per-transaction counterpart to `delete` (which erases the whole insight): an analyst who
//! wants a single spurious evidence row (a mis-fired reading, a duplicate txn ref) gone drops just
//! that occurrence. The row is addressed by `(insight_id, oseq)` — `oseq` is the stable, monotone
//! per-insight sequence the ring read already surfaces (the ULID `capped_insert` id is internal).
//!
//! The parent's lifetime `count`/`first_ts`/`last_ts` are LEFT UNTOUCHED: `count` is the monotone
//! lifetime firing total (it may already exceed the stored rows — occurrences scope), so deleting
//! an evidence row removes it from the ring without rewriting history. Idempotent — deleting an
//! already-gone occurrence is `Ok`. No auth here (the host gates `mcp:insight.occurrence.delete:call`).

use lb_store::Store;
use serde_json::Value;

use crate::error::InsightsError;
use crate::occurrence::TABLE;

/// Delete the occurrence with sequence `oseq` from insight `insight_id`'s ring in workspace `ws`.
/// Idempotent. Addressed by `(insight_id, oseq)` — the stable per-insight sequence, not the internal
/// ULID row id. `count`/`first_ts`/`last_ts` on the parent are unchanged (lifetime truth).
// SCOPE: docs/scope/insights/insight-delete-scope.md §"Verb surface" (insight.occurrence.delete)
pub async fn delete_occurrence(
    store: &Store,
    ws: &str,
    insight_id: &str,
    oseq: u64,
) -> Result<(), InsightsError> {
    // The stored row carries both `insight_id` (the ring bucket) and `oseq` (our monotone sequence,
    // serialized under that name — see `Occurrence`). Match on both so a sequence from one insight
    // can never delete another's row (workspace + parent scoped).
    store
        .query_ws(
            ws,
            "DELETE FROM type::table($tb) WHERE insight_id = $iid AND oseq = $oseq",
            vec![
                ("tb".into(), Value::String(TABLE.to_string())),
                ("iid".into(), Value::String(insight_id.to_string())),
                ("oseq".into(), Value::Number(oseq.into())),
            ],
        )
        .await?;
    Ok(())
}
