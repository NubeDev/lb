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

use lb_store::Store;

use crate::error::InsightsError;
use crate::occurrence::Occurrence;

/// Append one occurrence row for `insight_id` in workspace `ws`. `cap == 0` stores nothing (the
/// parent's `count` still increments — the raise verb did that before calling here).
// SCOPE: docs/scope/insights/insight-occurrences-scope.md §"Verb surface" + §"The record"
pub async fn append_occurrence(
    _store: &Store,
    _ws: &str,
    _insight_id: &str,
    _occurrence: &Occurrence,
    _cap: usize,
) -> Result<(), InsightsError> {
    // 1. If `cap == 0` ⇒ return Ok (occurrences disabled for this ws; raise still succeeds).
    // 2. Serialize `occurrence.data` — if > MAX_DATA_BYTES (2 KB) ⇒ BadInput (whole raise rejects).
    //    (The raise verb validates this BEFORE the parent write so a reject leaves no orphan row —
    //    the implementing session decides whether to validate up front or rely on this guard.)
    // 3. Insert via `lb_store::capped_insert` (table=insight_occ, key=insight_id, cap=cap,
    //    id=ULID). The store primitive trims oldest in the same transaction (no over-eviction).
    todo!(
        "insights: capped-ring append + 2 KB enforcement — SCOPE: occurrences-scope.md §The record"
    )
}
