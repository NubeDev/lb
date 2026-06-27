//! `series.read(series, range)` — a range query over the committed series. Because a sample's id is
//! the composite `[series, producer, seq]`, "all committed samples for a series" is a fast filter on
//! the `series` field, ordered by `seq` (the monotonic ordering key — never wall-clock `ts`, which is
//! untrusted data). The range is an inclusive `[from_seq, to_seq]` over `seq`; either bound may be
//! `None` for "open-ended" (ingest scope).
//!
//! An open upper bound is `None`, NOT `u64::MAX` — passing `u64::MAX` as a bound is a footgun: the
//! engine coerces a near-`2^64` integer to a float and the `seq <=` comparison silently mis-evaluates
//! to false, returning nothing (debugging/ingest/u64-max-bound-coerces-to-float.md). So we OMIT the
//! clause entirely when a bound is open, rather than bind a sentinel.
//!
//! Namespace-scoped — a ws-B read can physically only return ws-B's samples (the hard wall). Raw
//! verb, run after `caps::check`.

use lb_store::{Store, StoreError};
use serde_json::Value;

use crate::sample::Sample;
use crate::staging::SERIES_TABLE;

/// Return every committed sample of `series` in `ws` whose `seq` is in the (optionally open) range
/// `[from_seq, to_seq]`, ordered by `seq` ascending. `None` bounds are open-ended. Spans all
/// producers of the series (each is a distinct `[series, producer, seq]` row).
pub async fn read(
    store: &Store,
    ws: &str,
    series: &str,
    from_seq: Option<u64>,
    to_seq: Option<u64>,
) -> Result<Vec<Sample>, StoreError> {
    let mut clauses = String::from("series = $series");
    let mut bindings: Vec<(String, Value)> =
        vec![("series".into(), Value::String(series.to_string()))];
    if let Some(from) = from_seq {
        clauses.push_str(" AND seq >= $from");
        bindings.push(("from".into(), Value::Number(from.into())));
    }
    if let Some(to) = to_seq {
        clauses.push_str(" AND seq <= $to");
        bindings.push(("to".into(), Value::Number(to.into())));
    }
    let sql = format!(
        "SELECT series, producer, seq, ts, payload FROM {SERIES_TABLE} \
         WHERE {clauses} ORDER BY seq ASC"
    );
    let mut resp = store.query_ws(ws, &sql, bindings).await?;
    let rows: Vec<Sample> = resp
        .take(0)
        .map_err(|e| StoreError::Decode(e.to_string()))?;
    Ok(rows)
}
