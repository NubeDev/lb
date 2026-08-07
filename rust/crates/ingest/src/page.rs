//! `series.read` keyset paging — the unbounded `Vec<Sample>` becomes a bounded page
//! (series-paging scope, slice B). The page seeks the `(series, ts)` index on the unique composite
//! sort key `(ts, seq, producer)` — `ts` leads because `seq` restarts at 0 on every producer
//! generation (see `cursor.rs`); `seq`/`producer` are the tiebreakers that keep the key unique. So
//! page-500 costs what page-1 costs: O(page), never an OFFSET scan.
//!
//! The query also accepts wall-clock `from_ts`/`to_ts` bounds (epoch ms, half-open `[from, to)`) —
//! `ts` is a real `datetime` since the schema slice — alongside the legacy `from_seq`/`to_seq`.
//! Bounds compose with the cursor; the cursor always wins the seek.

use lb_store::{Store, StoreError};
use serde_json::Value;

use crate::cursor::Cursor;
use crate::sample::Sample;
use crate::staging::SERIES_TABLE;

/// Default page size when the caller sends no `limit` — bounded by design; "no limit" is impossible.
pub const DEFAULT_PAGE_LIMIT: usize = 10_000;
/// Hard ceiling a caller-supplied `limit` is clamped to (never honored above it).
pub const MAX_PAGE_LIMIT: usize = 10_000;

/// Page direction over the `(ts, seq, producer)` sort key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Direction {
    /// Ascending commit order (oldest first) — the default.
    #[default]
    Fwd,
    /// Descending (newest first) — the dashboard backfill direction.
    Back,
}

/// One `series.read` page request. All bounds optional; `limit` is clamped to [`MAX_PAGE_LIMIT`].
#[derive(Debug, Clone, Default)]
pub struct PageQuery {
    pub from_seq: Option<u64>,
    pub to_seq: Option<u64>,
    /// Wall-clock window, epoch milliseconds, half-open `[from_ts, to_ts)`.
    pub from_ts: Option<u64>,
    pub to_ts: Option<u64>,
    pub limit: Option<usize>,
    /// Opaque cursor from a previous page (`Cursor` wire form). Malformed → clean error.
    pub cursor: Option<String>,
    pub direction: Direction,
}

/// One page of committed samples plus the chain bookmarks. `next_cursor == None` means
/// end-of-range (the page came back short of `limit`).
#[derive(Debug, Clone)]
pub struct Page {
    pub rows: Vec<Sample>,
    pub next_cursor: Option<String>,
    pub prev_cursor: Option<String>,
}

/// A paging failure: a malformed cursor (reject cleanly, never mis-seek) or a store error.
#[derive(Debug, thiserror::Error)]
pub enum PageError {
    #[error("bad cursor: {0}")]
    BadCursor(String),
    #[error(transparent)]
    Store(#[from] StoreError),
}

/// Read one keyset page of `series` in `ws`. Rows are ordered by `(ts, seq, producer)` in
/// `direction`; the returned `next_cursor` resumes exactly after the last row (exactly-once, no
/// gaps/dupes). `ts` leads because `seq` restarts at 0 per producer generation — see `cursor.rs`.
pub async fn read_page(
    store: &Store,
    ws: &str,
    series: &str,
    q: &PageQuery,
) -> Result<Page, PageError> {
    let limit = q
        .limit
        .unwrap_or(DEFAULT_PAGE_LIMIT)
        .clamp(1, MAX_PAGE_LIMIT);
    let mut clauses = String::from("series = $series");
    let mut bindings: Vec<(String, Value)> =
        vec![("series".into(), Value::String(series.to_string()))];

    if let Some(from) = q.from_seq {
        clauses.push_str(" AND seq >= $from_seq");
        bindings.push(("from_seq".into(), Value::Number(from.into())));
    }
    if let Some(to) = q.to_seq {
        clauses.push_str(" AND seq <= $to_seq");
        bindings.push(("to_seq".into(), Value::Number(to.into())));
    }
    if let Some(from) = q.from_ts {
        clauses.push_str(" AND ts >= time::from::millis($from_ts)");
        bindings.push(("from_ts".into(), Value::Number(from.into())));
    }
    if let Some(to) = q.to_ts {
        clauses.push_str(" AND ts < time::from::millis($to_ts)");
        bindings.push(("to_ts".into(), Value::Number(to.into())));
    }
    if let Some(wire) = &q.cursor {
        let c = Cursor::decode(wire).map_err(PageError::BadCursor)?;
        // Seek strictly past the cursor position on the composite key — the tiebreaker discipline.
        // The comparison must use the SAME key the ORDER BY does, or the seek skips or repeats rows.
        //
        // A `v2` cursor carries `ts` and seeks the lexicographic `(ts, seq, producer)` key. A legacy
        // `v1` cursor has no `ts`, so it can only seek the old `(seq, producer)` key — correct for
        // resuming a chain issued before this change, and self-limiting because every cursor this
        // node issues from here on is `v2`.
        match (c.ts, q.direction) {
            (Some(_), Direction::Fwd) => clauses.push_str(
                " AND (ts > time::from::millis($cts) \
                   OR (ts = time::from::millis($cts) AND seq > $cseq) \
                   OR (ts = time::from::millis($cts) AND seq = $cseq AND producer > $cprod))",
            ),
            (Some(_), Direction::Back) => clauses.push_str(
                " AND (ts < time::from::millis($cts) \
                   OR (ts = time::from::millis($cts) AND seq < $cseq) \
                   OR (ts = time::from::millis($cts) AND seq = $cseq AND producer < $cprod))",
            ),
            (None, Direction::Fwd) => {
                clauses.push_str(" AND (seq > $cseq OR (seq = $cseq AND producer > $cprod))")
            }
            (None, Direction::Back) => {
                clauses.push_str(" AND (seq < $cseq OR (seq = $cseq AND producer < $cprod))")
            }
        }
        if let Some(cts) = c.ts {
            bindings.push(("cts".into(), Value::Number(cts.into())));
        }
        bindings.push(("cseq".into(), Value::Number(c.seq.into())));
        bindings.push(("cprod".into(), Value::String(c.producer)));
    }

    let order = match q.direction {
        Direction::Fwd => "ASC",
        Direction::Back => "DESC",
    };
    // `ts` round-trips as epoch ms (`time::millis`); order keys are in the projection (the engine's
    // ORDER-BY-needs-selected-idiom).
    //
    // ORDER BY `ts` FIRST — `seq` is per-producer-generation and RESTARTS AT 0 on an extension
    // restart, so it does not order by time (see `cursor.rs` for the full account and the live
    // evidence). `(seq, producer)` stay as tiebreakers to keep the sort key unique. `series_ts_idx`
    // on `(series, ts)` backs this, so the page stays O(page).
    let sql = format!(
        "SELECT series, producer, seq, time::millis(ts) AS ts, payload FROM {SERIES_TABLE} \
         WHERE {clauses} ORDER BY ts {order}, seq {order}, producer {order} LIMIT {limit}"
    );
    let mut resp = store.query_ws(ws, &sql, bindings).await?;
    let rows: Vec<Sample> = resp
        .take(0)
        .map_err(|e| StoreError::Decode(e.to_string()))?;

    let key = |s: &Sample| {
        Cursor {
            ts: Some(s.ts),
            seq: s.seq,
            producer: s.producer.clone(),
        }
        .encode()
    };
    let next_cursor = (rows.len() == limit).then(|| key(rows.last().expect("non-empty")));
    let prev_cursor = rows.first().map(key);
    Ok(Page {
        rows,
        next_cursor,
        prev_cursor,
    })
}
