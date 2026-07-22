//! `rename_series` — rename a series `from` → `to`, carrying its whole footprint (data-console
//! scope: series lifecycle). A series name is denormalized onto every sample (`series` field), every
//! rollup row, its registry id, and its tag entity (`series:<name>`) — so a rename is a rewrite of
//! all of them, not a metadata tweak.
//!
//! **No silent merge.** The dedup identity is `(series, producer, seq)`; rewriting `from`'s rows to
//! an already-populated `to` could collide two logical samples under one key. So a rename **fails if
//! `to` already exists** (registered or holding rows) — the caller renames into a free name only.
//!
//! Retention policies (prefix-keyed) are left untouched. The UI's `__schema.<name>` meta-series is
//! renamed by the caller as its own `rename_series`. Raw verb — run after `caps::check`. Every
//! statement runs in `ws` (the hard wall).

use lb_store::{Store, StoreError};
use lb_tags::{entity_parts, TAGGED_TABLE};
use serde_json::Value;

use crate::meta::is_registered;
use crate::schema::{ROLLUP_TABLE, SERIES_LATEST_TABLE, SERIES_META_TABLE};
use crate::staging::{SERIES_TABLE, STAGING_TABLE};

/// Why a rename was refused.
#[derive(Debug, thiserror::Error)]
pub enum RenameError {
    #[error("rename: target series already exists: {0}")]
    TargetExists(String),
    #[error("rename: source and target are the same")]
    Unchanged,
    #[error(transparent)]
    Store(#[from] StoreError),
}

/// Rename `from` → `to` in `ws`, carrying its samples, rollups, staged rows, registry row, and tag
/// edges. Fails with [`RenameError::TargetExists`] if `to` is already a series (never merges), and
/// [`RenameError::Unchanged`] if `from == to`. Names are bound, never interpolated.
pub async fn rename_series(
    store: &Store,
    ws: &str,
    from: &str,
    to: &str,
) -> Result<(), RenameError> {
    if from == to {
        return Err(RenameError::Unchanged);
    }
    // Guard the merge: refuse a target that already carries data (registry OR sample rows — a series
    // seeded by a producer registers on commit, so the registry check covers the normal case; the
    // row check defends a partially-migrated series with rows but no registry row).
    if is_registered(store, ws, to).await? || has_rows(store, ws, to).await? {
        return Err(RenameError::TargetExists(to.to_string()));
    }
    let from_entity = format!("series:{from}");
    let to_entity = format!("series:{to}");
    let (from_tb, from_id) = entity_parts(&from_entity);
    let (to_tb, to_id) = entity_parts(&to_entity);
    // Rewrite the denormalized name everywhere, move the registry row (carrying `labels_applied` so
    // the once-per-series latch survives), then re-point the tag edges to the `series:<to>` entity.
    // `series_meta`'s record id IS the name, so the row is deleted + re-inserted rather than updated.
    let sql = format!(
        "UPDATE {SERIES_TABLE} SET series = $to WHERE series = $from;
         UPDATE {ROLLUP_TABLE} SET series = $to WHERE series = $from;
         UPDATE {STAGING_TABLE} SET sample.series = $to WHERE sample.series = $from;
         LET $meta = (SELECT labels_applied FROM type::thing($meta_tb, $from))[0];
         DELETE type::thing($meta_tb, $from);
         UPSERT type::thing($meta_tb, $to) SET series = $to, \
             labels_applied = ($meta.labels_applied OR false);
         LET $ptr = (SELECT producer, seq, ts, payload FROM ONLY type::thing($latest_tb, $from));
         DELETE type::thing($latest_tb, $from);
         IF $ptr != NONE {{ UPSERT type::thing($latest_tb, $to) CONTENT {{ \
             series: $to, producer: $ptr.producer, seq: $ptr.seq, ts: $ptr.ts, payload: $ptr.payload }}; }};
         UPDATE {TAGGED_TABLE} SET in = type::thing($to_tb, $to_id), ent = $to_entity \
             WHERE in = type::thing($from_tb, $from_id);"
    );
    store
        .query_ws(
            ws,
            &sql,
            vec![
                ("from".into(), Value::String(from.to_string())),
                ("to".into(), Value::String(to.to_string())),
                ("meta_tb".into(), Value::String(SERIES_META_TABLE.into())),
                ("latest_tb".into(), Value::String(SERIES_LATEST_TABLE.into())),
                ("from_tb".into(), Value::String(from_tb.to_string())),
                ("from_id".into(), Value::String(from_id.to_string())),
                ("to_tb".into(), Value::String(to_tb.to_string())),
                ("to_id".into(), Value::String(to_id.to_string())),
                ("to_entity".into(), Value::String(to_entity.clone())),
            ],
        )
        .await?;
    Ok(())
}

/// Does `series` have any committed sample rows in `ws`? (The row-level half of the merge guard.)
async fn has_rows(store: &Store, ws: &str, series: &str) -> Result<bool, StoreError> {
    let mut resp = store
        .query_ws(
            ws,
            &format!("SELECT series FROM {SERIES_TABLE} WHERE series = $series LIMIT 1"),
            vec![("series".into(), Value::String(series.to_string()))],
        )
        .await?;
    let rows: Vec<Value> = resp
        .take(0)
        .map_err(|e| StoreError::Decode(e.to_string()))?;
    Ok(!rows.is_empty())
}
