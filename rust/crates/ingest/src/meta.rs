//! The per-workspace series registry — one `series_meta` row per distinct series name. Three jobs
//! (series schema slice):
//!   - the **cardinality cap**: the count of rows here is "how many distinct series this workspace
//!     has", checked before a commit admits a NEW series name (the ingest scope's highest-risk
//!     item — unbounded series names are unbounded index + tag growth);
//!   - the **label→tag flag**: `labels_applied` records that a series' wire labels were converted
//!     to tag edges, so the conversion runs once per series, not once per sample.
//!   - the **source unit** (`unit`): the series' provenance, so a value read back can be converted
//!     into the unit a viewer's prefs ask for.
//!
//! # Why the unit lives HERE
//!
//! `format.quantity`/`convert.unit` need a `from_unit`, and before this column nothing on the data
//! path carried one: the registry had two columns, `series.list` returned bare strings, and the viz
//! `Field` had no unit — so a correct converter sat unreachable behind missing provenance. A unit
//! belongs to the SERIES, not the sample: every point in `sensor.tank.level` is in the same unit,
//! and storing it per-sample would repeat one token across millions of rows.
//!
//! The column is **optional and validated**. Optional because every series that exists today has no
//! unit and must keep working (an absent unit means "unknown", and the caller renders the canonical
//! value with no conversion — today's behaviour exactly). Validated against the closed
//! [`lb_prefs::axis::Unit`] enum because an unparseable free-text unit (`"degrees celsius"`) is
//! worse than none: it would look like provenance and convert to nothing.

use lb_prefs::axis::Unit;
use lb_store::{Store, StoreError};
use serde_json::Value;

use crate::schema::SERIES_META_TABLE;

/// Default cap on distinct series names per workspace.
pub const DEFAULT_SERIES_CAP: usize = 10_000;

/// Count of registered (distinct) series names in `ws`.
pub async fn series_count(store: &Store, ws: &str) -> Result<usize, StoreError> {
    let mut resp = store
        .query_ws(
            ws,
            &format!("SELECT count() FROM {SERIES_META_TABLE} GROUP ALL"),
            vec![],
        )
        .await?;
    let n: Option<i64> = resp
        .take("count")
        .map_err(|e| StoreError::Decode(e.to_string()))?;
    Ok(n.unwrap_or(0).max(0) as usize)
}

/// Is `series` already registered in `ws`?
pub async fn is_registered(store: &Store, ws: &str, series: &str) -> Result<bool, StoreError> {
    let mut resp = store
        .query_ws(
            ws,
            &format!("SELECT series FROM type::thing('{SERIES_META_TABLE}', $series)"),
            vec![("series".into(), Value::String(series.to_string()))],
        )
        .await?;
    let rows: Vec<Value> = resp
        .take(0)
        .map_err(|e| StoreError::Decode(e.to_string()))?;
    Ok(!rows.is_empty())
}

/// Register `series` (idempotent; preserves an existing `labels_applied` and `unit`).
pub async fn register(store: &Store, ws: &str, series: &str) -> Result<(), StoreError> {
    store
        .query_ws(
            ws,
            &format!(
                "UPSERT type::thing('{SERIES_META_TABLE}', $series) SET series = $series, \
                 labels_applied = labels_applied OR false"
            ),
            vec![("series".into(), Value::String(series.to_string()))],
        )
        .await?;
    Ok(())
}

/// The series' declared source unit, or `None` when it has never declared one (the ordinary case
/// for every series that predates the column). A stored token that no longer parses is treated as
/// absent rather than an error — the vocabulary is closed, so a value outside it is not provenance
/// the converter can honour, and a read path must not fail over it.
pub async fn unit(store: &Store, ws: &str, series: &str) -> Result<Option<Unit>, StoreError> {
    let mut resp = store
        .query_ws(
            ws,
            &format!("SELECT unit FROM type::thing('{SERIES_META_TABLE}', $series)"),
            vec![("series".into(), Value::String(series.to_string()))],
        )
        .await?;
    let rows: Vec<Value> = resp
        .take("unit")
        .map_err(|e| StoreError::Decode(e.to_string()))?;
    Ok(rows.first().and_then(Value::as_str).and_then(Unit::parse))
}

/// Declare `series`' source unit. Registers the series if it is not yet known, so a producer can
/// declare a unit and write in either order. Idempotent; a later call REPLACES the unit (a sensor
/// genuinely rescaled is a real event, and the alternative — a first-write-wins latch — would make
/// a typo permanent).
pub async fn set_unit(store: &Store, ws: &str, series: &str, unit: Unit) -> Result<(), StoreError> {
    store
        .query_ws(
            ws,
            &format!(
                "UPSERT type::thing('{SERIES_META_TABLE}', $series) SET series = $series, \
                 labels_applied = labels_applied OR false, unit = $unit"
            ),
            vec![
                ("series".into(), Value::String(series.to_string())),
                ("unit".into(), Value::String(unit.as_str().to_string())),
            ],
        )
        .await?;
    Ok(())
}

/// The `(name, unit)` pairs for the registered series in `ws` matching `prefix`, ascending — the
/// listing `series.list` serves when a caller asks for metadata. One query, same shape and cost as
/// [`series_names`]; a series with no declared unit yields `None`.
pub async fn series_units(
    store: &Store,
    ws: &str,
    prefix: &str,
) -> Result<Vec<(String, Option<Unit>)>, StoreError> {
    let mut resp = store
        .query_ws(
            ws,
            &format!(
                "SELECT series, unit FROM {SERIES_META_TABLE} \
                 WHERE string::starts_with(series, $prefix) ORDER BY series ASC"
            ),
            vec![("prefix".into(), Value::String(prefix.to_string()))],
        )
        .await?;
    let rows: Vec<Value> = resp
        .take(0)
        .map_err(|e| StoreError::Decode(e.to_string()))?;
    Ok(rows
        .iter()
        .filter_map(|r| {
            let name = r.get("series")?.as_str()?.to_string();
            let unit = r.get("unit").and_then(Value::as_str).and_then(Unit::parse);
            Some((name, unit))
        })
        .collect())
}

/// Has this series' labels already been converted to tag edges?
pub async fn labels_applied(store: &Store, ws: &str, series: &str) -> Result<bool, StoreError> {
    let mut resp = store
        .query_ws(
            ws,
            &format!("SELECT labels_applied FROM type::thing('{SERIES_META_TABLE}', $series)"),
            vec![("series".into(), Value::String(series.to_string()))],
        )
        .await?;
    let rows: Vec<Value> = resp
        .take("labels_applied")
        .map_err(|e| StoreError::Decode(e.to_string()))?;
    Ok(rows.first().and_then(|v| v.as_bool()).unwrap_or(false))
}

/// Mark the series' labels as converted (the once-per-series latch).
pub async fn mark_labels_applied(store: &Store, ws: &str, series: &str) -> Result<(), StoreError> {
    store
        .query_ws(
            ws,
            &format!(
                "UPDATE type::thing('{SERIES_META_TABLE}', $series) SET labels_applied = true"
            ),
            vec![("series".into(), Value::String(series.to_string()))],
        )
        .await?;
    Ok(())
}

/// The registered series names in `ws` starting with `prefix` (empty = all), ascending.
pub async fn series_names(
    store: &Store,
    ws: &str,
    prefix: &str,
) -> Result<Vec<String>, StoreError> {
    let mut resp = store
        .query_ws(
            ws,
            &format!(
                "SELECT series FROM {SERIES_META_TABLE} \
                 WHERE string::starts_with(series, $prefix) ORDER BY series ASC"
            ),
            vec![("prefix".into(), Value::String(prefix.to_string()))],
        )
        .await?;
    let names: Vec<String> = resp
        .take("series")
        .map_err(|e| StoreError::Decode(e.to_string()))?;
    Ok(names)
}
