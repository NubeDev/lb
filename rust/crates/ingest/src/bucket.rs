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

use crate::align::{bucket_start, phase_of, start_of_index, Align};
use crate::bucket_acc::{finish, fold_rollup, Acc};
use crate::page::{read_page, Direction, PageError, PageQuery};
use crate::rollup::read_rollups;
use crate::staging::SERIES_TABLE;
use serde_json::Value;

/// Hard ceiling on buckets per read — a width/window pair that would exceed it is rejected.
pub const MAX_BUCKETS: usize = 2_000;
/// Chunk size of the internal keyset scan (memory bound per fold step).
const SCAN_CHUNK: usize = 10_000;

/// Which table's rows a bucket was actually built from.
///
/// A merged read ([`merge_rollups`]) draws from two tables with one flat wire shape, so without
/// this the caller cannot tell a bucket folded from an evicted tier from one folded from live raw
/// — they are byte-identical. That ambiguity is the whole reason a viewer cannot tell "this window
/// is empty by retention policy" from "this read is broken".
///
/// `Mixed` is not a rounding of the other two: within ONE bucket, raw and rollup rows can both
/// contribute when the requested width straddles the eviction boundary. Reporting such a bucket as
/// either pure source would be a lie in one direction or the other.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    /// Built only from live rows in `series`.
    Raw,
    /// Built only from stored rows in `series_rollup`.
    Rollup,
    /// Both contributed — the bucket straddles the raw-eviction boundary.
    Mixed,
}

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
    /// The chronologically FIRST payload by `(ts, seq)` — the `first` method's value, and half of
    /// what `nearest` needs (series-normalize scope). On the wire because a caller reading the full
    /// stat row wants both ends of the bucket, not just the last.
    pub first: Value,
    /// The tier method's single value, when a method governs this read. Absent = today's exact
    /// behaviour: the full stat row and no `value` column (`method.rs`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<Value>,
    /// Which table(s) this bucket was built from. ON the wire — it is the field that makes a
    /// merged read honest about itself.
    pub source: Source,
    /// The `count` split by origin, so a `Mixed` bucket is not just labelled but quantified. Sums
    /// to `count` by construction.
    pub raw_count: u64,
    pub rollup_count: u64,
    /// Exact-re-aggregation carriers (GC stores these on rollup rows); not part of the wire shape.
    #[serde(skip_serializing)]
    pub sum: f64,
    #[serde(skip_serializing)]
    pub num_count: u64,
    #[serde(skip_serializing)]
    pub last_ts: u64,
    #[serde(skip_serializing)]
    pub first_ts: u64,
    /// Is `first` real? A rollup row folded before the column existed carries none, and `first` /
    /// `nearest` must then be a clear error rather than a plausible substitute (`method.rs`).
    #[serde(skip_serializing)]
    pub has_first: bool,
}

/// A bucketed-read request: a required half-open window `[from_ts, to_ts)` (epoch ms) and either an
/// explicit `width_ms` or a target point `budget` the width is derived from.
///
/// `Default` is derived so the NEXT additive field costs no call-site churn — every construction
/// site can spread `..Default::default()` (the argument `Policy` already makes). The default window
/// is empty, which [`effective_width`] rejects loudly; that is the right failure for a query nobody
/// filled in.
#[derive(Debug, Clone, Default)]
pub struct BucketQuery {
    pub from_ts: u64,
    pub to_ts: u64,
    pub width_ms: Option<u64>,
    pub budget: Option<usize>,
    /// Where the read's buckets START. `None` = the UTC epoch grid — this crate's behaviour since it
    /// shipped, unchanged.
    ///
    /// **This must be the SAME alignment the GC folded the tier on** or the read mixes two griddings
    /// and nothing errors ([`crate::align`]). The host read verb resolves it from the governing
    /// policy for exactly that reason; `run_gc` sets it from the tier it is folding.
    pub align: Option<Align>,
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
    let floor = |ts: u64| bucket_start(ts, width_ms, q.align);
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
            acc.raw_count += 1;
            if let Some(v) = s.payload.as_f64() {
                acc.fold_num(v);
            }
            if (s.ts, s.seq) >= acc.last_key || acc.count == 1 {
                acc.last_key = (s.ts, s.seq);
                acc.last = s.payload.clone();
            }
            acc.fold_first((s.ts, s.seq), &s.payload);
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
/// both O(buckets) out. Keys buckets on the **absolute** floor `floor((ts - phase)/width)` — exactly
/// the fold's key — so the reconstructed `t` lands on the same absolute grid regardless of whether
/// `from` is aligned to it. (Keying on `floor((ts-from)/width)` would group by offset-from-`from` and
/// split an absolute bucket across two `from`-relative ones whenever `from` is unaligned — the seam
/// the `pushdown_handles_an_unaligned_from` test guards.)
///
/// `phase` is the tier's alignment reduced modulo the width ([`crate::align`]); it is `0` for an
/// unaligned read, which makes this statement character-for-character the one that shipped, minus a
/// `- 0`. The reconstruction `t = index*width + phase` goes through [`start_of_index`] — the same
/// function the Rust floor uses — so the SQL grid and the fold grid cannot drift even in the join.
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
    let phase = phase_of(q.align, width_ms);
    let sql = format!(
        "SELECT math::floor((time::millis(ts) - $phase)/$width) AS b, count() AS num_count, \
           math::min(payload) AS min, math::max(payload) AS max, math::sum(payload) AS sum \
         FROM {SERIES_TABLE} \
         WHERE series = $s AND type::is::number(payload) \
           AND ts >= time::from::millis($from) AND ts < time::from::millis($to) GROUP BY b; \
         SELECT b, count() AS count, array::last(p) AS last, array::last(t) AS last_ts, \
           array::first(p) AS first, array::first(t) AS first_ts \
         FROM (SELECT math::floor((time::millis(ts) - $phase)/$width) AS b, payload AS p, \
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
                ("phase".into(), phase.into()),
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
    // index on the `(width, phase)` grid, so `start_of_index` is the fold's `bucket_start(ts)`
    // exactly — the SAME function, not a re-derivation of it.
    let mut accs: BTreeMap<u64, Acc> = BTreeMap::new();
    for r in num {
        let acc = accs
            .entry(start_of_index(r.b as i128, width_ms, phase))
            .or_default();
        acc.min = r.min;
        acc.max = r.max;
        acc.sum = r.sum.unwrap_or(0.0);
        acc.num_count = r.num_count;
    }
    for r in cnt {
        let acc = accs
            .entry(start_of_index(r.b as i128, width_ms, phase))
            .or_default();
        acc.count = r.count;
        // Assignment, not `+=`, mirrors `count` above: the pushdown populates each bucket once,
        // before `merge_rollups` runs, so every sample it reports is by definition raw.
        acc.raw_count = r.count;
        acc.last = r.last;
        acc.last_key = (r.last_ts, 0); // ts only; the ordered subquery already broke the seq tie
                                       // Same ordered subquery, opposite end: `array::first` over `ORDER BY t, seq` is the
                                       // chronologically first payload, exactly what the fold oracle's `(ts, seq)` minimum is.
        acc.fold_first((r.first_ts, 0), &r.first);
    }
    Ok(accs)
}

/// One `GROUP BY b` row of the numeric-aggregate query (Query N). Non-numeric payloads never reach
/// it (`type::is::number` predicate), so `num_count` is the numeric count and `avg = sum/num_count`.
#[derive(serde::Deserialize)]
struct NumRow {
    /// SIGNED: a phase-shifted grid puts `ts < phase` in bucket `-1`. Unreachable for real data (the
    /// whole of that bucket predates 2 January 1970) but `u64` would fail the DECODE rather than
    /// clamp, taking the read down instead of returning a short first bucket.
    b: i64,
    num_count: u64,
    /// Lenient like [`crate::rollup::RollupRow`]'s twins, and for a sharper reason: these are the
    /// values SurrealDB's own `GROUP BY` aggregate produced. `math::sum` over integer-valued samples
    /// returns an INTEGER, so a series of whole-numbered meter readings fails this decode and takes
    /// the whole bucketed read — and therefore the entire retention GC fold — down with it.
    #[serde(default, deserialize_with = "de_opt_lenient_f64")]
    min: Option<f64>,
    #[serde(default, deserialize_with = "de_opt_lenient_f64")]
    max: Option<f64>,
    #[serde(default, deserialize_with = "de_opt_lenient_f64")]
    sum: Option<f64>,
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
#[derive(serde::Deserialize)]
struct CountRow {
    /// Signed for the same reason as [`NumRow::b`].
    b: i64,
    count: u64,
    #[serde(default)]
    last: Value,
    last_ts: u64,
    #[serde(default)]
    first: Value,
    first_ts: u64,
}

/// Merge the finest stored rollup tier into buckets raw didn't cover (post-GC history). Shared by
/// both the pushdown and the fold oracle so the tail is aggregated identically.
///
/// **"Raw didn't cover" is enforced, not assumed.** A rollup row describes a COMPLETE bucket, so a
/// row whose range overlaps raw that is still on disc is REDUNDANT with that raw, not complementary
/// to it — folding both counts every sample twice. `rollup.rs` states the invariant that makes this
/// normally moot ("rollup rows exist only where retention has evicted the raw beneath them"), but
/// the GC can transiently break it: within one pass every tier folds BEFORE any raw is evicted, so
/// a policy with two tiers has the finer tier's rows on disc while the coarser tier is still reading
/// the raw underneath them. That doubled the coarse tier's `count`/`sum` on the first pass over any
/// series — self-healing on the next pass (which re-folds from the rollups alone) and wrong on every
/// read in between. Found by the idempotence assertion in `series_align_grid_test`; it predates
/// alignment, and per-tier fold cutoffs would have widened the window it is wrong in.
///
/// The guard is one comparison against the oldest raw instant this read already folded — `accs`
/// holds nothing but raw when this runs, so `first_key` is a raw timestamp by construction.
async fn merge_rollups(
    store: &Store,
    ws: &str,
    series: &str,
    q: &BucketQuery,
    width_ms: u64,
    accs: &mut BTreeMap<u64, Acc>,
) -> Result<(), PageError> {
    let floor = |ts: u64| bucket_start(ts, width_ms, q.align);
    let tiers = read_rollups(store, ws, series, q.from_ts, q.to_ts).await?;
    let oldest_raw = accs.values().filter_map(|a| a.first_key).map(|k| k.0).min();
    if let Some(finest) = tiers.iter().map(|r| r.width_ms).min() {
        for r in tiers.iter().filter(|r| r.width_ms == finest) {
            if oldest_raw.is_some_and(|raw| r.t.saturating_add(r.width_ms) > raw) {
                continue; // the raw beneath this row is still here — it is the same samples
            }
            fold_rollup(accs.entry(floor(r.t)).or_default(), r);
        }
    }
    Ok(())
}
