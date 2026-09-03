//! The deadband / min-interval anchor: the last **committed** sample of each `(series, producer)`,
//! read before a commit batch is filtered and written back inside the same transaction
//! (series-normalize scope).
//!
//! **Where it lives, and why there.** On the series' existing `series_meta` row, as a
//! `filter_state` map keyed by producer — the row the commit path already touches for the
//! cardinality cap. A separate table would add a round-trip per batch behind the store's global
//! session mutex; a process-local cache would re-open every deadband on restart and silently store
//! a burst of redundant samples on every reboot.
//!
//! **Keyed per `(series, producer)`, never per series.** Two producers on one series have
//! independent `ts`/`seq` axes, so comparing B's value against A's would filter on a change that
//! never happened. Same axis lesson as
//! `debugging/ingest/latest-pinned-to-pre-restart-sample.md`.
//!
//! The read is SKIPPED entirely when no matched policy has a stateful filter
//! ([`Filter::needs_state`](crate::filter::Filter::needs_state)) — an unfiltered workspace pays
//! nothing for this file existing.

use std::collections::BTreeMap;
use std::collections::HashMap;

use lb_store::{Store, StoreError};
use serde_json::{json, Value};

use crate::filter::LastCommitted;
use crate::schema::SERIES_META_TABLE;

/// One series' anchors, keyed by producer.
pub type ProducerState = BTreeMap<String, LastCommitted>;

/// The `series_meta` field the anchors live in.
pub const FILTER_STATE_FIELD: &str = "filter_state";

/// Read the anchors for `names` in ONE query (never one per series — the commit path runs behind the
/// store's global session mutex). Series with no state yet are simply absent from the map.
pub async fn read_filter_state(
    store: &Store,
    ws: &str,
    names: &[String],
) -> Result<HashMap<String, ProducerState>, StoreError> {
    if names.is_empty() {
        return Ok(HashMap::new());
    }
    let mut resp = store
        .query_ws(
            ws,
            &format!(
                "SELECT series, {FILTER_STATE_FIELD} FROM {SERIES_META_TABLE} \
                 WHERE $names CONTAINS series"
            ),
            vec![("names".into(), json!(names))],
        )
        .await?;
    let rows: Vec<Row> = resp
        .take(0)
        .map_err(|e| StoreError::Decode(e.to_string()))?;
    Ok(rows
        .into_iter()
        .map(|r| (r.series, r.filter_state.unwrap_or_default()))
        .collect())
}

/// One projected `series_meta` row. `filter_state` is absent on every row written before this slice.
#[derive(serde::Serialize, serde::Deserialize)]
struct Row {
    series: String,
    #[serde(default)]
    filter_state: Option<ProducerState>,
}

/// The statement + bindings that persist one series' anchors, to be appended to the commit
/// transaction so the anchor is exactly as durable as the samples it describes.
///
/// A crash between "sample committed" and "anchor advanced" would re-open the deadband for one
/// batch; inside the transaction, it cannot happen. `UPDATE` (not `UPSERT`) because commit has
/// already registered the series — this must never mint a bare `series_meta` row missing its
/// `series` field.
pub fn write_filter_state_sql(idx: usize) -> String {
    let (s, v) = (format!("fss{idx}"), format!("fsv{idx}"));
    format!("UPDATE type::record('{SERIES_META_TABLE}', ${s}) SET {FILTER_STATE_FIELD} = ${v};\n")
}

/// The bindings [`write_filter_state_sql`] expects, for `series` at the same `idx`.
pub fn write_filter_state_bindings(
    idx: usize,
    series: &str,
    state: &ProducerState,
) -> Vec<(String, Value)> {
    vec![
        (format!("fss{idx}"), Value::String(series.to_string())),
        (format!("fsv{idx}"), json!(state)),
    ]
}

// SurrealDB 3 reads query rows through `SurrealValue`. These delegate to serde rather than
// deriving, so `#[serde(default)]` and `deserialize_with = "de_opt_lenient_f64"` keep working
// unchanged — the derive supports neither. See `lb_store::surreal_value_via_serde!`.
lb_store::surreal_value_via_serde!(Row);
