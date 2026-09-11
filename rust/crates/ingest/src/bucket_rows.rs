//! The row shapes `raw_bucket_query`'s two result sets decode into.
//!
//! Split out of `bucket.rs` so that file stays about the query and the fold. These are the wire
//! contract: change one and the SQL that fills it changes with it.

use serde_json::Value;

/// One `GROUP BY b` row of the numeric-aggregate query (Query N). Non-numeric payloads never reach
/// it (`type::is_number` predicate), so `num_count` is the numeric count and `avg = sum/num_count`.
#[derive(serde::Serialize, serde::Deserialize)]
pub(crate) struct NumRow {
    /// SIGNED: a phase-shifted grid puts `ts < phase` in bucket `-1`. Unreachable for real data (the
    /// whole of that bucket predates 2 January 1970) but `u64` would fail the DECODE rather than
    /// clamp, taking the read down instead of returning a short first bucket.
    pub(crate) b: i64,
    pub(crate) num_count: u64,
    /// Lenient like [`crate::rollup::RollupRow`]'s twins, and for a sharper reason: these are the
    /// values SurrealDB's own `GROUP BY` aggregate produced. `math::sum` over integer-valued samples
    /// returns an INTEGER, so a series of whole-numbered meter readings fails this decode and takes
    /// the whole bucketed read — and therefore the entire retention GC fold — down with it.
    #[serde(default, deserialize_with = "de_opt_lenient_f64")]
    pub(crate) min: Option<f64>,
    #[serde(default, deserialize_with = "de_opt_lenient_f64")]
    pub(crate) max: Option<f64>,
    #[serde(default, deserialize_with = "de_opt_lenient_f64")]
    pub(crate) sum: Option<f64>,
}

/// Accept an integer OR a float for a persisted/aggregated `f64` — see [`crate::rollup`]'s twin.
fn de_opt_lenient_f64<'de, D>(d: D) -> Result<Option<f64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize;
    match Option::<Value>::deserialize(d)? {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v
            .as_f64()
            .map(Some)
            .ok_or_else(|| serde::de::Error::custom(format!("expected a number, found {v}"))),
    }
}

/// One `GROUP BY b` row of the count + ordered-last query (Query L). `count` is the TOTAL sample
/// count (numeric + non-numeric); `last`/`last_ts` are the chronologically last `(ts, seq)` payload.
#[derive(serde::Serialize, serde::Deserialize)]
pub(crate) struct CountRow {
    /// Signed for the same reason as [`NumRow::b`].
    pub(crate) b: i64,
    pub(crate) count: u64,
    /// `[ts_ms, seq, payload]` for the bucket's FIRST sample, out of
    /// `array::first(array::sort(array::group(…)))`. A triple, not three columns: the sort is over
    /// the whole triple, so splitting them could pair one row's `ts` with another's `payload`.
    #[serde(default)]
    pub(crate) first_triple: Vec<Value>,
    /// Same, for the LAST sample.
    #[serde(default)]
    pub(crate) last_triple: Vec<Value>,
}

impl CountRow {
    /// `(ts_ms, payload)` from a `[ts, seq, payload]` triple; an empty one reads `(0, Null)`.
    pub(crate) fn split(triple: &[Value]) -> (u64, Value) {
        let ts = triple.first().and_then(|v| v.as_u64()).unwrap_or(0);
        let payload = triple.get(2).cloned().unwrap_or(Value::Null);
        (ts, payload)
    }
}

// Delegate `SurrealValue` to serde rather than deriving it: the derive supports neither
// `#[serde(default)]` nor `deserialize_with`, and both rows need them.
lb_store::surreal_value_via_serde!(NumRow, CountRow);
