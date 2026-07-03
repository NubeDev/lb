//! `retain_terminal` — bounded retention for **terminal** job rows. A `Done`/`Failed`/`Cancelled`
//! job is a fire-once record with no resume value once drained; nothing purged them, so the `job`
//! table grew a row per flow run / agent run forever and the reactor's drain scan grew with it (the
//! CPU-burn in `docs/debugging/jobs/node-pegs-cpu-reactor-rescans-job-table.md`). This trims the
//! terminal set to the newest `cap` per workspace, so the table stabilises at a bound.
//!
//! ## The one unacceptable failure: never trim a resumable job
//!
//! Deleting a `Running`/`Suspended` job would make the reactor lose a live run and, on the next
//! enqueue/resume, **double-run** it. So the delete predicate is `status IN (terminal)` and nothing
//! else — a resumable job outside the window is kept **forever**, by design. The count bound is
//! applied *within* the terminal set only (a `WHERE data.status IN $terminal` on both the keep-set
//! and the delete). This is the load-bearing invariant the retention tests guard.
//!
//! ## Why a sweep, not a transactional trim at the write
//!
//! `capped.rs` prefers insert+trim in one transaction when there is a single write chokepoint. Jobs
//! reach a terminal state through **two** verbs (`complete` → `Done`/`Failed`, `cancel` →
//! `Cancelled`), so there is no single chokepoint to hang the trim on; a periodic sweep trims them
//! uniformly in one place (the scope's option (b), chosen for this table for that reason). The bound
//! is *soft* — a retention bound, not a correctness-critical ring cap — so the mild overshoot between
//! sweeps is acceptable (`capped.rs` calls the reaper "an optional secondary safety net"; here it is
//! the primary mechanism because the bound is soft). We reuse `capped.rs`'s exact safe-delete idiom:
//! a `LET $keep = (SELECT … ORDER BY … DESC LIMIT n)` then `DELETE … WHERE … NOT IN $keep` (never the
//! inline `DELETE … NOT IN (subquery)` form, which SurrealDB mis-evaluates — see `capped.rs`).
//!
//! Ordering is by the record id (`<string>id`): a job id is workspace-unique and stable, and terminal
//! rows accrue in roughly enqueue order, so "newest `cap` by id" keeps the most recent history. The
//! sweep is a raw store verb, run under the reactor's own node-internal authority — no user cap.

use lb_store::{Store, StoreError};
use serde_json::Value;

use super::TABLE;

/// Compiled fallback for how many terminal jobs to keep per workspace. Generous on purpose — ordinary
/// run history (a few hundred recent runs) is not lost; the goal is bounding runaway growth, not
/// aggressive GC (`capped.rs`: "defaults live in the caller"). There is no numeric prefs axis today
/// (prefs is a closed typed-axis system), so this is the caller-owned default; an operator override
/// would slot in here as a resolved value, not a change to the primitive.
pub const DEFAULT_TERMINAL_JOB_CAP: usize = 500;

/// The stored (`kebab-case`) status strings that are terminal — the ONLY rows this sweep may delete.
/// `Running`/`Suspended` are deliberately absent (see the module invariant).
const TERMINAL_STATUSES: [&str; 3] = ["done", "failed", "cancelled"];

/// Trim workspace `ws`'s terminal job rows to the newest `cap`, deleting the oldest terminal rows
/// beyond it. Resumable jobs are never in the delete set. `cap == 0` is clamped to 1 (keeping zero
/// terminal rows is never asked for and would churn). Returns the number of rows deleted.
pub async fn retain_terminal(store: &Store, ws: &str, cap: usize) -> Result<usize, StoreError> {
    let n = cap.max(1);
    let terminal = Value::Array(
        TERMINAL_STATUSES
            .iter()
            .map(|s| Value::String(s.to_string()))
            .collect(),
    );

    // One transaction, mirroring `capped_insert`'s safe-delete idiom:
    //   * `$keep` = the newest `cap` terminal rows' ids (ORDER BY id DESC LIMIT n).
    //   * DELETE the terminal rows whose id is NOT in `$keep`.
    // The keep-set and the delete are BOTH constrained to `data.status IN $terminal`, so a resumable
    // job is neither counted toward the bound nor ever deleted. `<string>id` orders totally; the
    // idiom is selected into `$keep` (SurrealDB requires the ORDER BY idiom to be literally selected).
    // `LIMIT {n}` is a formatted integer literal (the only cross-version-safe LIMIT shape) — `n` is a
    // caller value, never external input.
    // `$keep`  = the ids (as strings) of the newest `cap` terminal rows.
    // `$doomed` = the terminal rows NOT kept — the exact set we delete, captured as strings BEFORE the
    //             delete so we can both drive the delete and count it (deleting first would leave
    //             nothing to count, and a `DELETE ... RETURN meta::id(id)` mis-evaluates the id to NONE
    //             at return time here). Both are plain strings (`<string>id`), so they deserialize and
    //             compare cleanly. Ordering is by `id` (the selected idiom; a cast in ORDER BY is
    //             rejected by this SurrealDB version), which sorts terminal rows in id order.
    let sql = format!(
        "BEGIN TRANSACTION;\
         LET $keep = (SELECT VALUE id FROM type::table($tb) \
            WHERE data.status IN $terminal ORDER BY id DESC LIMIT {n});\
         LET $doomed = (SELECT VALUE <string>id FROM type::table($tb) \
            WHERE data.status IN $terminal AND id NOT IN $keep);\
         DELETE FROM type::table($tb) WHERE <string>id IN $doomed;\
         RETURN count($doomed);\
         COMMIT TRANSACTION;"
    );
    let bindings = vec![
        ("tb".into(), Value::String(TABLE.to_string())),
        ("terminal".into(), terminal),
    ];
    let mut resp = store.query_ws(ws, &sql, bindings).await?;

    // A transaction whose body ends in `RETURN` collapses to a SINGLE result set at index 0 (the
    // RETURN value), not one-per-statement — so the scalar `count($doomed)` is at index 0.
    let counts: Vec<Value> = resp
        .take(0)
        .map_err(|e| StoreError::Decode(e.to_string()))?;
    let deleted = counts.first().and_then(|c| c.as_u64()).unwrap_or(0) as usize;
    Ok(deleted)
}
