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
use lb_ingest::SERIES_TABLE;
use lb_store::Store;

use super::authorize::authorize_ingest;
use super::error::IngestError;

/// The hard cap on how many distinct series names one list returns — a workspace with a huge number
/// of series still returns a bounded set (the `store.scan` bound, applied to discovery).
pub const MAX_SERIES_LIST: usize = 500;

/// List the distinct series names in `ws` whose name starts with `prefix` (empty `prefix` = all),
/// sorted ascending, bounded by [`MAX_SERIES_LIST`]. Gated by `mcp:series.list:call`.
pub async fn series_list(
    store: &Store,
    principal: &Principal,
    ws: &str,
    prefix: &str,
) -> Result<Vec<String>, IngestError> {
    authorize_ingest(principal, ws, "series.list")?;

    // Project the distinct `series` field off the committed series table. `GROUP BY series`
    // collapses the many `[series, producer, seq]` rows of one series to a single row;
    // `string::starts_with` filters by prefix (empty = all). `prefix` is bound, never interpolated.
    // We project a named field (`series AS name`) rather than `SELECT VALUE … GROUP BY` (which
    // mis-projects to `{series: None}` under grouping — see the GROUP-BY projection quirk).
    let sql = format!(
        "SELECT series AS name FROM {SERIES_TABLE} \
         WHERE string::starts_with(series, $prefix) \
         GROUP BY name ORDER BY name ASC LIMIT {MAX_SERIES_LIST}"
    );
    let mut resp = store
        .query_ws(
            ws,
            &sql,
            vec![(
                "prefix".into(),
                serde_json::Value::String(prefix.to_string()),
            )],
        )
        .await?;
    let rows: Vec<NameRow> = resp
        .take(0)
        .map_err(|e| IngestError::Store(lb_store::StoreError::Decode(e.to_string())))?;
    Ok(dedup_sorted(rows.into_iter().map(|r| r.name).collect()))
}

/// `GROUP BY series` already collapses duplicates, but defend against engine quirks: dedup a sorted
/// list in place. (`SELECT VALUE … GROUP BY` returns each group once; this is belt-and-braces.)
fn dedup_sorted(mut names: Vec<String>) -> Vec<String> {
    names.dedup();
    names
}

#[derive(serde::Deserialize)]
struct NameRow {
    name: String,
}
