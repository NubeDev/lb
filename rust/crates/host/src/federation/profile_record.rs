//! The `datasource_profile:{ws}:{source}` store record (datasource-profile scope) — the durable
//! per-source **discovery profile**: per table, the columns and their kinds, the real foreign keys,
//! per-text-column cardinality + top values, per-numeric min/max + null fraction, and the grouped
//! value ranges that separate a metric column from a place column.
//!
//! **Derived, always rebuildable** — the embeddings doctrine. Wiping this table loses nothing; the
//! next pass recomputes it from the source. That is what makes it safe to keep it off the read
//! path's critical section and refresh it on a clock.
//!
//! **Rule 10:** `tables` is stored as opaque JSON exactly as the sidecar emitted it. The host never
//! reinterprets per-kind detail, so a new source kind needs no host change — and no kind can be
//! special-cased here even by accident.
//!
//! **No DSN, ever.** The connection string is mediated host-side into the sidecar call and is not
//! part of the pass's result; nothing here can carry it.

use lb_store::{read, write, Store, StoreError};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The current record shape version. A future reader compares to up-convert (the `db_schema`
/// precedent). Distinct from the sidecar's `PROFILE_VERSION`, which versions the per-table payload.
pub const PROFILE_RECORD_VERSION: u32 = 1;

/// The store table for datasource-profile records (one place owns the name).
pub const TABLE: &str = "datasource_profile";

/// How long a `profiling_since` stamp is honoured as "a pass is in flight" before another enqueue is
/// allowed. Without an expiry, a node that died mid-pass would leave the source permanently
/// un-refreshable; with one, the worst case is a duplicated pass an hour later.
pub const PROFILING_GUARD_SECS: u64 = 3600;

/// A datasource's discovery profile. Keyed by the source alias within the workspace namespace, so
/// the full id is `datasource_profile:{ws}:{source}` and a ws-B read of a ws-A source finds nothing.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DatasourceProfile {
    /// The datasource alias this profiles (the record key).
    pub source: String,
    /// The record shape version.
    pub version: u32,
    /// When the last completed pass landed (caller-injected logical time — no wall clock).
    pub profiled_at: u64,
    /// Set while a pass is IN FLIGHT, cleared when it lands. The reactor's idempotence guard: a
    /// stale record already being profiled must not be enqueued again on the next tick.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profiling_since: Option<u64>,
    /// The sidecar's per-table sub-objects, verbatim. Per-table (not one flat blob) so a consumer
    /// can ref a single table's slice within the context-basket body budget.
    #[serde(default)]
    pub tables: Vec<Value>,
    /// True when the pass hit a bound (tables cut, or a cardinality count capped) — the profile is
    /// honest about being partial rather than silently reading as complete.
    #[serde(default)]
    pub truncated: bool,
    /// The constant discriminator so the reactor's scan (and any future list verb) has an indexable
    /// handle — the store has no "list a whole table" verb.
    #[serde(default = "profile_tag")]
    pub tag: String,
    /// A soft-delete marker, mirroring `datasource.remove`: a removed source's profile reads as
    /// absent (the store has no delete verb; a tombstone keeps the id stable + idempotent).
    #[serde(default)]
    pub removed: bool,
}

/// The constant `tag` value every profile record carries (the scan discriminator).
pub fn profile_tag() -> String {
    "datasource_profile".to_string()
}

impl DatasourceProfile {
    /// Build a landed profile from a sidecar pass result.
    pub fn from_pass(source: &str, pass: &Value, profiled_at: u64) -> Self {
        Self {
            source: source.to_string(),
            version: PROFILE_RECORD_VERSION,
            profiled_at,
            profiling_since: None,
            tables: pass
                .get("tables")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default(),
            truncated: pass
                .get("truncated")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            tag: profile_tag(),
            removed: false,
        }
    }
}

/// Ensure the `(data.tag, data.profiled_at)` composite index exists on the profile table in `ws`.
///
/// **Load-bearing.** The reactor selects stale profiles by `tag` + `profiled_at` on every tick;
/// without this index SurrealDB scans the whole table, which is precisely the reactor-rescan CPU
/// burn that pegged a Pi (`docs/debugging/jobs/node-pegs-cpu-reactor-rescans-job-table.md`).
///
/// Lazy, per-namespace, idempotent — the `lb_jobs::define_job_index` convention. Note the `data.`
/// prefix: every store write nests the host body under `data`, so an index on bare `profiled_at`
/// would be silently ignored and the query would scan.
pub async fn define_profile_index(store: &Store, ws: &str) -> Result<(), StoreError> {
    let sql = format!(
        "DEFINE INDEX IF NOT EXISTS datasource_profile_stale ON TABLE {TABLE} \
         COLUMNS data.tag, data.profiled_at"
    );
    store.query_ws(ws, &sql, vec![]).await?;
    Ok(())
}

/// Persist (upsert) a profile record in `ws`. Workspace-namespaced by the store (the hard wall).
pub async fn put(store: &Store, ws: &str, rec: &DatasourceProfile) -> Result<(), StoreError> {
    define_profile_index(store, ws).await?;
    let value = serde_json::to_value(rec).map_err(|e| StoreError::Decode(e.to_string()))?;
    write(store, ws, TABLE, &rec.source, &value).await
}

/// Resolve `source`'s profile in `ws`. `None` if never profiled OR tombstoned — which is exactly
/// what a cross-tenant name resolves to (a ws-B caller naming a ws-A source finds nothing).
pub async fn resolve(
    store: &Store,
    ws: &str,
    source: &str,
) -> Result<Option<DatasourceProfile>, StoreError> {
    let Some(value) = read(store, ws, TABLE, source).await? else {
        return Ok(None);
    };
    let rec: DatasourceProfile =
        serde_json::from_value(value).map_err(|e| StoreError::Decode(e.to_string()))?;
    if rec.removed {
        return Ok(None);
    }
    Ok(Some(rec))
}

/// The profiles in `ws` older than `cutoff`, **index-backed and LIMIT-bounded**.
///
/// Both bounds matter: the `WHERE` rides the composite index defined above, and the `LIMIT` caps how
/// much one tick can do regardless. A tick that finds 10 000 stale profiles enqueues `limit` of them
/// and picks the rest up next tick — bounded work per tick, never a burst.
pub async fn stale(
    store: &Store,
    ws: &str,
    cutoff: u64,
    limit: usize,
) -> Result<Vec<DatasourceProfile>, StoreError> {
    define_profile_index(store, ws).await?;
    let sql = format!(
        "SELECT data FROM type::table($tb) \
         WHERE data.tag = $tag AND data.profiled_at < $cutoff LIMIT {limit}"
    );
    let mut resp = store
        .query_ws(
            ws,
            &sql,
            vec![
                ("tb".to_string(), Value::String(TABLE.to_string())),
                ("tag".to_string(), Value::String(profile_tag())),
                ("cutoff".to_string(), Value::from(cutoff)),
            ],
        )
        .await?;
    let rows: Vec<Value> = resp
        .take(0)
        .map_err(|e| StoreError::Decode(e.to_string()))?;
    Ok(rows
        .into_iter()
        // Each row nests the body under `data`; a row that fails to decode is skipped, never fatal
        // (one malformed record must not stop the reactor for the whole workspace).
        .filter_map(|row| {
            let body = row.get("data").cloned().unwrap_or(row);
            serde_json::from_value::<DatasourceProfile>(body).ok()
        })
        .filter(|p| !p.removed)
        .collect())
}
