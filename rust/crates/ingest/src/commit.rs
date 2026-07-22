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
//! Since the series schema slice, commit also:
//!   - ensures the series-plane schema/indexes and stores `ts` as a real `datetime`
//!     (`time::from::millis` — the wire `ts` is epoch ms);
//!   - enforces the **series cardinality cap**: a sample naming a NEW series past the per-workspace
//!     cap is diverted to the dead-letter table (in the same tx), never silently dropped and never
//!     an unbounded index;
//!   - registers each series in `series_meta` and converts its wire `labels` to tag edges once
//!     (`labels::apply_labels`) — so `series.find` actually finds what ingest wrote.
//!
//! Payload is stored **typed, not opaque** — SurrealDB's `CONTENT` preserves the JSON value's type.

use std::collections::HashSet;

use lb_store::{Store, StoreError};
use serde_json::Value;

use crate::labels::apply_labels;
use crate::meta::{is_registered, register, series_count, DEFAULT_SERIES_CAP};
use crate::schema::{ensure_series_schema, SERIES_LATEST_TABLE};
use crate::staging::{Staged, DEAD_LETTER_TABLE, SERIES_TABLE, STAGING_TABLE};

/// Outcome of one commit pass: how many samples were committed exactly-once this batch, and how
/// many were diverted to the dead-letter table by the series cardinality cap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommitPass {
    pub committed: usize,
    pub dead_lettered: usize,
}

/// Drain up to `batch` staged samples from `ws` and commit them under the default series
/// cardinality cap. Returns the counts (0/0 when staging is empty). Call repeatedly to drain.
pub async fn commit_batch(store: &Store, ws: &str, batch: usize) -> Result<CommitPass, StoreError> {
    commit_batch_capped(store, ws, batch, DEFAULT_SERIES_CAP).await
}

/// [`commit_batch`] with an explicit per-workspace cap on distinct series names (`0` = unbounded).
pub async fn commit_batch_capped(
    store: &Store,
    ws: &str,
    batch: usize,
    series_cap: usize,
) -> Result<CommitPass, StoreError> {
    let staged = drain(store, ws, batch).await?;
    if staged.is_empty() {
        return Ok(CommitPass {
            committed: 0,
            dead_lettered: 0,
        });
    }
    ensure_series_schema(store, ws).await?;

    // Cardinality gate: decide, per distinct series name in this batch, whether it is admitted.
    // Existing series always pass; new names are admitted while the registry stays under the cap.
    let mut admitted_series: HashSet<String> = HashSet::new();
    let mut rejected_series: HashSet<String> = HashSet::new();
    let mut count = series_count(store, ws).await?;
    for s in &staged {
        let name = &s.sample.series;
        if admitted_series.contains(name) || rejected_series.contains(name) {
            continue;
        }
        if is_registered(store, ws, name).await? {
            admitted_series.insert(name.clone());
        } else if series_cap == 0 || count < series_cap {
            register(store, ws, name).await?;
            count += 1;
            admitted_series.insert(name.clone());
        } else {
            rejected_series.insert(name.clone());
        }
    }

    // Per-series NEWEST sample in this batch, by (ts, seq) — the axis `latest` orders on. Used to
    // advance the `series_latest` pointer transactionally (schema::SERIES_LATEST_TABLE), so reads are
    // a point lookup instead of a full ordered scan. Only admitted (non-dead-lettered) samples count.
    let mut batch_newest: std::collections::HashMap<&str, &crate::sample::Sample> =
        std::collections::HashMap::new();
    for s in &staged {
        if !admitted_series.contains(&s.sample.series) {
            continue;
        }
        let smp = &s.sample;
        batch_newest
            .entry(smp.series.as_str())
            .and_modify(|cur| {
                if (smp.ts, smp.seq) > (cur.ts, cur.seq) {
                    *cur = smp;
                }
            })
            .or_insert(smp);
    }

    // Build one BEGIN…COMMIT: admitted samples UPSERT into series; cap-rejected samples divert to
    // the dead-letter table. Both delete their staged row in the SAME tx (atomic dequeue).
    let mut sql = String::from("BEGIN TRANSACTION;\n");
    let mut bindings: Vec<(String, Value)> = Vec::new();
    let mut committed = 0;
    let mut dead_lettered = 0;

    for (i, s) in staged.iter().enumerate() {
        let (se, pr, sq, ts, pl) = (
            format!("se{i}"),
            format!("pr{i}"),
            format!("sq{i}"),
            format!("ts{i}"),
            format!("pl{i}"),
        );
        if admitted_series.contains(&s.sample.series) {
            // UPSERT keyed on the composite [series, producer, seq] → exactly-once on re-drain.
            // `ts` lands as a real datetime (wire ts is epoch ms).
            sql.push_str(&format!(
                "UPSERT type::thing('{SERIES_TABLE}', [${se}, ${pr}, ${sq}]) \
                 CONTENT {{ series: ${se}, producer: ${pr}, seq: ${sq}, \
                 ts: time::from::millis(${ts}), payload: ${pl} }};\n"
            ));
            committed += 1;
        } else {
            // Over the series cap: dead-letter, never a silent drop (and never a new index entry).
            sql.push_str(&format!(
                "UPSERT type::thing('{DEAD_LETTER_TABLE}', [${se}, ${pr}, ${sq}]) \
                 CONTENT {{ sample: {{ series: ${se}, producer: ${pr}, seq: ${sq}, ts: ${ts}, \
                 payload: ${pl} }}, reason: 'series-cap' }};\n"
            ));
            dead_lettered += 1;
        }
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

    // Advance the newest-sample pointer for each series touched this batch — in the SAME tx, so the
    // pointer is exactly as durable as the raw write. It moves FORWARD only: the guard skips the
    // UPSERT when an existing pointer already holds a `(ts, seq)` ≥ this batch's newest (a replayed
    // or late batch never regresses it). `ts` stored as epoch ms (integer) so the guard is a plain
    // numeric compare — `latest` reads it straight back without a datetime round-trip.
    for (j, (series, smp)) in batch_newest.iter().enumerate() {
        let (lse, lpr, lsq, lts, lpl) = (
            format!("lse{j}"),
            format!("lpr{j}"),
            format!("lsq{j}"),
            format!("lts{j}"),
            format!("lpl{j}"),
        );
        // Read the current pointer's (ts, seq); UPSERT only if this batch's newest strictly beats it
        // (or none exists). `??` defaults an absent pointer to below any real sample.
        sql.push_str(&format!(
            "LET $cur{j} = (SELECT ts, seq FROM ONLY type::thing('{SERIES_LATEST_TABLE}', ${lse}))?.{{ ts: ts, seq: seq }} ?? {{ ts: -1, seq: -1 }};\n\
             IF [${lts}, ${lsq}] > [$cur{j}.ts, $cur{j}.seq] {{ \
               UPSERT type::thing('{SERIES_LATEST_TABLE}', ${lse}) CONTENT {{ \
                 series: ${lse}, producer: ${lpr}, seq: ${lsq}, ts: ${lts}, payload: ${lpl} }}; \
             }};\n"
        ));
        bindings.push((lse, Value::String((*series).to_string())));
        bindings.push((lpr, Value::String(smp.producer.clone())));
        bindings.push((lsq, Value::Number(smp.seq.into())));
        bindings.push((lts, Value::Number(smp.ts.into())));
        bindings.push((lpl, smp.payload.clone()));
    }

    sql.push_str("COMMIT TRANSACTION;");

    store.query_ws(ws, &sql, bindings).await?;

    // Label→tag conversion, once per series (post-tx: edges are derived truth, re-derivable).
    let mut labeled: HashSet<&str> = HashSet::new();
    for s in &staged {
        if admitted_series.contains(&s.sample.series) && labeled.insert(s.sample.series.as_str()) {
            apply_labels(store, ws, &s.sample).await?;
        }
    }

    Ok(CommitPass {
        committed,
        dead_lettered,
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
    let rows: Vec<Staged> = resp
        .take(0)
        .map_err(|e| StoreError::Decode(e.to_string()))?;
    Ok(rows)
}
