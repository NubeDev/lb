//! The `(cap_key, seq)` index every [`capped_insert`](crate::capped_insert) table needs — the one
//! place that owns it.
//!
//! The trim reads "the first row past the newest `cap` for one key, by `seq`" and deletes from
//! there down. Without this index that read is a **full table scan**, and a ring holds `cap` rows
//! for EVERY key: the telemetry ring at the default 1000/source sits at ~20k rows on a busy node, so
//! every telemetry event paid a whole-ring scan and a busy node pegged several cores
//! (`docs/debugging/store/capped-insert-trim-scans-whole-ring.md`). With it, the read walks one
//! key's rows in `seq` order (`capped_test::trim_reads_one_key_through_the_index` pins the plan).
//!
//! **Lazy, per-namespace, idempotent** — the convention `lb_jobs::define_job_index` set: workspaces
//! are namespaces, there is no boot-time schema pass, so it is ensured on first touch with
//! `IF NOT EXISTS`. Unlike the job index it is NOT re-issued on every write: a capped table takes a
//! write per telemetry event, and a schema transaction on that path would contend with itself.
//!
//! **Once, single-flight.** Each `(ws, table)` has one `OnceCell` on the [`Store`]. Concurrent first
//! writers wait for the one build in flight instead of each starting their own — SurrealDB gives
//! every `DEFINE INDEX` a fresh build, so N racing callers would scan the ring N times at once,
//! right after an upgrade, which is the spike this file exists to remove.
//!
//! **Best effort.** The index makes the trim cheap; the trim is correct without it. A failed DDL is
//! logged and the insert goes ahead, and the cell stays empty so the next insert tries again.
//! Failing the insert instead would drop a telemetry row silently (the layer is fire-and-forget) and
//! fail an insight raise after its parent counter had already moved.
//!
//! The memory is never cleared. Nothing removes a capped table or its index while a store is open
//! (`telemetry.purge` deletes rows; the index stays). If something ever does, the cost is a slow
//! trim until the store reopens, never a wrong one.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tokio::sync::OnceCell;

use crate::open::{Store, StoreError};
use crate::write_batch::is_bare_ident;

/// One build slot per `(ws, table)` whose index this store has ensured or is ensuring. Lives on
/// the [`Store`] (not a process-wide static) so it dies with the store: two stores in one process,
/// or a store reopened at a fresh path, each ensure their own index.
#[derive(Default)]
pub(crate) struct CappedIndexes(Mutex<HashMap<String, Arc<OnceCell<()>>>>);

impl CappedIndexes {
    fn slot(&self, ws: &str, table: &str) -> Arc<OnceCell<()>> {
        self.0
            .lock()
            .expect("capped index map poisoned")
            .entry(format!("{ws}\u{1}{table}"))
            .or_default()
            .clone()
    }
}

/// The index name. Stable so `IF NOT EXISTS` recognises a prior definition across restarts; index
/// names are per table, so one name serves every capped table.
const INDEX: &str = "capped_key_seq";

/// Ensure the `(cap_key, seq)` index on `table` in workspace `ws`, once per store.
///
/// `table` is interpolated into DDL (an index target cannot be a bound parameter), so it must be a
/// bare identifier; every caller passes a compiled table-name constant. Anything else is refused
/// (`Err`) rather than escaped. A DDL failure is NOT an error here — see the module note.
pub(crate) async fn ensure_capped_index(
    store: &Store,
    ws: &str,
    table: &str,
) -> Result<(), StoreError> {
    if !is_bare_ident(table) {
        return Err(StoreError::Decode(format!(
            "capped_insert: table {table:?} is not a plain identifier"
        )));
    }
    let slot = store.capped_indexes().slot(ws, table);
    let sql = format!("DEFINE INDEX IF NOT EXISTS {INDEX} ON TABLE {table} FIELDS cap_key, seq");
    // Retrying: a capped write in the same namespace can still conflict with the DDL, and SurrealDB
    // aborts one side with a retryable conflict (the job-index incident). The statement is idempotent.
    let built = slot
        .get_or_try_init(|| async { store.query_ws_retrying(ws, &sql, vec![]).await.map(|_| ()) })
        .await;
    if let Err(e) = built {
        tracing::warn!(ws, table, error = %e, "capped ring index not ensured; trimming without it");
    }
    Ok(())
}
