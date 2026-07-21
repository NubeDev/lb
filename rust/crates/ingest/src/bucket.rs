//! `series.read {mode:"buckets"}` — server-side time-bucket decimation (series-decimation scope,
//! slice C; series-read-perf scope). A window's raw samples decimate into ≤ budget buckets of
//! `{t, min, max, avg, last, count}` so spikes survive (`max`/`min` carry what a plain `avg` would
//! smooth away) and a month-long window ships ~1000 bucket records, never millions of rows.
//!
//! Execution is a **pushed-down `GROUP BY`** — the decimation the decimation scope always intended.
//! The raw window is aggregated **where the data lives** so a 10 k-sample window returns ≤ budget
//! bucket rows, never 10 k raw rows crossing the store boundary. It is two reads of one committed
//! snapshot ([`raw_bucket_query`]):
//!   - a numeric aggregate (`math::min/max/sum`, `count()` over `type::is::number(payload)`) — the
//!     `math::*` set skips non-numerics natively, so `avg = sum/num_count` is exact; and
//!   - a total-count + ordered-`last` read (`array::last` over an `ORDER BY ts, seq` subquery) — the
//!     ordered subquery makes `last` the chronologically last payload by `(ts, seq)`, non-numeric
//!     included, exactly what the old in-Rust fold guaranteed.
//!
//! The single-statement two-query split (both in one `query_ws` snapshot) is what buys both
//! properties the fold had — numeric-only aggregates AND a non-numeric-tolerant exact `last` — in one
//! pushed-down read (verified against SurrealDB 2.6.5 on the real `mem://` store, 2026-07-21).
//!
//! The fold that shipped first ([`read_buckets_fold`]) is retained **only as the parity test oracle**
//! — the pushdown must return a `Vec<Bucket>` byte-identical to it. Both then run the **same** rollup
//! merge ([`merge_rollups`]) for the post-GC tail (`rollup::read_rollups`), re-aggregating tier rows
//! exactly (min-of-mins, sum+count for avg). The scan itself rides the `(series, ts)` index window.

use std::collections::BTreeMap;

use lb_store::Store;

use crate::page::{read_page, Direction, PageError, PageQuery};
use crate::rollup::{read_rollups, RollupRow};
use crate::staging::SERIES_TABLE;
use serde_json::Value;

/// Hard ceiling on buckets per read — a width/window pair that would exceed it is rejected.
pub const MAX_BUCKETS: usize = 2_000;
/// Chunk size of the internal keyset scan (memory bound per fold step).
const SCAN_CHUNK: usize = 10_000;

/// One decimated bucket. `t` is the bucket's start (epoch ms, aligned to `width_ms`); min/max/avg
/// are over numeric payloads only; `last` is the raw payload of the chronologically last sample.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Bucket {
    pub t: u64,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub avg: Option<f64>,
    pub last: Value,
    pub count: u64,
    /// Exact-re-aggregation carriers (GC stores these on rollup rows); not part of the wire shape.
    #[serde(skip_serializing)]
    pub sum: f64,
    #[serde(skip_serializing)]
    pub num_count: u64,
    #[serde(skip_serializing)]
    pub last_ts: u64,
}

/// A bucketed-read request: a required half-open window `[from_ts, to_ts)` (epoch ms) and either an
/// explicit `width_ms` or a target point `budget` the width is derived from.
#[derive(Debug, Clone)]
pub struct BucketQuery {
    pub from_ts: u64,
    pub to_ts: u64,
    pub width_ms: Option<u64>,
    pub budget: Option<usize>,
}

/// Derive the effective bucket width: explicit width wins; else `span / budget` (ceil), clamped so
/// the bucket count never exceeds [`MAX_BUCKETS`]. Errors on an empty/inverted window.
pub fn effective_width(q: &BucketQuery) -> Result<u64, String> {
    if q.to_ts <= q.from_ts {
        return Err("empty window: to_ts must be > from_ts".into());
    }
    let span = q.to_ts - q.from_ts;
    let width = match (q.width_ms, q.budget) {
        (Some(w), _) if w > 0 => w,
        (_, Some(b)) if b > 0 => span.div_ceil(b.min(MAX_BUCKETS) as u64).max(1),
        _ => return Err("need width_ms or budget".into()),
    };
    if span.div_ceil(width) as usize > MAX_BUCKETS {
        return Err(format!("window/width yields > {MAX_BUCKETS} buckets"));
    }
    Ok(width)
}

/// Running aggregate for one bucket.
#[derive(Debug, Clone, Default)]
struct Acc {
    min: Option<f64>,
    max: Option<f64>,
    sum: f64,
    num_count: u64,
    count: u64,
    last_key: (u64, u64), // (ts, seq) — "last" is exact, not scan-order luck
    last: Value,
}

impl Acc {
    fn fold_num(&mut self, v: f64) {
        self.min = Some(self.min.map_or(v, |m| m.min(v)));
        self.max = Some(self.max.map_or(v, |m| m.max(v)));
        self.sum += v;
        self.num_count += 1;
    }
}

/// Decimate `series` in `ws` over the window into sparse, time-ordered buckets (empty buckets are
/// omitted). Raw samples aggregate via a pushed-down `GROUP BY`; rollup tiers fill buckets raw no
/// longer covers. This is the production path — O(buckets) out, not O(raw rows).
pub async fn read_buckets(
    store: &Store,
    ws: &str,
    series: &str,
    q: &BucketQuery,
    width_ms: u64,
) -> Result<Vec<Bucket>, PageError> {
    let mut accs = raw_bucket_query(store, ws, series, q, width_ms).await?;
    merge_rollups(store, ws, series, q, width_ms, &mut accs).await?;
    Ok(finish(accs))
}

/// The chronologically-ordered in-Rust fold that shipped first — kept **only as the parity oracle**
/// for [`read_buckets`]'s pushdown (the pushdown must be byte-identical to it). Same rollup merge,
/// same output shape; the only difference is that this pages every raw row into the host.
pub async fn read_buckets_fold(
    store: &Store,
    ws: &str,
    series: &str,
    q: &BucketQuery,
    width_ms: u64,
) -> Result<Vec<Bucket>, PageError> {
    let floor = |ts: u64| ts / width_ms * width_ms;
    let mut accs: BTreeMap<u64, Acc> = BTreeMap::new();

    // Chunked keyset scan of the raw window — O(SCAN_CHUNK) memory regardless of window size.
    let mut cursor: Option<String> = None;
    loop {
        let page = read_page(
            store,
            ws,
            series,
            &PageQuery {
                from_ts: Some(q.from_ts),
                to_ts: Some(q.to_ts),
                limit: Some(SCAN_CHUNK),
                cursor: cursor.clone(),
                direction: Direction::Fwd,
                ..Default::default()
            },
        )
        .await?;
        for s in &page.rows {
            let acc = accs.entry(floor(s.ts)).or_default();
            acc.count += 1;
            if let Some(v) = s.payload.as_f64() {
                acc.fold_num(v);
            }
            if (s.ts, s.seq) >= acc.last_key || acc.count == 1 {
                acc.last_key = (s.ts, s.seq);
                acc.last = s.payload.clone();
            }
        }
        match page.next_cursor {
            Some(c) => cursor = Some(c),
            None => break,
        }
    }

    merge_rollups(store, ws, series, q, width_ms, &mut accs).await?;
    Ok(finish(accs))
}

/// Push the raw-window decimation into SurrealDB: two `GROUP BY` reads of one committed snapshot,
/// both O(buckets) out. Keys buckets on the **absolute** floor `floor(ts/width)` — exactly the fold's
/// key — so `t = b*width` lands on the same absolute width grid regardless of whether `from` is
/// width-aligned. (Keying on `floor((ts-from)/width)` would group by offset-from-`from` and split an
/// absolute bucket across two `from`-relative ones whenever `from` is unaligned — the seam the
/// `pushdown_handles_an_unaligned_from` test guards.)
async fn raw_bucket_query(
    store: &Store,
    ws: &str,
    series: &str,
    q: &BucketQuery,
    width_ms: u64,
) -> Result<BTreeMap<u64, Acc>, PageError> {
    // One statement, two result sets → one snapshot (a concurrent commit can't split N from L).
    // Query N: numeric aggregates only (predicate makes `num_count` the numeric count → exact avg).
    // Query L: total count + ordered `last` (subquery ORDER BY makes `array::last` chronological).
    let sql = format!(
        "SELECT math::floor(time::millis(ts)/$width) AS b, count() AS num_count, \
           math::min(payload) AS min, math::max(payload) AS max, math::sum(payload) AS sum \
         FROM {SERIES_TABLE} \
         WHERE series = $s AND type::is::number(payload) \
           AND ts >= time::from::millis($from) AND ts < time::from::millis($to) GROUP BY b; \
         SELECT b, count() AS count, array::last(p) AS last, array::last(t) AS last_ts \
         FROM (SELECT math::floor(time::millis(ts)/$width) AS b, payload AS p, \
                 time::millis(ts) AS t, seq FROM {SERIES_TABLE} \
               WHERE series = $s \
                 AND ts >= time::from::millis($from) AND ts < time::from::millis($to) \
               ORDER BY t ASC, seq ASC) GROUP BY b"
    );
    let mut resp = store
        .query_ws(
            ws,
            &sql,
            vec![
                ("s".into(), Value::String(series.to_string())),
                ("from".into(), q.from_ts.into()),
                ("to".into(), q.to_ts.into()),
                ("width".into(), width_ms.into()),
            ],
        )
        .await
        .map_err(PageError::Store)?;
    let num: Vec<NumRow> = resp
        .take(0)
        .map_err(|e| PageError::Store(lb_store::StoreError::Decode(e.to_string())))?;
    let cnt: Vec<CountRow> = resp
        .take(1)
        .map_err(|e| PageError::Store(lb_store::StoreError::Decode(e.to_string())))?;

    // Join the two result sets by bucket index — O(buckets), never O(rows). `b` is the absolute
    // width-multiple index, so `t = b*width` is the fold's `floor(ts)` bucket start exactly.
    let mut accs: BTreeMap<u64, Acc> = BTreeMap::new();
    for r in num {
        let acc = accs.entry(r.b * width_ms).or_default();
        acc.min = r.min;
        acc.max = r.max;
        acc.sum = r.sum.unwrap_or(0.0);
        acc.num_count = r.num_count;
    }
    for r in cnt {
        let acc = accs.entry(r.b * width_ms).or_default();
        acc.count = r.count;
        acc.last = r.last;
        acc.last_key = (r.last_ts, 0); // ts only; the ordered subquery already broke the seq tie
    }
    Ok(accs)
}

/// One `GROUP BY b` row of the numeric-aggregate query (Query N). Non-numeric payloads never reach
/// it (`type::is::number` predicate), so `num_count` is the numeric count and `avg = sum/num_count`.
#[derive(serde::Deserialize)]
struct NumRow {
    b: u64,
    num_count: u64,
    min: Option<f64>,
    max: Option<f64>,
    sum: Option<f64>,
}

/// One `GROUP BY b` row of the count + ordered-last query (Query L). `count` is the TOTAL sample
/// count (numeric + non-numeric); `last`/`last_ts` are the chronologically last `(ts, seq)` payload.
#[derive(serde::Deserialize)]
struct CountRow {
    b: u64,
    count: u64,
    #[serde(default)]
    last: Value,
    last_ts: u64,
}

/// Merge the finest stored rollup tier into buckets raw didn't cover (post-GC history). Shared by
/// both the pushdown and the fold oracle so the tail is aggregated identically.
async fn merge_rollups(
    store: &Store,
    ws: &str,
    series: &str,
    q: &BucketQuery,
    width_ms: u64,
    accs: &mut BTreeMap<u64, Acc>,
) -> Result<(), PageError> {
    let floor = |ts: u64| ts / width_ms * width_ms;
    let tiers = read_rollups(store, ws, series, q.from_ts, q.to_ts).await?;
    if let Some(finest) = tiers.iter().map(|r| r.width_ms).min() {
        for r in tiers.iter().filter(|r| r.width_ms == finest) {
            fold_rollup(accs.entry(floor(r.t)).or_default(), r);
        }
    }
    Ok(())
}

/// Finalize the bucket map into the sparse, time-ordered wire shape (empty buckets already absent).
fn finish(accs: BTreeMap<u64, Acc>) -> Vec<Bucket> {
    accs.into_iter()
        .map(|(t, a)| Bucket {
            t,
            min: a.min,
            max: a.max,
            avg: (a.num_count > 0).then(|| a.sum / a.num_count as f64),
            last: a.last,
            count: a.count,
            sum: a.sum,
            num_count: a.num_count,
            last_ts: a.last_key.0,
        })
        .collect()
}

/// Re-aggregate one stored rollup row into a (wider or equal) requested bucket — exact for
/// min/max/avg because the row carries `sum` and `count`, not just the mean.
fn fold_rollup(acc: &mut Acc, r: &RollupRow) {
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
}
