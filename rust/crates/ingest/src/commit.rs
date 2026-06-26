//! The commit worker: drain one batch from staging and commit it to the `series` tables in **ONE
//! SurrealDB transaction**. This is the load-bearing seam of the exactly-once guarantee (ingest
//! scope):
//!   - **one batch = one transaction** — a die-mid-batch rolls the whole batch back (atomic), never
//!     a half-applied partial commit;
//!   - **commit is an UPSERT keyed on `[series, producer, seq]`** — a re-drain after a restart
//!     upserts each sample exactly once (idempotent), so "no double-commit on restart" is true, not
//!     hoped;
//!   - **the staged rows are deleted in the SAME transaction** as the series upsert — so a sample is
//!     either (committed to series AND removed from staging) or (still staged), never both or
//!     neither. A crash before COMMIT leaves the batch in staging for the next drain.
//!
//! `producer` is part of the key, so producer-A's seq=5 and producer-B's seq=5 on one series are
//! TWO rows — both survive (the two-producer-collision guarantee). Keying on `(series, seq)` would
//! have lost one; that weaker key is rejected by design.
//!
//! Payload is stored **typed, not opaque**: a scalar stays a number/bool, structured data a native
//! nested object, binary a record-as-content reference (buckets are unavailable per the store spike).
//! SurrealDB's `CONTENT` preserves the JSON value's type, so the richest form is what lands.

use lb_store::{Store, StoreError};
use serde_json::Value;

use crate::staging::{Staged, SERIES_TABLE, STAGING_TABLE};

/// Outcome of one commit pass: how many samples were committed exactly-once this batch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommitPass {
    pub committed: usize,
}

/// Drain up to `batch` staged samples from `ws` and commit them to the series tables in one
/// transaction. Returns the count committed (0 when staging is empty). Call repeatedly to drain.
pub async fn commit_batch(store: &Store, ws: &str, batch: usize) -> Result<CommitPass, StoreError> {
    let staged = drain(store, ws, batch).await?;
    if staged.is_empty() {
        return Ok(CommitPass { committed: 0 });
    }

    // Build one BEGIN…COMMIT with an UPSERT-into-series + DELETE-from-staging per sample. All-or-
    // nothing: a crash before COMMIT rolls every statement back and the batch stays in staging.
    let mut sql = String::from("BEGIN TRANSACTION;\n");
    let mut bindings: Vec<(String, Value)> = Vec::new();

    for (i, s) in staged.iter().enumerate() {
        let (se, pr, sq, ts, pl) = (
            format!("se{i}"),
            format!("pr{i}"),
            format!("sq{i}"),
            format!("ts{i}"),
            format!("pl{i}"),
        );
        // UPSERT keyed on the composite [series, producer, seq] → exactly-once on re-drain.
        sql.push_str(&format!(
            "UPSERT type::thing('{SERIES_TABLE}', [${se}, ${pr}, ${sq}]) \
             CONTENT {{ series: ${se}, producer: ${pr}, seq: ${sq}, ts: ${ts}, payload: ${pl} }};\n"
        ));
        // Delete the staged row in the SAME tx so commit + dequeue are atomic.
        sql.push_str(&format!(
            "DELETE type::thing('{STAGING_TABLE}', [${se}, ${pr}, ${sq}]);\n"
        ));
        bindings.push((se, Value::String(s.sample.series.clone())));
        bindings.push((pr, Value::String(s.sample.producer.clone())));
        bindings.push((sq, Value::Number(s.sample.seq.into())));
        bindings.push((ts, Value::Number(s.sample.ts.into())));
        bindings.push((pl, s.sample.payload.clone()));
    }
    sql.push_str("COMMIT TRANSACTION;");

    store.query_ws(ws, &sql, bindings).await?;
    Ok(CommitPass {
        committed: staged.len(),
    })
}

/// Read up to `batch` staged rows from `ws`, oldest-first (by seq then ts). The drain does NOT
/// delete here — deletion happens inside the commit transaction so it is atomic with the series
/// upsert. A crash between drain and commit simply re-reads the same rows next pass.
async fn drain(store: &Store, ws: &str, batch: usize) -> Result<Vec<Staged>, StoreError> {
    let mut resp = store
        .query_ws(
            ws,
            // SurrealDB requires an ORDER BY idiom to appear in the projection — so we also select
            // the order keys (debugging/store/order-by-needs-selected-idiom.md). We only consume
            // `sample` (via `Staged`); the extra fields are projection-only.
            &format!(
                "SELECT sample, sample.seq AS _seq, sample.ts AS _ts FROM {STAGING_TABLE} \
                 ORDER BY _seq ASC, _ts ASC LIMIT {batch}"
            ),
            vec![],
        )
        .await?;
    let rows: Vec<Staged> = resp.take(0).map_err(|e| StoreError::Decode(e.to_string()))?;
    Ok(rows)
}
