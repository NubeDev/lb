//! `retain_runs` — bounded retention for **finished** flow runs and their step rows. A `flipflop`/
//! `cron` demo flow mints a `flow_run` row and several `flow_step_output` rows every firing; nothing
//! purged them, so those two tables are the actual bulk behind the node's disk/scan bloat (~2× the
//! runs in `flow_step_output` — `docs/debugging/jobs/node-pegs-cpu-reactor-rescans-job-table.md`).
//! This trims the finished-run set to the newest `cap` per workspace and deletes the step rows of
//! every run it purges, so both tables stabilise at a bound.
//!
//! ## Never trim a live run (the same invariant as job retention)
//!
//! A run is *finished* only in a terminal status (`success`/`partialFailure`/`failed`/`cancelled`);
//! `pending` (still executing) and `suspended` (paused on a human decision, restartable) are **never**
//! in the delete set — purging a pending/suspended run would orphan or double-run it. The count bound
//! is applied *within* the terminal set only. This mirrors `lb_jobs::retain_terminal`'s load-bearing
//! rule and is guarded by the same style of correctness test.
//!
//! ## Why a sweep across three tables, not a trim at the transition
//!
//! `flow_run` reaches terminal through a single chokepoint (`set_run_status`), so option (a) would
//! fit *it* — but a run's `flow_step_output` rows are keyed `{run_id}:{node_id}` and written by a
//! different verb, so trimming the coordinator alone would leave the step rows (the real bulk)
//! dangling. The three tables must be trimmed **in tandem, keyed by the purged run ids**, which a
//! single sweep does and a per-write transactional trim cannot reach. So this table takes the scope's
//! option (b), for the cross-table reason. The bound is soft (a retention bound), so the mild
//! overshoot between sweeps is fine (`capped.rs`: the reaper is acceptable when the bound is soft).
//!
//! Reuses `capped.rs`'s safe-delete idiom (`LET $keep = (SELECT … LIMIT n); DELETE … NOT IN $keep`),
//! never the inline `DELETE … NOT IN (subquery)` form SurrealDB mis-evaluates. Raw store verb under
//! the reactor's node-internal authority; workspace-walled via `query_ws`.

use lb_store::{Store, StoreError};
use serde_json::Value;

use super::record::{FLOW_RUN_TABLE, FLOW_STEP_TABLE};

/// Compiled fallback for how many finished flow runs to keep per workspace. Generous so ordinary run
/// history the flow UI shows is not lost (`capped.rs`: "defaults live in the caller"); the goal is
/// bounding runaway growth, not aggressive GC. No numeric prefs axis exists today (prefs is a closed
/// typed-axis system), so this caller-owned constant is the default; an operator override slots in
/// here as a resolved value.
pub const DEFAULT_FINISHED_RUN_CAP: usize = 500;

/// Terminal (finished) run statuses — the ONLY runs this sweep may delete. `pending`/`suspended` are
/// deliberately absent (a live or restartable run is never trimmed — the module invariant).
const TERMINAL_RUN_STATUSES: [&str; 4] = ["success", "partialFailure", "failed", "cancelled"];

/// Trim workspace `ws`'s finished flow runs to the newest `cap`, deleting the oldest finished runs
/// beyond it **and** every `flow_step_output` row belonging to a purged run. Non-terminal runs
/// (`pending`/`suspended`) are never touched. `cap == 0` is clamped to 1. Returns the number of
/// `flow_run` rows deleted (step rows deleted in the same transaction are not separately counted).
pub async fn retain_runs(store: &Store, ws: &str, cap: usize) -> Result<usize, StoreError> {
    let n = cap.max(1);
    let terminal = Value::Array(
        TERMINAL_RUN_STATUSES
            .iter()
            .map(|s| Value::String(s.to_string()))
            .collect(),
    );

    // One transaction over the two related deletes:
    //   * `$keep`  = the newest `cap` finished runs' ids (ORDER BY id DESC LIMIT n).
    //   * `$purge` = the finished runs' `run_id`s that are NOT kept — the runs we are dropping (a plain
    //                string field, so it drives the step delete and deserializes cleanly).
    //   * DELETE those flow_run rows, then DELETE their flow_step_output rows (keyed on data.run_id).
    // Both the keep-set and the delete are constrained to `data.status IN $terminal`, so a
    // `pending`/`suspended` run is neither counted nor deleted. The trailing `RETURN count($purge)`
    // collapses the transaction to a single scalar result (index 0) — the run-delete count.
    let sql = format!(
        "BEGIN TRANSACTION;\
         LET $keep = (SELECT VALUE id FROM type::table($runs) \
            WHERE data.status IN $terminal ORDER BY id DESC LIMIT {n});\
         LET $purge = (SELECT VALUE data.run_id FROM type::table($runs) \
            WHERE data.status IN $terminal AND id NOT IN $keep);\
         DELETE FROM type::table($runs) \
            WHERE data.status IN $terminal AND data.run_id IN $purge;\
         DELETE FROM type::table($steps) WHERE data.run_id IN $purge;\
         RETURN count($purge);\
         COMMIT TRANSACTION;"
    );
    let bindings = vec![
        ("runs".into(), Value::String(FLOW_RUN_TABLE.to_string())),
        ("steps".into(), Value::String(FLOW_STEP_TABLE.to_string())),
        ("terminal".into(), terminal),
    ];
    let mut resp = store.query_ws(ws, &sql, bindings).await?;

    let counts: Vec<Value> = resp
        .take(0)
        .map_err(|e| StoreError::Decode(e.to_string()))?;
    let deleted = counts.first().and_then(|c| c.as_u64()).unwrap_or(0) as usize;
    Ok(deleted)
}
