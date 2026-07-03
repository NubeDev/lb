//! The `job` table's index definition — the one place that owns the `(kind, status)` composite
//! index [`pending`](crate::pending) queries against. Without it, the drain query silently falls
//! back to a full table scan and the CPU-burn this crate's retention work fixes comes right back
//! (see `docs/debugging/jobs/node-pegs-cpu-reactor-rescans-job-table.md`).
//!
//! **Field path is `data.kind` / `data.status`, not `kind` / `status`.** Every store write nests
//! the host body under a single `data` field (`lb_store::record`), so a job's `kind`/`status` live
//! at `data.kind`/`data.status` on the stored row. The index must target the exact path the query
//! filters on, or SurrealDB ignores it and scans (the scope's "index correctness on the stored
//! shape" risk).
//!
//! **Lazy, per-namespace, idempotent.** Workspaces are SurrealDB namespaces; a `DEFINE INDEX` runs
//! in whichever namespace `query_ws` selected, so it must run once per workspace. Following the
//! established convention (`lb_prefs::define_prefs_schema`, `lb_tags` counts view), we ensure it on
//! first touch — [`create`](crate::create) calls this before its write — with `IF NOT EXISTS` so a
//! re-run on every create is a cheap no-op. There is no global boot-time schema pass to hang it on.

use lb_store::{Store, StoreError};

/// The index name — stable so `IF NOT EXISTS` recognises a prior definition across restarts.
const JOB_KIND_STATUS_INDEX: &str = "job_kind_status";

/// Ensure the `(data.kind, data.status)` composite index exists on the `job` table in workspace
/// `ws`. Idempotent (`IF NOT EXISTS`) and namespace-scoped (`query_ws` selects `ws` first), so the
/// reactor's [`pending`](crate::pending) drain query is an index lookup — O(pending) — not a scan.
pub async fn define_job_index(store: &Store, ws: &str) -> Result<(), StoreError> {
    let sql = format!(
        "DEFINE INDEX IF NOT EXISTS {JOB_KIND_STATUS_INDEX} ON TABLE {table} \
         COLUMNS data.kind, data.status",
        table = super::TABLE,
    );
    store.query_ws(ws, &sql, vec![]).await?;
    Ok(())
}
