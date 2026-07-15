//! `scan_all` — drain EVERY page of a ws-scoped table scan (the cursor loop `lb_store::scan`
//! deliberately does not do). `scan` hard-caps one page at [`MAX_SCAN_LIMIT`] rows; every flows verb
//! that reads a shared table (step slots, node last-values, retained inputs, runs, flows) and filters
//! in code MUST go through this loop — a single-page read silently drops rows once the workspace
//! outgrows one page, which surfaces as missing node values, runs that never finalise, and reactors
//! skipping flows (debugging/flows/single-scan-page-drops-rows-past-200.md).
//!
//! Correct-over-clever: this drains the whole table and lets the caller prefix/body-filter, because
//! the scan cursor is the SurrealDB `<string>id` rendering (`⟨⟩`-bracketed for composite ids) whose
//! ordering does not agree with the display id — a prefix-seeded early exit would be unsound. The
//! flows tables are retention-bounded (`retain_runs`, sweeps), so the full drain stays small; a
//! store-level prefix scan is the follow-up if one ever profiles hot.

use lb_store::{scan, Row, Store, StoreError, MAX_SCAN_LIMIT};

/// Every row of `table` in `ws`, in id order — the cursor loop over `lb_store::scan` pages.
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
