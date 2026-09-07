//! The commit engine: write a batch of samples to the `series` tables in **ONE SurrealDB
//! transaction**. This is the load-bearing seam of the exactly-once guarantee (ingest scope):
//!   - **one batch = one transaction** — a die-mid-batch rolls the whole batch back (atomic), never
//!     a half-applied partial commit;
//!   - **commit is an UPSERT keyed on `[series, producer, seq]`** — a producer that re-pushes after
//!     a reconnect or a lost ack upserts each sample exactly once (idempotent), so "no double-write
//!     on retry" is true, not hoped.
//!
//! `producer` is part of the key, so producer-A's seq=5 and producer-B's seq=5 on one series are
//! TWO rows — both survive (the two-producer-collision guarantee). Keying on `(series, seq)` would
//! have lost one; that weaker key is rejected by design.
//!
//! Since the series schema slice, commit also:
//!   - ensures the series-plane schema/indexes and stores `ts` as a real `datetime`
//!     (`time::from_millis` — the wire `ts` is epoch ms);
//!   - enforces the **series cardinality cap**: a sample naming a NEW series past the per-workspace
//!     cap is diverted to the dead-letter table (in the same tx), never silently dropped and never
//!     an unbounded index;
//!   - registers each series in `series_meta` and converts its wire `labels` to tag edges once
//!     (`labels::apply_labels`) — so `series.find` actually finds what ingest wrote.
//!
//! Payload is stored **typed, not opaque** — SurrealDB's `CONTENT` preserves the JSON value's type.
//!
//! **One engine, one entry point.** [`commit_direct`](crate::commit_direct) chunks a caller's batch
//! and feeds it to this builder. That is the whole write path — there is no second way in, so
//! "exactly-once is keyed on `[series, producer, seq]`" is one fact in one place.

use std::collections::HashSet;

use lb_store::{Store, StoreError};
use serde_json::{json, Value};

use crate::filter::FilterCounts;
use crate::filter_pass::{filter_batch, Verdict};
use crate::filter_state::{write_filter_state_bindings, write_filter_state_sql};
use crate::labels::apply_labels;
use crate::meta::{is_registered, register, series_count};
use crate::sample::Sample;
use crate::schema::{ensure_series_schema, SERIES_LATEST_TABLE};
use crate::tables::{DEAD_LETTER_TABLE, SERIES_TABLE};

/// Outcome of one commit pass: how many samples were committed exactly-once this batch, how many
/// were diverted to the dead-letter table by the series cardinality cap, and what the write-time
/// normalize filters discarded (per reason — counted, never silent).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CommitPass {
    pub committed: usize,
    pub dead_lettered: usize,
    /// Per-reason tally of samples the policy's `filter` refused to store, plus the `clamped` count
    /// of samples stored at a `range` bound (series-normalize scope).
    pub filtered: FilterCounts,
}

impl CommitPass {
    /// How many samples this pass HANDLED — committed, dead-lettered, or filtered away.
    ///
    /// This, never `committed`, is the "did the pass do anything?" signal. A muted or heavily
    /// deadbanded prefix commits ZERO rows while handling a full batch, so anything that reads
    /// `committed == 0` as "nothing was there" is wrong
    /// (`debugging/ingest/filtered-batch-stops-the-drain-loop.md`).
    pub fn handled(&self) -> usize {
        self.committed + self.dead_lettered + self.filtered.dropped()
    }
}

/// Commit an already-materialized batch of samples to the series plane in ONE transaction.
///
/// The single commit engine, and the only way a sample reaches the series plane. One batch = one tx,
/// the cardinality gate, the normalize filters, the forward-only `series_latest` pointer, and the
/// filter anchors persisted in the same tx as the samples that moved them.
pub async fn commit_samples(
    store: &Store,
    ws: &str,
    samples: &[Sample],
    series_cap: usize,
) -> Result<CommitPass, StoreError> {
    if samples.is_empty() {
        return Ok(CommitPass::default());
    }
    ensure_series_schema(store, ws).await?;

    // NORMALIZE FILTERS run FIRST — before the cardinality gate (series-normalize scope). A sample
    // the operator's own policy discards must not mint a `series_meta` registry row on its way out:
    // a muted prefix would otherwise consume the workspace's distinct-series budget for data it
    // never stores. Filtering is also strictly cheaper than the gate's per-series registry probes.
    let filtered = filter_batch(store, ws, samples).await?;

    // Cardinality gate: decide, per distinct series name in this batch, whether it is admitted.
    // Existing series always pass; new names are admitted while the registry stays under the cap.
    let mut admitted_series: HashSet<String> = HashSet::new();
    let mut rejected_series: HashSet<String> = HashSet::new();
    let mut count = series_count(store, ws).await?;
    for (i, s) in samples.iter().enumerate() {
        if filtered.verdicts[i] == Verdict::Dropped {
            continue;
        }
        let name = &s.series;
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
    //
    // A filter-dropped sample is excluded here too: the newest STORED sample is what `series.latest`
    // must return. Letting a dropped sample advance the pointer would make `latest` report a value
    // no `series.read` can find — the pointer's whole contract is that it mirrors a committed row.
    let mut batch_newest: std::collections::HashMap<&str, (&crate::sample::Sample, Value)> =
        std::collections::HashMap::new();
    for (i, s) in samples.iter().enumerate() {
        if !admitted_series.contains(&s.series) || filtered.verdicts[i] == Verdict::Dropped {
            continue;
        }
        let payload = stored_payload(&s.payload, filtered.verdicts[i]);
        let smp = s;
        batch_newest
            .entry(smp.series.as_str())
            .and_modify(|cur| {
                if (smp.ts, smp.seq) > (cur.0.ts, cur.0.seq) {
                    *cur = (smp, payload.clone());
                }
            })
            .or_insert((smp, payload));
    }

    // Build one BEGIN…COMMIT: admitted samples UPSERT into series; cap-rejected samples divert to
    // the dead-letter table.
    // Declare every table this transaction touches, first, in the same transaction.
    //
    // SurrealDB 3 raises `NotFoundError::Table` for a SELECT/UPDATE/DELETE against a table nothing
    // has written, and inside a TRANSACTION that aborts everything after it — the caller then sees
    // only "The query was not executed due to a failed transaction", nowhere near the real cause. A
    // first commit on a fresh node hits exactly that: `series_latest` has no
    // rows yet. `lb_store` restores the "absent table reads as empty" contract for ordinary
    // statements but cannot help inside a transaction, where the writes genuinely did not happen.
    // Idempotent, and the same thing prefs / agent config / tags / undo do for their own tables.
    let mut sql = format!(
        "BEGIN TRANSACTION;\n\
         DEFINE TABLE IF NOT EXISTS {SERIES_TABLE};\n\
         DEFINE TABLE IF NOT EXISTS {SERIES_LATEST_TABLE};\n\
         DEFINE TABLE IF NOT EXISTS {DEAD_LETTER_TABLE};\n"
    );
    let mut bindings: Vec<(String, Value)> = Vec::new();
    // WHEN this node diverted a sample, stamped once for the whole transaction. `prune_dead_letters`
    // ages a row on this, falling back to the sample's own `ts` when it is absent — and that fallback
    // is the producer's UNTRUSTED clock, so a skewed producer could make its dead letters immortal
    // and grow the one ingest table nothing else bounds. Stamping it here closes that: the cap
    // divert is the only writer of dead letters, so no row reaches the table unstamped.
    bindings.push((
        "dead_at".into(),
        Value::Number(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis().min(u64::MAX as u128) as u64)
                .unwrap_or(0)
                .into(),
        ),
    ));
    let mut committed = 0;
    let mut dead_lettered = 0;

    for (i, s) in samples.iter().enumerate() {
        let (se, pr, sq, ts, pl) = (
            format!("se{i}"),
            format!("pr{i}"),
            format!("sq{i}"),
            format!("ts{i}"),
            format!("pl{i}"),
        );
        if filtered.verdicts[i] == Verdict::Dropped {
            // Filtered by the operator's own policy: nothing is stored. The sample was ACCEPTED and
            // then filtered, not lost in transit — it is tallied per-reason on the pass result, so
            // the discard is observable to the writer.
        } else if admitted_series.contains(&s.series) {
            // UPSERT keyed on the composite [series, producer, seq] → exactly-once on a replay.
            // `ts` lands as a real datetime (wire ts is epoch ms).
            sql.push_str(&format!(
                "UPSERT type::record('{SERIES_TABLE}', [${se}, ${pr}, ${sq}]) \
                 CONTENT {{ series: ${se}, producer: ${pr}, seq: ${sq}, \
                 ts: time::from_millis(${ts}), payload: ${pl} }};\n"
            ));
            committed += 1;
        } else {
            // Over the series cap: dead-letter, never a silent drop (and never a new index entry).
            sql.push_str(&format!(
                "UPSERT type::record('{DEAD_LETTER_TABLE}', [${se}, ${pr}, ${sq}]) \
                 CONTENT {{ sample: {{ series: ${se}, producer: ${pr}, seq: ${sq}, ts: ${ts}, \
                 payload: ${pl} }}, reason: 'series-cap', dead_at: $dead_at }};\n"
            ));
            dead_lettered += 1;
        }
        bindings.push((se, Value::String(s.series.clone())));
        bindings.push((pr, Value::String(s.producer.clone())));
        bindings.push((sq, Value::Number(s.seq.into())));
        bindings.push((ts, Value::Number(s.ts.into())));
        // A clamped sample stores the BOUND, not the reading — that is the whole point of `clamp`
        // mode, and the anchor the next deadband compares against was advanced to the same value.
        bindings.push((pl, stored_payload(&s.payload, filtered.verdicts[i])));
    }

    // Advance the newest-sample pointer for each series touched this batch — in the SAME tx, so the
    // pointer is exactly as durable as the raw write. It moves FORWARD only: the guard skips the
    // UPSERT when an existing pointer already holds a `(ts, seq)` ≥ this batch's newest (a replayed
    // or late batch never regresses it). `ts` stored as epoch ms (integer) so the guard is a plain
    // numeric compare — `latest` reads it straight back without a datetime round-trip.
    for (j, (series, (smp, payload))) in batch_newest.iter().enumerate() {
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
            "LET $cur{j} = (SELECT ts, seq FROM ONLY type::record('{SERIES_LATEST_TABLE}', ${lse})).{{ ts: ts, seq: seq }} ?? {{ ts: -1, seq: -1 }};\n\
             IF [${lts}, ${lsq}] > [$cur{j}.ts, $cur{j}.seq] {{ \
               UPSERT type::record('{SERIES_LATEST_TABLE}', ${lse}) CONTENT {{ \
                 series: ${lse}, producer: ${lpr}, seq: ${lsq}, ts: ${lts}, payload: ${lpl} }}; \
             }};\n"
        ));
        bindings.push((lse, Value::String((*series).to_string())));
        bindings.push((lpr, Value::String(smp.producer.clone())));
        bindings.push((lsq, Value::Number(smp.seq.into())));
        bindings.push((lts, Value::Number(smp.ts.into())));
        bindings.push((lpl, payload.clone()));
    }

    // Persist the filter anchors in the SAME transaction as the samples that moved them. A crash
    // between "sample committed" and "anchor advanced" would re-open the deadband for one batch and
    // store a redundant sample; inside the tx that window does not exist. This also makes the anchor
    // survive a node restart, which a process-local cache would not (`filter_state.rs`).
    for (k, (series, state)) in filtered.state.iter().enumerate() {
        sql.push_str(&write_filter_state_sql(k));
        bindings.extend(write_filter_state_bindings(k, series, state));
    }

    sql.push_str("COMMIT TRANSACTION;");

    // Bounded retry-on-conflict (`store::query_ws_retrying`): with ≥2 producers pushing every couple
    // of seconds, two concurrent commits race on the shared `series`/`series_latest` rows —
    // SurrealDB's optimistic MVCC aborts one with `read or write conflict … can be retried`, and the
    // GC pass evicting raw from `series` adds a second collision surface. Retrying the WHOLE
    // transaction is safe: it is atomic (a conflict aborts and fully rolls back — never a partial
    // commit to reconcile) and idempotent (the samples UPSERT is keyed on `[series, producer, seq]`),
    // so a retry re-applies the batch exactly once — the same guarantee exactly-once relies on.
    store.query_ws_retrying(ws, &sql, bindings).await?;

    // Label→tag conversion, once per series (post-tx: edges are derived truth, re-derivable).
    let mut labeled: HashSet<&str> = HashSet::new();
    for s in samples {
        if admitted_series.contains(&s.series) && labeled.insert(s.series.as_str()) {
            apply_labels(store, ws, s).await?;
        }
    }

    Ok(CommitPass {
        committed,
        dead_lettered,
        filtered: filtered.counts,
    })
}

/// The payload a verdict actually stores: the sample's own value, or the `range` bound a `clamp` chose.
///
/// A clamped value is indistinguishable from a real reading at the bound — which is exactly why the
/// pass counts it (`filtered.clamped`) and why `drop` is the default mode.
fn stored_payload(payload: &Value, verdict: Verdict) -> Value {
    match verdict {
        Verdict::StoreClamped(v) => json!(v),
        _ => payload.clone(),
    }
}
