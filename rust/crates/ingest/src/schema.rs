//! The series-plane schema — the named indexes + `datetime` time semantics the read fast-paths are
//! pinned on (series schema slice; paging scope "index guarantee"). The `series` table stays
//! SCHEMALESS (payload is any typed value), but three things are DEFINEd:
//!   - `ts` is a real SurrealDB `datetime` (so temporal windowing works), with an **idempotent
//!     migration** for legacy rows that committed `ts` as a plain number (epoch milliseconds);
//!   - a `(series, seq)` index — the keyset-paging seek key (commit order, unique with `producer`);
//!   - a `(series, ts)` index — the wall-clock window scan, **off by default**: see below.
//!
//! # The `(series, ts)` index is opt-in, and defaults OFF
//!
//! SurrealDB 3.2.4 answers a DATETIME range over an indexed field WRONGLY: it admits rows outside
//! the bounds and drops the `ORDER BY` entirely. Measured in
//! `store/tests/index_range_scan_probe.rs`; the same statement is correct with the index absent, and
//! correct for a NUMERIC second field (so `(series, seq)` and the rollup index are unaffected — it
//! is datetime specifically). It surfaced as a paged read returning a sample 500 ms below its own
//! window, which is the silent-wrong-data failure a timeseries store can least afford.
//!
//! So the index is created only when an operator asks for it ([`set_series_time_index`], filled from
//! `BootConfig` at the binary boundary), and is REMOVED when they have not — otherwise a store that
//! once had it would keep serving wrong rows after the upgrade. Turn it on to trade correctness for
//! scan speed, or once the engine is fixed.
//!
//! `ensure_series_schema` is idempotent (`DEFINE … IF NOT EXISTS` + a type-guarded migration) and
//! cheap after the first call per (process, workspace) — a process-local guard skips repeats, so the
//! commit worker can call it every pass without re-scanning the table.

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::sync::OnceLock;

use lb_store::{Store, StoreError};

use crate::tables::SERIES_TABLE;

/// The rollup-tier table (retention GC writes, bucketed reads merge). Indexed here alongside the
/// raw series indexes so every series-plane index lives in one place.
pub const ROLLUP_TABLE: &str = "series_rollup";

/// The per-workspace series registry (one row per distinct series name) — the cardinality cap and
/// the label→tag "applied once" flag live here.
pub const SERIES_META_TABLE: &str = "series_meta";

/// The newest-sample POINTER table (one row per series, at `series_latest:[series]`) — a materialized
/// "latest" maintained transactionally by the commit worker so `series.latest`/`series.latest_many`
/// are O(1)/O(k) point lookups instead of an `ORDER BY ts DESC LIMIT 1` that SurrealDB 2.6.5 serves
/// with a full in-memory sort of the whole series (`MemoryOrderedLimit` — the index only narrows the
/// `series=` equality, never the ordered limit; verified by EXPLAIN on a 10k-row series taking ~1 s).
/// NOT a device shadow / cache: it advances only when a newly-committed sample beats the stored
/// `(ts, seq)`, in the SAME transaction as the raw write, so it cannot go stale under late/replayed
/// samples. `latest.rs` reads it; `commit.rs` writes it; `delete`/`rename` keep it consistent.
pub const SERIES_LATEST_TABLE: &str = "series_latest";

/// Process-local "already ensured" guard, keyed by workspace. The DDL is idempotent anyway; this
/// just skips the migration UPDATE re-scan on every commit pass.
fn ensured() -> &'static Mutex<HashSet<String>> {
    static ENSURED: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    ENSURED.get_or_init(|| Mutex::new(HashSet::new()))
}

/// Define the series-plane schema in `ws`: migrate legacy numeric `ts` rows to `datetime`, then pin
/// the `(series, seq)` and `(series, ts)` indexes (+ the rollup-tier index). Idempotent.
/// Whether to define the `(series, ts)` index. OFF unless an operator turns it on.
///
/// Process state rather than a parameter: the only production caller is the commit worker, deep in
/// the write path, and threading a boot flag down to it would put configuration in every signature
/// between. Set ONCE from `BootConfig` at boot ([`set_series_time_index`]) — this crate reads no
/// environment of its own (CLAUDE §5: env is a binary concern).
static TIME_INDEX: AtomicBool = AtomicBool::new(false);

/// Enable or disable the `(series, ts)` index, from `BootConfig`. Call once, at boot, before the
/// first commit. Read the module note before turning it ON.
pub fn set_series_time_index(on: bool) {
    TIME_INDEX.store(on, Ordering::Relaxed);
}

/// Is the `(series, ts)` index enabled?
pub fn series_time_index_enabled() -> bool {
    TIME_INDEX.load(Ordering::Relaxed)
}

pub async fn ensure_series_schema(store: &Store, ws: &str) -> Result<(), StoreError> {
    if ensured().lock().expect("schema guard").contains(ws) {
        return Ok(());
    }
    // Migration FIRST (a numeric `ts` under a datetime-typed index definition would be rejected):
    // legacy rows committed `ts` as epoch milliseconds; convert in place. Type-guarded → idempotent.
    let sql = format!(
        "UPDATE {SERIES_TABLE} SET ts = time::from_millis(ts) WHERE type::is_number(ts);
         DEFINE FIELD IF NOT EXISTS ts ON {SERIES_TABLE} TYPE datetime;
         DEFINE INDEX IF NOT EXISTS series_seq_idx ON {SERIES_TABLE} FIELDS series, seq;
         {time_index}
         DEFINE INDEX IF NOT EXISTS series_rollup_idx ON {ROLLUP_TABLE} FIELDS series, width_ms, t;",
        // REMOVE, not merely "don't define": a store upgraded from an earlier build already carries
        // this index, and leaving it would keep that store answering datetime windows wrongly.
        time_index = if series_time_index_enabled() {
            format!("DEFINE INDEX IF NOT EXISTS series_ts_idx ON {SERIES_TABLE} FIELDS series, ts;")
        } else {
            format!("REMOVE INDEX IF EXISTS series_ts_idx ON {SERIES_TABLE};")
        }
    );
    store.query_ws(ws, &sql, vec![]).await?;
    ensured()
        .lock()
        .expect("schema guard")
        .insert(ws.to_string());
    Ok(())
}
