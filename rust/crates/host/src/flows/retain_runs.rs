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
//! the reactor's node-internal authority; workspace-walled via `query_ws`. Not a transaction, and
//! steps are deleted before runs — see the body for why that is both required and safe.

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
/// `flow_run` rows deleted (their step rows are deleted in the same pass, not separately counted).
pub async fn retain_runs(store: &Store, ws: &str, cap: usize) -> Result<usize, StoreError> {
    let n = cap.max(1);
    let terminal = Value::Array(
        TERMINAL_RUN_STATUSES
            .iter()
            .map(|s| Value::String(s.to_string()))
            .collect(),
    );

    // Two round-trips, deliberately **not** one transaction:
    //   1. read the purge set — the finished runs' `run_id`s outside the newest `cap`
    //      (`capped.rs`'s `LET $keep … NOT IN $keep` idiom), into Rust;
    //   2. delete their `flow_step_output` rows FIRST, then the `flow_run` rows, keyed on that set.
    // Both the keep-set and the delete are constrained to `data.status IN $terminal`, so a
    // `pending`/`suspended` run is neither counted nor deleted.
    //
    // **Why no BEGIN/COMMIT.** Under SurrealDB 3, reading or deleting a table that does not exist
    // yet fails the enclosing transaction — so a workspace with no flow runs ("…due to a cancelled
    // transaction") or with runs but no step rows ("…due to a failed transaction") failed this sweep
    // on EVERY cycle, forever, and the warning read as noise. Outside a transaction the same
    // statements are fine on a missing table.
    //
    // **Why steps first.** The transaction only ever protected one thing: a run deleted while its step
    // rows survive, orphaned for good (a later purge set is keyed on runs that would be gone). Deleting
    // steps BEFORE runs makes that partial failure harmless — if the run delete fails, the runs are
    // still there, the next sweep computes the same purge set and finishes. Every step is idempotent.
    //
    // **Why the count is Rust's.** A top-level `RETURN` outside a transaction does not give back a
    // slot the way the in-transaction one did (SurrealDB 3 — measured: 5 statements, 4 empty slots),
    // and this verb has already under-reported once through a shape change (`take_transaction_return`).
    // The purge set is in hand, so its length IS the count.
    let bindings = |purge: Option<Value>| {
        let mut b = vec![
            ("runs".into(), Value::String(FLOW_RUN_TABLE.to_string())),
            ("steps".into(), Value::String(FLOW_STEP_TABLE.to_string())),
            ("terminal".into(), terminal.clone()),
        ];
        if let Some(p) = purge {
            b.push(("purge".into(), p));
        }
        b
    };

    let select = format!(
        "LET $keep = (SELECT VALUE id FROM type::table($runs) \
            WHERE data.status IN $terminal ORDER BY id DESC LIMIT {n});\
         SELECT VALUE data.run_id FROM type::table($runs) \
            WHERE data.status IN $terminal AND id NOT IN $keep;"
    );
    let mut resp = store.query_ws(ws, &select, bindings(None)).await?;
    let purge: Vec<Value> = resp
        .take(1)
        .map_err(|e| StoreError::Decode(format!("retain_runs: purge set: {e}")))?;
    if purge.is_empty() {
        return Ok(0);
    }

    let delete = "DELETE FROM type::table($steps) WHERE data.run_id IN $purge;\
                  DELETE FROM type::table($runs) \
                     WHERE data.status IN $terminal AND data.run_id IN $purge;";
    store
        .query_ws(ws, delete, bindings(Some(Value::Array(purge.clone()))))
        .await?;
    Ok(purge.len())
}
