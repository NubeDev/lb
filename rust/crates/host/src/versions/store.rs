//! Reading the ring back out — the only place `entity_version` is queried
//! (`docs/scope/versions/entity-version-history-scope.md`, "Storage").
//!
//! Rows are written by `lb_store::capped_insert`, which stores them **raw** (no `{data, rev}`
//! envelope) with `cap_key` and `seq` injected. Every read here goes through `query_ws`, so the
//! workspace wall is the store namespace — a ws-B caller physically cannot see ws-A's ring, and
//! there is no `ws` argument anywhere in this file to get wrong.

use serde_json::Value;

use lb_store::{Store, StoreError};

use super::record::{cap_key, EntityVersion, TABLE};

/// The columns a ring row is read back by. Explicitly enumerated rather than `SELECT *` because the
/// stored record also carries SurrealDB's own `id` — a `Thing`, not JSON — which fails to decode
/// into a plain `Value` ("invalid type: enum, expected any valid JSON value"). The row's ULID is
/// already available as `version_id`, so `id` is never needed.
///
/// `version_id` is also what the list ORDERS BY: SurrealDB requires an `ORDER BY` idiom to be
/// literally present in the selection, and `version_id` carries the SAME ULID `capped_insert` writes
/// into `seq` — so ordering by it is ordering by the ring's own FIFO key, with one fewer column.
const COLUMNS: &str =
    "version_id, kind, entity_id, entity_rev, entity_version, tool, actor, ts, hash, snapshot";

/// How many rows a single `versions.list` may return. The ring itself is capped far below this
/// ([`super::cap::MAX_VERSION_CAP`]); the ceiling exists so a caller-supplied `limit` cannot turn a
/// metadata read into a scan.
pub const MAX_LIST: usize = 100;

/// One entity's ring, **newest-first**. Ordered by the row's ULID — the same value the trim orders
/// by — so "what `list` shows" and "what gets evicted next" can never disagree.
pub async fn read_ring(
    store: &Store,
    ws: &str,
    kind: &str,
    id: &str,
    limit: usize,
) -> Result<Vec<EntityVersion>, StoreError> {
    let n = limit.clamp(1, MAX_LIST);
    let sql = format!(
        "SELECT {COLUMNS} FROM type::table($tb) WHERE cap_key = $key ORDER BY version_id DESC LIMIT {n}"
    );
    let mut resp = store
        .query_ws(
            ws,
            &sql,
            vec![
                ("tb".into(), Value::String(TABLE.to_string())),
                ("key".into(), Value::String(cap_key(kind, id))),
            ],
        )
        .await?;
    let rows: Vec<Value> = resp
        .take(0)
        .map_err(|e| StoreError::Decode(e.to_string()))?;
    // A row that fails to decode is SKIPPED, not fatal: one malformed row (an older node's shape)
    // must not make the whole history unreadable, and the ring will age it out.
    Ok(rows
        .into_iter()
        .filter_map(|r| serde_json::from_value::<EntityVersion>(r).ok())
        .collect())
}

/// The ring head's snapshot hash — what the dedupe compares a fresh after-image against. `None` when
/// the ring is empty (the first capture always writes).
pub async fn head_hash(
    store: &Store,
    ws: &str,
    kind: &str,
    id: &str,
) -> Result<Option<String>, StoreError> {
    Ok(read_ring(store, ws, kind, id, 1)
        .await?
        .first()
        .map(|v| v.hash.clone()))
}

/// One version by id, **scoped to the entity it claims to belong to**. The `kind`/`entity_id` match
/// is not redundant with the id lookup: it is what makes "restore version X of dashboard Y" refuse
/// when X is a *flow's* version, so a caller cannot use a `versions.restore` grant on one entity to
/// drive a save on another. A miss (wrong workspace, wrong entity, evicted) is a plain `None` — the
/// caller maps it to `NotFound`, which carries no existence signal about other workspaces.
pub async fn read_version(
    store: &Store,
    ws: &str,
    kind: &str,
    id: &str,
    version_id: &str,
) -> Result<Option<EntityVersion>, StoreError> {
    let sql = format!("SELECT {COLUMNS} FROM ONLY type::record($tb, $id) LIMIT 1");
    let mut resp = store
        .query_ws(
            ws,
            &sql,
            vec![
                ("tb".into(), Value::String(TABLE.to_string())),
                ("id".into(), Value::String(version_id.to_string())),
            ],
        )
        .await?;
    let row: Option<Value> = resp
        .take(0)
        .map_err(|e| StoreError::Decode(e.to_string()))?;
    let Some(v) = row.and_then(|r| serde_json::from_value::<EntityVersion>(r).ok()) else {
        return Ok(None);
    };
    if v.kind != kind || v.entity_id != id {
        return Ok(None);
    }
    Ok(Some(v))
}
