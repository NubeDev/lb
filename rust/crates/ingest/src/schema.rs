//! The series-plane schema — the named indexes + `datetime` time semantics the read fast-paths are
//! pinned on (series schema slice; paging scope "index guarantee"). The `series` table stays
//! SCHEMALESS (payload is any typed value), but three things are DEFINEd:
//!   - `ts` is a real SurrealDB `datetime` (so temporal windowing works), with an **idempotent
//!     migration** for legacy rows that committed `ts` as a plain number (epoch milliseconds);
//!   - a `(series, seq)` index — the keyset-paging seek key (commit order, unique with `producer`);
//!   - a `(series, ts)` index — the wall-clock window scan for bucketed reads and retention GC.
//!
//! `ensure_series_schema` is idempotent (`DEFINE … IF NOT EXISTS` + a type-guarded migration) and
//! cheap after the first call per (process, workspace) — a process-local guard skips repeats, so the
//! commit worker can call it every pass without re-scanning the table.

use std::collections::HashSet;
use std::sync::Mutex;
use std::sync::OnceLock;

use lb_store::{Store, StoreError};

use crate::staging::SERIES_TABLE;

/// The rollup-tier table (retention GC writes, bucketed reads merge). Indexed here alongside the
/// raw series indexes so every series-plane index lives in one place.
pub const ROLLUP_TABLE: &str = "series_rollup";

/// The per-workspace series registry (one row per distinct series name) — the cardinality cap and
/// the label→tag "applied once" flag live here.
pub const SERIES_META_TABLE: &str = "series_meta";

/// Process-local "already ensured" guard, keyed by workspace. The DDL is idempotent anyway; this
/// just skips the migration UPDATE re-scan on every commit pass.
fn ensured() -> &'static Mutex<HashSet<String>> {
    static ENSURED: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    ENSURED.get_or_init(|| Mutex::new(HashSet::new()))
}

/// Define the series-plane schema in `ws`: migrate legacy numeric `ts` rows to `datetime`, then pin
/// the `(series, seq)` and `(series, ts)` indexes (+ the rollup-tier index). Idempotent.
pub async fn ensure_series_schema(store: &Store, ws: &str) -> Result<(), StoreError> {
    if ensured().lock().expect("schema guard").contains(ws) {
        return Ok(());
    }
    // Migration FIRST (a numeric `ts` under a datetime-typed index definition would be rejected):
    // legacy rows committed `ts` as epoch milliseconds; convert in place. Type-guarded → idempotent.
    let sql = format!(
        "UPDATE {SERIES_TABLE} SET ts = time::from::millis(ts) WHERE type::is::number(ts);
         DEFINE FIELD IF NOT EXISTS ts ON {SERIES_TABLE} TYPE datetime;
         DEFINE INDEX IF NOT EXISTS series_seq_idx ON {SERIES_TABLE} FIELDS series, seq;
         DEFINE INDEX IF NOT EXISTS series_ts_idx ON {SERIES_TABLE} FIELDS series, ts;
         DEFINE INDEX IF NOT EXISTS series_rollup_idx ON {ROLLUP_TABLE} FIELDS series, width_ms, t;"
    );
    store.query_ws(ws, &sql, vec![]).await?;
    ensured()
        .lock()
        .expect("schema guard")
        .insert(ws.to_string());
    Ok(())
}
