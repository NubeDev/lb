//! `series.list(prefix)` — list the distinct series **names** in a workspace, optionally filtered by
//! a name prefix. The ingest scope names this verb; the data-console scope's open question ("dedicated
//! `series.list` vs. `series.find([])`") is resolved **here, in favour of the small verb**: prefix
//! listing over the committed `series` table and tag-faceted discovery (`series.find`) are different
//! queries — a `series.find` with empty facets returns nothing by design (a query must constrain
//! something), and listing should not require a tag to exist.
//!
//! Gated by `mcp:series.list:call`; namespace-scoped (the hard wall) — a ws-B caller lists ws-B's
//! series only. A denial is opaque. Read-only.

use lb_auth::Principal;
use lb_ingest::series_names;
use lb_store::Store;

use super::authorize::authorize_ingest;
use super::error::IngestError;

/// The hard cap on how many distinct series names one list returns — a workspace with a huge number
/// of series still returns a bounded set (the `store.scan` bound, applied to discovery).
pub const MAX_SERIES_LIST: usize = 500;

/// List the distinct series names in `ws` whose name starts with `prefix` (empty `prefix` = all),
/// sorted ascending, bounded by [`MAX_SERIES_LIST`]. Gated by `mcp:series.list:call`.
///
/// Reads the **`series_meta` registry** (one row per distinct series name), NOT a `GROUP BY` over the
/// committed `series` samples table. The samples table holds one row per `[series, producer, seq]`
/// datapoint — grouping it to recover 40 names scanned the whole table (seconds on any real ingest
/// volume; `string::starts_with` in the WHERE also defeats the `(series, …)` indexes). The registry is
/// the purpose-built listing source: `lb_ingest::series_names` runs the same prefix filter over one row
/// per series, so the read is proportional to the series COUNT, not the sample count.
pub async fn series_list(
    store: &Store,
    principal: &Principal,
    ws: &str,
    prefix: &str,
) -> Result<Vec<String>, IngestError> {
    authorize_ingest(principal, ws, "series.list")?;

    // Registry-backed listing (fast): one row per series. Already sorted ascending; apply the same
    // discovery cap the samples-scan path used so a huge workspace still returns a bounded set.
    let mut names = series_names(store, ws, prefix)
        .await
        .map_err(IngestError::Store)?;
    names.truncate(MAX_SERIES_LIST);
    Ok(names)
}
