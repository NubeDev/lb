//! `delete_series` — remove a whole series and everything it owns (data-console scope: series
//! lifecycle). A series' state is spread across the plane: the committed sample rows (`series`
//! table), its rollup tiers (`series_rollup`), its registry row (`series_meta`), and its tag-graph
//! edges (the `series:<name>` entity). Deleting the sample rows alone would leave orphaned rollups,
//! a stale cardinality-cap count, and dangling tag edges that `series.find` would still return — so
//! this clears **all four** in one call.
//!
//! Retention policies are NOT touched: they key on a name *prefix*, not a series, so a policy may
//! still cover other series under the same prefix. The `__schema.<name>` meta-series (a UI-side
//! convention, itself just another series) is deleted by the caller as its own `delete_series`.
//!
//! Authorization is NOT here — this is a raw verb run after the host's `caps::check`
//! (capability-first §3.5). Namespace-scoped: every statement runs in `ws` (the hard wall).

use lb_store::{Store, StoreError};
use lb_tags::{entity_parts, TAGGED_TABLE};
use serde_json::Value;

use crate::schema::{ROLLUP_TABLE, SERIES_META_TABLE};
use crate::staging::{SERIES_TABLE, STAGING_TABLE};

/// Delete every trace of `series` in `ws`: committed samples, rollups, any still-staged samples, the
/// registry row, and the series' tag edges. Idempotent — deleting an unknown series is a no-op (each
/// `DELETE` simply matches nothing). Bind the name, never interpolate it.
pub async fn delete_series(store: &Store, ws: &str, series: &str) -> Result<(), StoreError> {
    let entity = format!("series:{series}");
    let (etb, eid) = entity_parts(&entity);
    // One multi-statement query: sample rows (raw + rollup + not-yet-committed staging), the registry
    // row, then the tag edges pointing at the `series:<name>` entity. `sample.series`/`series` are the
    // denormalized name fields; the tag edge links via `in = type::thing($etb, $eid)` (dotted-id safe).
    let sql = format!(
        "DELETE {SERIES_TABLE} WHERE series = $series;
         DELETE {ROLLUP_TABLE} WHERE series = $series;
         DELETE {STAGING_TABLE} WHERE sample.series = $series;
         DELETE {SERIES_META_TABLE} WHERE series = $series;
         DELETE {TAGGED_TABLE} WHERE in = type::thing($etb, $eid);"
    );
    store
        .query_ws(
            ws,
            &sql,
            vec![
                ("series".into(), Value::String(series.to_string())),
                ("etb".into(), Value::String(etb.to_string())),
                ("eid".into(), Value::String(eid.to_string())),
            ],
        )
        .await?;
    Ok(())
}
