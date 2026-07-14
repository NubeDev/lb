//! `series.read {mode:"buckets"}` — server-side time-bucket decimation (series-decimation scope,
//! slice C). A window's raw samples fold into ≤ budget buckets of `{t, min, max, avg, last, count}`
//! so spikes survive (`max`/`min` carry what a plain `avg` would smooth away) and a month-long
//! window ships ~1000 bucket records, never millions of rows.
//!
//! Execution is a **chunked fold over the keyset pager** (O(page) memory), not a SurrealDB
//! `GROUP BY`: the engine's aggregate set has no ordered `last`, and folding in the host keeps
//! `last` exact (max `(ts, seq)` in the bucket) and tolerates non-numeric payloads (they count and
//! carry `last`, but skip min/max/avg). The scan itself rides the `(series, ts)` index window.
//! Where retention GC has evicted raw samples, the fold merges the stored rollup tier for the
//! missing part of the window (`rollup::read_rollups`), re-aggregating tier rows exactly
//! (min-of-mins, sum+count for avg).

use std::collections::BTreeMap;

use lb_store::Store;

use crate::page::{read_page, Direction, PageError, PageQuery};
use crate::rollup::{read_rollups, RollupRow};
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
/// omitted). Raw samples fold first; rollup tiers fill buckets raw no longer covers.
pub async fn read_buckets(
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

    // Merge the finest stored rollup tier into buckets raw didn't cover (post-GC history).
    let tiers = read_rollups(store, ws, series, q.from_ts, q.to_ts).await?;
    if let Some(finest) = tiers.iter().map(|r| r.width_ms).min() {
        for r in tiers.iter().filter(|r| r.width_ms == finest) {
            fold_rollup(accs.entry(floor(r.t)).or_default(), r);
        }
    }

    Ok(accs
        .into_iter()
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
        .collect())
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
