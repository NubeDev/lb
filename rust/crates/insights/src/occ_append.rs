//! `append_occurrence` — the per-raise ring write (insight-occurrences-scope.md).
//!
//! Called by the raise verb after the parent write. Two load-bearing things this does, both
//! deferred to the implementing session:
//!   - **2 KB enforcement:** serialize `data`; if it exceeds `MAX_DATA_BYTES`, reject the whole
//!     raise as `BadInput` (never silent truncation — the producer must slim its payload).
//!   - **Capped-ring eviction:** append via `lb_store::capped_insert` keyed by `insight_id`, cap
//!     from the workspace policy record (default 100, bounds `[0, 1000]`; 0 ⇒ store nothing but
//!     the parent `count` still increments).
//!
//! `seq` is host-assigned (the parent's `count` after the bump, monotone per insight).

use lb_store::{capped_insert, new_ulid, Store};

use crate::error::InsightsError;
use crate::occurrence::{Occurrence, MAX_DATA_BYTES, TABLE};

/// Validate an occurrence's `data` against the 2 KB size cap WITHOUT writing. The raise verb calls
/// this up front (before the parent write) so an oversize payload rejects the whole raise and
/// leaves no orphan parent row (occurrences scope: "never silent truncation").
pub fn validate_occurrence_size(occurrence: &Occurrence) -> Result<(), InsightsError> {
    if occurrence.data.is_null() {
        return Ok(());
    }
    let bytes = serde_json::to_vec(&occurrence.data)
        .map_err(|e| InsightsError::Store(lb_store::StoreError::Decode(e.to_string())))?;
    if bytes.len() > MAX_DATA_BYTES {
        return Err(InsightsError::BadInput(format!(
            "occurrence data {} bytes exceeds the {MAX_DATA_BYTES}-byte cap — slim the payload or link evidence elsewhere",
            bytes.len()
        )));
    }
    Ok(())
}

/// Append one occurrence row for `insight_id` in workspace `ws`. `cap == 0` stores nothing (the
/// parent's `count` still increments — the raise verb did that before calling here).
// SCOPE: docs/scope/insights/insight-occurrences-scope.md §"Verb surface" + §"The record"
pub async fn append_occurrence(
    store: &Store,
    ws: &str,
    insight_id: &str,
    occurrence: &Occurrence,
    cap: usize,
) -> Result<(), InsightsError> {
    // The size cap is enforced up front by the raise verb (`validate_occurrence_size`); re-check
    // here as the last line of defence (a direct caller can't slip an oversize row past the ring).
    validate_occurrence_size(occurrence)?;
    // cap == 0 ⇒ occurrences disabled for this workspace; the raise still succeeds, nothing stored.
    if cap == 0 {
        return Ok(());
    }
    // The stored body carries `insight_id` (the `data.insight_id` filter path the ring read uses)
    // plus the occurrence's own fields (`oseq`/ts/severity/data). `capped_insert` injects `cap_key`
    // (= insight_id, the FIFO bucket) + `seq` (a ULID, the eviction order) — one row per raise,
    // trimmed to `cap` newest in the same transaction.
    let mut body = serde_json::to_value(occurrence)
        .map_err(|e| InsightsError::Store(lb_store::StoreError::Decode(e.to_string())))?;
    if let Some(obj) = body.as_object_mut() {
        obj.insert("insight_id".into(), serde_json::json!(insight_id));
    }
    let id = new_ulid();
    capped_insert(store, ws, TABLE, &id, insight_id, cap, &body).await?;
    Ok(())
}
