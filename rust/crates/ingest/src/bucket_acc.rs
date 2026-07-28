//! The running aggregate behind one bucket — the shape every read path folds into, whether the
//! samples arrive from a pushed-down `GROUP BY`, from the in-Rust fold oracle, or from a stored
//! rollup row (`bucket.rs`).
//!
//! It lives on its own because all three of those paths must agree on it EXACTLY. The pushdown is
//! only allowed to exist because it produces a `Vec<Bucket>` byte-identical to the fold's, and that
//! claim is only checkable if there is one accumulator, one `finish`, and one rollup re-aggregation
//! rather than three that look alike.
//!
//! Two properties are load-bearing and easy to lose:
//!   - `last` / `first` are the chronologically last / first payload by `(ts, seq)` — never
//!     scan-order luck;
//!   - min/max/avg re-aggregate EXACTLY from a stored rollup row, because the row carries `sum` and
//!     `count` rather than a mean.

use std::collections::BTreeMap;

use serde_json::Value;

use crate::bucket::Bucket;
use crate::rollup::RollupRow;

/// Running aggregate for one bucket.
#[derive(Debug, Clone, Default)]
pub(crate) struct Acc {
    pub(crate) min: Option<f64>,
    pub(crate) max: Option<f64>,
    pub(crate) sum: f64,
    pub(crate) num_count: u64,
    pub(crate) count: u64,
    pub(crate) last_key: (u64, u64), // (ts, seq) — "last" is exact, not scan-order luck
    pub(crate) last: Value,
    /// `(ts, seq)` of the chronologically first sample, and its payload. `None` until something
    /// contributes one.
    pub(crate) first_key: Option<(u64, u64)>,
    pub(crate) first: Value,
    /// Set when a contributing rollup row predates the `first` column — the bucket then cannot
    /// answer `first`/`nearest` at all, no matter what else merged into it.
    pub(crate) first_missing: bool,
}

impl Acc {
    pub(crate) fn fold_num(&mut self, v: f64) {
        self.min = Some(self.min.map_or(v, |m| m.min(v)));
        self.max = Some(self.max.map_or(v, |m| m.max(v)));
        self.sum += v;
        self.num_count += 1;
    }

    /// Offer a `first` candidate; the chronologically earliest `(ts, seq)` wins.
    pub(crate) fn fold_first(&mut self, key: (u64, u64), payload: &Value) {
        if self.first_key.is_none_or(|cur| key < cur) {
            self.first_key = Some(key);
            self.first = payload.clone();
        }
    }
}

/// Finalize the bucket map into the sparse, time-ordered wire shape (empty buckets already absent).
pub(crate) fn finish(accs: BTreeMap<u64, Acc>) -> Vec<Bucket> {
    accs.into_iter()
        .map(|(t, a)| Bucket {
            t,
            min: a.min,
            max: a.max,
            avg: (a.num_count > 0).then(|| a.sum / a.num_count as f64),
            last: a.last,
            count: a.count,
            first: a.first,
            value: None,
            sum: a.sum,
            num_count: a.num_count,
            last_ts: a.last_key.0,
            first_ts: a.first_key.map(|k| k.0).unwrap_or(t),
            has_first: a.first_key.is_some() && !a.first_missing,
        })
        .collect()
}

/// Re-aggregate one stored rollup row into a (wider or equal) requested bucket — exact for
/// min/max/avg because the row carries `sum` and `count`, not just the mean.
pub(crate) fn fold_rollup(acc: &mut Acc, r: &RollupRow) {
    acc.count += r.count;
    if let (Some(min), Some(max)) = (r.min, r.max) {
        acc.min = Some(acc.min.map_or(min, |m| m.min(min)));
        acc.max = Some(acc.max.map_or(max, |m| m.max(max)));
        acc.sum += r.sum;
        acc.num_count += r.num_count;
    }
    // Rollups only ever cover data OLDER than surviving raw, so a raw `last` (higher ts) wins.
    if (r.last_ts, 0) > acc.last_key {
        acc.last_key = (r.last_ts, 0);
        acc.last = r.last.clone();
    }
    // The `first` representative re-aggregates exactly: the first of a wider bucket is the earliest
    // `first` among the tier rows it absorbs. A row folded BEFORE the column existed poisons the
    // bucket's `has_first` — `first`/`nearest` then error rather than silently reporting a
    // later-but-present sample as the bucket's first (`method.rs`).
    match r.first_ts {
        Some(ts) => acc.fold_first((ts, 0), &r.first),
        None => acc.first_missing = true,
    }
}
