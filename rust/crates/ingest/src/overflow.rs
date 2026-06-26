//! The single overflow policy, bounded at BOTH ends (producer staging AND cloud staging) — the
//! robustness primitive that stops a burst from OOMing or filling a disk (ingest scope). One
//! policy per sample, chosen by its QoS:
//!   - **best-effort → drop-oldest** (lossy by design; the default for high-rate telemetry);
//!   - **must-deliver → dead-letter** (never silently dropped; diverted to a dead-letter table).
//!
//! Bounding is on the *count of staged rows per workspace* — a coarse but honest cap that prevents
//! unbounded growth. (Rate-limiting and the checkpointed-ring optimization are out of this slice.)

use lb_store::{Store, StoreError};
use serde_json::{json, Value};

use crate::sample::Sample;
use crate::staging::{DEAD_LETTER_TABLE, STAGING_TABLE};

/// What to do when staging is at its bound and another sample arrives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverflowPolicy {
    /// Evict the oldest staged row to admit the new one (best-effort; lossy by design).
    DropOldest,
    /// Divert the incoming sample to the dead-letter table (must-deliver; never silently dropped).
    DeadLetter,
}

/// Ensure staging in `ws` has room for one more sample under `bound`. Returns `true` if the caller
/// should proceed to append the sample to staging, `false` if the sample was diverted (dead-letter)
/// and must NOT also be appended. A `bound` of 0 means unbounded (no enforcement).
pub async fn enforce_bound(
    store: &Store,
    ws: &str,
    bound: usize,
    policy: OverflowPolicy,
    incoming: &Sample,
) -> Result<bool, StoreError> {
    if bound == 0 {
        return Ok(true);
    }
    let count = staged_count(store, ws).await?;
    if count < bound {
        return Ok(true);
    }
    match policy {
        OverflowPolicy::DropOldest => {
            drop_oldest(store, ws).await?;
            Ok(true)
        }
        OverflowPolicy::DeadLetter => {
            dead_letter(store, ws, incoming).await?;
            Ok(false)
        }
    }
}

/// Count of staged rows in `ws` (the workspace-partitioned bound — never another workspace's rows).
async fn staged_count(store: &Store, ws: &str) -> Result<usize, StoreError> {
    let mut resp = store
        .query_ws(
            ws,
            &format!("SELECT count() FROM {STAGING_TABLE} GROUP ALL"),
            vec![],
        )
        .await?;
    let n: Option<i64> = resp.take("count").map_err(|e| StoreError::Decode(e.to_string()))?;
    Ok(n.unwrap_or(0).max(0) as usize)
}

/// Evict the oldest staged row (lowest `sample.ts`, then `sample.seq`) — drop-oldest for best-effort.
/// `DELETE … ORDER BY … LIMIT` is not supported by the engine, so we DELETE the rows returned by a
/// subquery that picks the single oldest id (order keys in the projection — the idiom the drain uses,
/// debugging/store/order-by-needs-selected-idiom.md). The id never round-trips through host JSON
/// (a Thing does not deserialize into `serde_json::Value` — the same enum-tag mismatch as `record`).
async fn drop_oldest(store: &Store, ws: &str) -> Result<(), StoreError> {
    store
        .query_ws(
            ws,
            &format!(
                "DELETE (SELECT id, sample.ts AS _ts, sample.seq AS _seq FROM {STAGING_TABLE} \
                 ORDER BY _ts ASC, _seq ASC LIMIT 1)"
            ),
            vec![],
        )
        .await?;
    Ok(())
}

/// Divert a must-deliver sample to the dead-letter table (keyed like staging, so idempotent).
async fn dead_letter(store: &Store, ws: &str, sample: &Sample) -> Result<(), StoreError> {
    store
        .query_ws(
            ws,
            &format!(
                "UPSERT type::thing('{DEAD_LETTER_TABLE}', [$series, $producer, $seq]) CONTENT $row"
            ),
            vec![
                ("series".into(), Value::String(sample.series.clone())),
                ("producer".into(), Value::String(sample.producer.clone())),
                ("seq".into(), Value::Number(sample.seq.into())),
                ("row".into(), json!({ "sample": sample })),
            ],
        )
        .await?;
    Ok(())
}
