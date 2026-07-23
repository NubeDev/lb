//! `series.samples.update` — edit **committed raw** samples of one series in place (payload and/or
//! `ts`). Strict UPDATE semantics, never UPSERT: `UPDATE type::thing(...)` on a missing record is a
//! no-op in the engine, so an update naming a non-existent sample is **skipped** — it can never
//! create a row, and in particular can never plant a row under a foreign producer identity (the
//! `(series, producer, seq)` dedup identity stays owned by whoever committed it).
//!
//! Only the raw tail (`series` table) is editable. Rolled-up history is immutable through this
//! verb — a sample already evicted into `series_rollup` no longer exists as a raw row and simply
//! doesn't match. `seq` and `producer` are NOT editable: they are the record id (the ordering +
//! dedup identity); "move a sample" is delete + rewrite through ingest, not an edit.
//!
//! Authorization is NOT here — raw verb, run after the host's `caps::check` (capability-first
//! §3.5). Namespace-scoped: every statement runs in `ws` (the hard wall).

use lb_store::{Store, StoreError};
use serde::Deserialize;
use serde_json::Value;

use crate::staging::SERIES_TABLE;

/// One in-place edit: the sample's `(producer, seq)` identity plus the fields to replace. At least
/// one of `payload`/`ts` must be set (the host gate enforces it); `ts` is epoch milliseconds and
/// lands as a real datetime, exactly like the commit path.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct SampleUpdate {
    pub producer: String,
    pub seq: u64,
    #[serde(default)]
    pub payload: Option<Value>,
    #[serde(default)]
    pub ts: Option<u64>,
}

/// Apply `updates` to the committed samples of `series` in `ws`. Returns how many rows actually
/// existed and were updated (a missing sample, or an entry with nothing to set, contributes 0).
pub async fn update_samples(
    store: &Store,
    ws: &str,
    series: &str,
    updates: &[SampleUpdate],
) -> Result<usize, StoreError> {
    // Entries with nothing to set would be a SET-less statement — skip them here (the host gate
    // already refuses them as BadInput before this runs).
    let updates: Vec<&SampleUpdate> = updates
        .iter()
        .filter(|u| u.payload.is_some() || u.ts.is_some())
        .collect();
    if updates.is_empty() {
        return Ok(0);
    }
    // One statement pair per edit: bind the LET to the UPDATE's result set, then count it —
    // `count($hit) == 1` iff the row existed (cap.rs's count-a-LET idiom). UPDATE (not UPSERT) is
    // the whole point: a missing id updates nothing and creates nothing.
    let mut sql = String::new();
    let mut bindings: Vec<(String, Value)> =
        vec![("series".into(), Value::String(series.to_string()))];
    for (i, u) in updates.iter().enumerate() {
        let (pr, sq) = (format!("pr{i}"), format!("sq{i}"));
        let mut sets: Vec<String> = Vec::with_capacity(2);
        if let Some(pl) = &u.payload {
            let key = format!("pl{i}");
            sets.push(format!("payload = ${key}"));
            bindings.push((key, pl.clone()));
        }
        if let Some(ts) = u.ts {
            let key = format!("ts{i}");
            sets.push(format!("ts = time::from::millis(${key})"));
            bindings.push((key, Value::Number(ts.into())));
        }
        sql.push_str(&format!(
            "LET $hit{i} = (UPDATE type::thing('{SERIES_TABLE}', [$series, ${pr}, ${sq}]) SET {});
             RETURN count($hit{i});\n",
            sets.join(", ")
        ));
        bindings.push((pr, Value::String(u.producer.clone())));
        bindings.push((sq, Value::Number(u.seq.into())));
    }
    let mut resp = store.query_ws(ws, &sql, bindings).await?;
    // Two statements per edit; the RETURN counts sit at the odd indices.
    let mut updated = 0usize;
    for i in 0..updates.len() {
        let n: Option<i64> = resp
            .take(2 * i + 1)
            .map_err(|e| StoreError::Decode(e.to_string()))?;
        updated += n.unwrap_or(0).max(0) as usize;
    }
    Ok(updated)
}
