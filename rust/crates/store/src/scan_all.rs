//! `scan_all` — drain EVERY page of a ws-scoped table scan, the cursor loop [`scan`] deliberately
//! does not do. [`scan`] returns one bounded page (hard-capped at [`MAX_SCAN_LIMIT`] rows) plus a
//! `next` cursor; a caller that reads ONE page and filters in code silently drops every row past that
//! page the moment the workspace outgrows it. Any read that needs the WHOLE table — a roster list, a
//! full join over retained rows, a reactor's live set — MUST go through this loop
//! (debugging/flows/single-scan-page-drops-rows-past-200.md).
//!
//! Correct-over-clever: this drains the whole table (in id order) and hands the rows back for the
//! caller to prefix/body-filter. A prefix-seeded early exit would be unsound — the scan cursor is the
//! SurrealDB `<string>id` rendering (`⟨⟩`-bracketed for composite ids like `[series, producer, seq]`)
//! whose ordering does NOT agree with the display id, so a cursor that "looks past" the prefix can
//! still be ordered before a wanted row. Full drain is the only sound read; a store-level prefix scan
//! (cursor seeded at the prefix, server-side `string::starts_with`) is the follow-up if a drain ever
//! profiles hot.
//!
//! No silent backstop: a partial return here would just relocate the "rows vanish past N" bug to a
//! larger N. Tables read through this seam are expected to be bounded by retention/config limits (the
//! real bound lives there); an unbounded table is a retention problem to solve separately, not
//! something this read hides.

use crate::open::{Store, StoreError};
use crate::scan::{scan, Row, MAX_SCAN_LIMIT};

/// Every row of `table` in `ws`, in id order — the cursor loop over [`scan`]'s pages, drained to the
/// end. Returns the raw [`Row`]s (id + stored `data`); callers unwrap the `write`-envelope and decode
/// as their record shape demands.
pub async fn scan_all(store: &Store, ws: &str, table: &str) -> Result<Vec<Row>, StoreError> {
    let mut rows = Vec::new();
    let mut after: Option<String> = None;
    loop {
        let page = scan(store, ws, table, MAX_SCAN_LIMIT, after.as_deref()).await?;
        rows.extend(page.rows);
        match page.next {
            Some(cursor) => after = Some(cursor),
            None => return Ok(rows),
        }
    }
}
