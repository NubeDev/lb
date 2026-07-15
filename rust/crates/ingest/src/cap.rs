//! The per-series FIFO sample cap — the COUNT bound on the committed plane (series-sample-cap
//! scope, issue #65). "Keep at most N samples for this series; when N is exceeded, evict the oldest
//! first." The committed-plane twin of [`drop_oldest`](crate::overflow) at the staging bound.
//!
//! **Why a count bound exists at all.** Retention's time horizon (`raw_for_ms`) answers "how old is
//! too old", which does not bound bytes — **rate** does, and rate is the producer's choice, not the
//! operator's. A "keep 24h" policy is 864k samples at 10Hz and 86.4M at 1000Hz. `max_samples` is a
//! number an operator can multiply out in advance (~700 bytes/sample, measured).
//!
//! **The load-bearing detail: eviction orders by `ts`, NEVER `seq`.** `seq` is monotonic per
//! `(series, producer)` ONLY — comparing it across producers compares unrelated scales. This exact
//! mistake pinned `series.latest` to a pre-restart sample for hours in production (issue #63,
//! `debugging/ingest/latest-pinned-to-pre-restart-sample.md`). A `seq`-ordered cap on a
//! multi-producer series would **evict live rows and keep dead ones**. `ts` is the axis the streams
//! share, and it is the axis `series.latest` already uses.
//!
//! Eviction is batched: a series far over its cap converges over several passes rather than
//! deleting millions of rows in one transaction that stalls the store's global session mutex.

use lb_store::{Store, StoreError};
use serde_json::Value;

use crate::staging::SERIES_TABLE;

/// The recommended per-series sample cap (~70MB at the measured ~700 bytes/sample).
///
/// **Advisory in this release, not enforced.** A series past this bound is *warned* about
/// ([`over_cap_warning`]); nothing is evicted unless a policy sets `max_samples` explicitly. The
/// flip to enforcing this by default is release 2 — 100k is tight enough (~1.2 days at 1 sample/sec)
/// that a silent flip would evict real history on the next boot of a node whose operator never read
/// the release note. The window exists so policies can be set against a bound already visible in the
/// logs.
///
/// Why 100k and not 1M: the bound that matters is `max_samples × DEFAULT_SERIES_CAP` (10k series) —
/// the worst case an unattended workspace can actually reach. At 1M that is 7TB, a default bigger
/// than any disc we target (decorative, not safe); at 100k it is 0.7TB, and a realistic 200-series
/// node is 14GB, which fits a 64GB Pi. It is also ten full `MAX_PAGE_LIMIT` pages of raw history —
/// past that a caller is reading rollups regardless.
pub const DEFAULT_MAX_SAMPLES: u64 = 100_000;

/// Rows deleted per cap pass per series. Bounds one tick's work so a series 10M over its cap cannot
/// stall the store in a single transaction; the cap converges over subsequent passes.
pub const CAP_EVICT_BATCH: usize = 5_000;

/// Count of committed raw samples for `series` in `ws`.
pub async fn sample_count(store: &Store, ws: &str, series: &str) -> Result<u64, StoreError> {
    let mut resp = store
        .query_ws(
            ws,
            &format!("SELECT count() FROM {SERIES_TABLE} WHERE series = $series GROUP ALL"),
            vec![("series".into(), Value::String(series.to_string()))],
        )
        .await?;
    let n: Option<i64> = resp
        .take("count")
        .map_err(|e| StoreError::Decode(e.to_string()))?;
    Ok(n.unwrap_or(0).max(0) as u64)
}

/// The `ts` cutoff that keeps exactly the NEWEST `keep` samples of `series`: the `ts` of the oldest
/// row we intend to keep. Rows strictly older than it are the eviction set.
///
/// Ordered by `(ts DESC, seq DESC)` — the same axis `series.latest` uses. `seq` is only ever a
/// TIEBREAK within an identical `ts`, never the ordering axis itself (see the module docs).
///
/// `None` when the series has fewer than `keep` rows (nothing to evict).
pub(crate) async fn keep_cutoff_ts(
    store: &Store,
    ws: &str,
    series: &str,
    keep: u64,
) -> Result<Option<u64>, StoreError> {
    // Walk DESC to the boundary row: START at `keep` is the first row that must NOT be kept, so the
    // oldest KEPT row sits at offset keep-1. Taking its `ts` as an exclusive cutoff keeps ties on
    // that timestamp rather than splitting them — a cap may retain slightly more than `keep` for one
    // pass rather than evict a row that shares its ts with a keeper (never evict a row we can't
    // prove is older).
    // The order keys MUST appear in the projection — `ORDER BY` only sees selected idioms
    // (debugging/store/order-by-needs-selected-idiom.md; the same idiom the drain and `drop_oldest`
    // use). Projecting `time::millis(ts)` alone and ordering by bare `ts` is a PARSE error, not a
    // silent mis-sort — the one mercy here.
    let mut resp = store
        .query_ws(
            ws,
            &format!(
                "SELECT ts, seq, time::millis(ts) AS ts_ms FROM {SERIES_TABLE} \
                 WHERE series = $series ORDER BY ts DESC, seq DESC LIMIT 1 START $skip"
            ),
            vec![
                ("series".into(), Value::String(series.to_string())),
                (
                    "skip".into(),
                    Value::Number((keep.saturating_sub(1)).into()),
                ),
            ],
        )
        .await?;
    let rows: Vec<i64> = resp
        .take("ts_ms")
        .map_err(|e| StoreError::Decode(e.to_string()))?;
    Ok(rows.first().map(|v| (*v).max(0) as u64))
}

/// The `ts` (epoch ms) below which `series`' rows are over its `max_samples` bound — i.e. exactly
/// the window the cap is about to evict. The GC folds this window into the policy's rollup tiers
/// before evicting it, so coarse history survives a cap eviction.
///
/// `None` when the series is at or under the bound (nothing to roll up).
pub(crate) async fn cap_cutoff_ms(
    store: &Store,
    ws: &str,
    series: &str,
    max_samples: u64,
) -> Result<Option<u64>, StoreError> {
    if max_samples == 0 {
        return Ok(None);
    }
    keep_cutoff_ts(store, ws, series, max_samples).await
}

/// Evict the oldest samples of `series` until at most `max_samples` remain. Returns how many rows
/// were deleted. `max_samples == 0` is unbounded (no-op).
///
/// Deletes in batches of [`CAP_EVICT_BATCH`]; one call converges fully (it loops until at or under
/// the bound), but each transaction stays bounded.
pub async fn cap_series(
    store: &Store,
    ws: &str,
    series: &str,
    max_samples: u64,
) -> Result<usize, StoreError> {
    if max_samples == 0 {
        return Ok(0); // explicitly unbounded — the opt-out
    }
    let mut evicted = 0usize;
    loop {
        let count = sample_count(store, ws, series).await?;
        if count <= max_samples {
            return Ok(evicted);
        }
        let Some(cutoff) = keep_cutoff_ts(store, ws, series, max_samples).await? else {
            return Ok(evicted); // fewer rows than the bound — nothing to evict
        };
        let n = evict_older_than(store, ws, series, cutoff, CAP_EVICT_BATCH).await?;
        if n == 0 {
            // Every row over the bound shares the cutoff `ts` (a tie we refuse to split, above).
            // Bail rather than spin: the series sits marginally over its cap until a later sample
            // breaks the tie. Bounded overshoot beats an infinite loop or an arbitrary eviction.
            return Ok(evicted);
        }
        evicted += n;
    }
}

/// Delete up to `limit` samples of `series` with `ts` strictly older than `cutoff_ms`. Returns the
/// number deleted.
///
/// `DELETE … ORDER BY … LIMIT` is not supported by the engine, so we DELETE the rows returned by a
/// subquery that picks them — with the order keys in the projection (the idiom the drain and
/// `drop_oldest` use, `debugging/store/order-by-needs-selected-idiom.md`).
async fn evict_older_than(
    store: &Store,
    ws: &str,
    series: &str,
    cutoff_ms: u64,
    limit: usize,
) -> Result<usize, StoreError> {
    let mut resp = store
        .query_ws(
            ws,
            &format!(
                "LET $doomed = (SELECT id, ts AS _ts, seq AS _seq FROM {SERIES_TABLE} \
                 WHERE series = $series AND ts < time::from::millis($cutoff) \
                 ORDER BY _ts ASC, _seq ASC LIMIT $limit);
                 DELETE $doomed;
                 RETURN count($doomed);"
            ),
            vec![
                ("series".into(), Value::String(series.to_string())),
                ("cutoff".into(), Value::Number(cutoff_ms.into())),
                ("limit".into(), Value::Number(limit.into())),
            ],
        )
        .await?;
    // Statement 2 is the RETURN (LET binds without producing a result set).
    let n: Option<i64> = resp
        .take(2)
        .map_err(|e| StoreError::Decode(e.to_string()))?;
    Ok(n.unwrap_or(0).max(0) as usize)
}

/// The advisory warning for an unbounded series past [`DEFAULT_MAX_SAMPLES`] — release 1's job is to
/// make the need for a policy VISIBLE before release 2's default starts evicting. `None` when the
/// series is bounded by a policy or is under the recommended cap.
pub fn over_cap_warning(series: &str, count: u64, max_samples: u64) -> Option<String> {
    (max_samples == 0 && count > DEFAULT_MAX_SAMPLES).then(|| {
        format!(
            "series '{series}' holds {count} raw samples and is UNBOUNDED (no max_samples policy); \
             past the recommended cap of {DEFAULT_MAX_SAMPLES} (~{}MB at ~700 bytes/sample). Set a \
             retention policy: series.retention.set {{prefix, max_samples}}. A future release will \
             apply {DEFAULT_MAX_SAMPLES} by default; max_samples:0 opts out explicitly.",
            count * 700 / 1_000_000,
        )
    })
}
