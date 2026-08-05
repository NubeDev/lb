//! The per-tier rollup **method** — which single value a downsampled tier reads as
//! (series-normalize scope). A tier with `method: "avg"` makes `series.read {mode:"buckets"}` return
//! one plain value per bucket boundary (13:00, 13:15, 13:30…) instead of a stat row the caller has
//! to pick from; a state series sets `method: "last"` on its own longer prefix and reads as a step
//! chart. The full stat row stays on the wire either way — the method adds a `value` column, it
//! never removes anything.
//!
//! **The closed set is bounded by exactness.** Every method here is either re-aggregable from the
//! stored per-bucket stats (`avg` from `sum/num_count` — never a mean-of-means — plus `min`, `max`,
//! `sum`, `count`) or a kept representative sample (`last`, `first`, `nearest`). Percentiles and
//! stddev are deliberately absent: they are NOT exactly re-aggregable from bucket stats, and
//! admitting them would make rollups approximate — a storage policy quietly becoming a compute
//! plane. That is the line this crate does not cross.
//!
//! **A method the tier didn't store is a `BadInput`, never an approximation.** Buckets folded before
//! a method was set carry no representative for it; [`apply_method`] says so in words an operator can
//! act on rather than substituting a plausible neighbour.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::bucket::Bucket;
#[cfg(test)]
use crate::bucket::Source;

/// The value a rollup tier reads as. Serialized lowercase on the wire (`"avg"`, `"nearest"`, …).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Method {
    /// `sum / num_count` over numeric payloads — exact under re-aggregation.
    Avg,
    Min,
    Max,
    Sum,
    /// Total samples in the bucket (numeric or not).
    Count,
    /// The chronologically LAST sample by `(ts, seq-within-producer)`.
    Last,
    /// The chronologically FIRST sample by `(ts, seq-within-producer)`.
    First,
    /// The sample closest in absolute time to the bucket's boundary `t` — see [`apply_method`].
    Nearest,
}

impl Method {
    /// Parse a wire method name. The error names the closed set, so a typo is self-correcting.
    pub fn parse(s: &str) -> Result<Self, String> {
        serde_json::from_value(Value::String(s.to_string())).map_err(|_| {
            format!(
                "unknown method: {s} (expected one of avg, min, max, sum, count, last, first, nearest)"
            )
        })
    }

    /// The wire name.
    pub fn as_str(self) -> &'static str {
        match self {
            Method::Avg => "avg",
            Method::Min => "min",
            Method::Max => "max",
            Method::Sum => "sum",
            Method::Count => "count",
            Method::Last => "last",
            Method::First => "first",
            Method::Nearest => "nearest",
        }
    }

    /// Does this method need a kept representative sample (rather than a re-aggregable statistic)?
    fn needs_representative(self) -> bool {
        matches!(self, Method::First | Method::Nearest)
    }
}

/// Set each bucket's `value` column to `method`'s value, in place.
///
/// `buckets` must be ascending by `t` (the shape [`crate::read_buckets`] returns) — `nearest` reads
/// the previous bucket.
///
/// **`nearest` is snap-to-grid, and that is why it is not `first`.** The grid point is the bucket's
/// boundary `t`; the sample nearest it in ABSOLUTE time may sit just *before* the boundary, in the
/// previous bucket. So the candidates are this bucket's `first` (the closest from above) and the
/// previous bucket's `last` (the closest from below), and the nearer of the two wins. Restricting
/// the search to within the bucket would make `nearest` a synonym for `first` — which is exactly why
/// no separate `nearest`/`nearest_ts` column is stored: the two representatives already on the row
/// (`first` and `last`) determine it exactly, and a third column would be a byte-for-byte duplicate
/// of `first` in a slice whose entire purpose is storing less.
///
/// Ties go to `first` (the sample at or after the boundary) — a stable, documented rule beats a
/// float comparison deciding it.
pub fn apply_method(buckets: &mut [Bucket], method: Method) -> Result<(), String> {
    // The previous bucket's `(last_ts, last)` — the below-the-boundary candidate for `nearest`.
    let mut prev: Option<(u64, Value)> = None;
    for b in buckets.iter_mut() {
        if method.needs_representative() && !b.has_first {
            return Err(missing(b.t, method));
        }
        let value = match method {
            Method::Avg => num(b.avg),
            Method::Min => num(b.min),
            Method::Max => num(b.max),
            Method::Sum => num((b.num_count > 0).then_some(b.sum)),
            Method::Count => json!(b.count),
            Method::Last => b.last.clone(),
            Method::First => b.first.clone(),
            Method::Nearest => match &prev {
                // Strictly closer from below wins; a tie keeps `first`.
                Some((prev_ts, prev_last))
                    if b.t.saturating_sub(*prev_ts) < b.first_ts.saturating_sub(b.t) =>
                {
                    prev_last.clone()
                }
                _ => b.first.clone(),
            },
        };
        prev = Some((b.last_ts, b.last.clone()));
        b.value = Some(value);
    }
    Ok(())
}

/// A `f64` statistic as a JSON value — `null` when the bucket held no numeric payload at all, which
/// is honest: `avg` of an event series is not zero.
fn num(v: Option<f64>) -> Value {
    v.map(|x| json!(x)).unwrap_or(Value::Null)
}

/// The error for a method whose representative the tier never stored.
fn missing(t: u64, method: Method) -> String {
    format!(
        "bucket at t={t} was folded without a `{}` representative; set the method on the tier and \
         let new buckets accumulate (existing rollup rows are not re-derivable — the raw samples \
         they replaced are gone)",
        method.as_str()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A bucket with everything a fold would have populated.
    fn bucket(t: u64, first: f64, first_ts: u64, last: f64, last_ts: u64) -> Bucket {
        Bucket {
            t,
            min: Some(first.min(last)),
            max: Some(first.max(last)),
            avg: Some((first + last) / 2.0),
            last: json!(last),
            count: 2,
            sum: first + last,
            num_count: 2,
            last_ts,
            first: json!(first),
            first_ts,
            has_first: true,
            value: None,
            // These fixtures stand in for a plain raw fold; method selection is independent of
            // which table the bucket came from.
            source: Source::Raw,
            raw_count: 2,
            rollup_count: 0,
        }
    }

    #[test]
    fn parse_accepts_the_closed_set_and_names_it_on_a_typo() {
        for name in [
            "avg", "min", "max", "sum", "count", "last", "first", "nearest",
        ] {
            assert_eq!(Method::parse(name).unwrap().as_str(), name);
        }
        let err = Method::parse("p95").unwrap_err();
        assert!(err.contains("p95"), "{err}");
        assert!(
            err.contains("nearest"),
            "the error names the closed set: {err}"
        );
    }

    #[test]
    fn each_statistic_method_selects_its_own_column() {
        let mut b = vec![bucket(0, 10.0, 1, 20.0, 9)];
        for (m, want) in [
            (Method::Avg, json!(15.0)),
            (Method::Min, json!(10.0)),
            (Method::Max, json!(20.0)),
            (Method::Sum, json!(30.0)),
            (Method::Count, json!(2)),
            (Method::Last, json!(20.0)),
            (Method::First, json!(10.0)),
        ] {
            apply_method(&mut b, m).unwrap();
            assert_eq!(b[0].value.as_ref().unwrap(), &want, "method {}", m.as_str());
        }
    }

    #[test]
    fn nearest_reaches_back_across_the_boundary_when_that_sample_is_closer() {
        // width 100. Bucket 100's own first is at 190 (90ms after the boundary); bucket 0's last is
        // at 95 (5ms BEFORE it). Snap-to-grid must pick the 95 sample — `first` would pick 190.
        let mut b = vec![bucket(0, 1.0, 5, 2.0, 95), bucket(100, 3.0, 190, 4.0, 199)];
        apply_method(&mut b, Method::Nearest).unwrap();
        assert_eq!(
            b[0].value.as_ref().unwrap(),
            &json!(1.0),
            "no previous bucket → own first"
        );
        assert_eq!(
            b[1].value.as_ref().unwrap(),
            &json!(2.0),
            "the previous bucket's last is nearer"
        );

        // And it is NOT just an alias for first.
        apply_method(&mut b, Method::First).unwrap();
        assert_eq!(b[1].value.as_ref().unwrap(), &json!(3.0));
    }

    #[test]
    fn nearest_keeps_its_own_first_when_that_one_is_closer_or_tied() {
        // Own first at 102 (2ms after) beats the previous last at 95 (5ms before).
        let mut b = vec![bucket(0, 1.0, 5, 2.0, 95), bucket(100, 3.0, 102, 4.0, 150)];
        apply_method(&mut b, Method::Nearest).unwrap();
        assert_eq!(b[1].value.as_ref().unwrap(), &json!(3.0));

        // Exact tie (5ms either side) → documented rule: `first` wins.
        let mut b = vec![bucket(0, 1.0, 5, 2.0, 95), bucket(100, 3.0, 105, 4.0, 150)];
        apply_method(&mut b, Method::Nearest).unwrap();
        assert_eq!(b[1].value.as_ref().unwrap(), &json!(3.0));
    }

    #[test]
    fn a_method_the_tier_did_not_store_is_a_clear_error_never_an_approximation() {
        let mut b = vec![bucket(0, 1.0, 5, 2.0, 95)];
        b[0].has_first = false; // folded before `first` was stored
        for m in [Method::First, Method::Nearest] {
            let err = apply_method(&mut b, m).unwrap_err();
            assert!(err.contains(m.as_str()), "{err}");
            assert!(
                err.contains("set the method on the tier"),
                "actionable: {err}"
            );
        }
        // The re-aggregable methods still work off the stats that ARE stored.
        apply_method(&mut b, Method::Avg).unwrap();
        assert_eq!(b[0].value.as_ref().unwrap(), &json!(1.5));
    }

    #[test]
    fn a_bucket_with_no_numeric_payload_reads_null_not_zero() {
        let mut b = vec![bucket(0, 1.0, 5, 2.0, 95)];
        b[0].num_count = 0;
        b[0].sum = 0.0;
        b[0].avg = None;
        b[0].min = None;
        b[0].max = None;
        for m in [Method::Avg, Method::Min, Method::Max, Method::Sum] {
            apply_method(&mut b, m).unwrap();
            assert_eq!(
                b[0].value.as_ref().unwrap(),
                &Value::Null,
                "method {}",
                m.as_str()
            );
        }
        // `count` is a sample count, not a numeric one — it still reports.
        apply_method(&mut b, Method::Count).unwrap();
        assert_eq!(b[0].value.as_ref().unwrap(), &json!(2));
    }
}
