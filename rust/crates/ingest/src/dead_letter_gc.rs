//! The dead-letter horizon — the retention pass for [`DEAD_LETTER_TABLE`] (disk-budget scope,
//! decision 7). Must-deliver samples diverted by the staging bound ([`crate::enforce_bound`]) were
//! the one ingest table nothing ever pruned: bounded-by-default on the series plane is worth little
//! if the overflow table beside it still grows forever.
//!
//! **Its own horizon, deliberately separate from `raw_for_ms`.** Dead letters are diagnostic, and
//! they are small. They are worth keeping longer than the data that produced them — and a shared
//! horizon would mean that tightening series retention to debug a disk problem silently destroys the
//! evidence of *why* records were dead-lettered. So the horizon is a constant here, not a policy
//! field: there is nothing per-prefix about it (a dead letter is an operational event, not a data
//! plane), and a knob nobody can set wrong is one fewer way to lose the evidence.
//!
//! **This is the existing GC machinery, not a new one** — `run_gc` calls it on the same per-workspace
//! tick, with the same caller-injected `now_ms` (determinism §3), and it adds no table.
//!
//! Note what pruning does NOT do on this engine: a delete is a tombstone APPENDED to the commit log,
//! so a pass here bounds the ROW count and momentarily grows the bytes on disc. Only a compaction
//! reclaims them (`scope/store/disk-budget-scope.md`, the append-only ordering rule).

use lb_store::{Store, StoreError};
use serde_json::Value;

use crate::staging::DEAD_LETTER_TABLE;

/// How long a dead-lettered sample is kept: 30 days.
///
/// Longer than any plausible raw horizon on purpose (see the module docs). At the ~700 bytes/sample
/// measured for the series plane, 30 days of dead letters is only large if the overflow itself is
/// pathological — in which case the table is the symptom an operator most needs to still be there.
pub const DEAD_LETTER_KEEP_MS: u64 = 30 * 24 * 60 * 60 * 1000;

/// Delete dead letters in `ws` older than `keep_for_ms` at logical time `now_ms`. Returns how many
/// rows were evicted. `keep_for_ms == 0` keeps them forever (the same `0 = unbounded` grammar every
/// other horizon in this crate uses).
///
/// A row's age is its `dead_at` stamp — when the node diverted it, which is the only honest age for
/// a diagnostic record. Rows written before that field existed fall back to the sample's own `ts`:
/// the producer's clock is untrusted, but for a row that has no other timestamp it is strictly
/// better than an upgrade that pins the whole table to "never expires".
pub async fn prune_dead_letters(
    store: &Store,
    ws: &str,
    now_ms: u64,
    keep_for_ms: u64,
) -> Result<usize, StoreError> {
    if keep_for_ms == 0 || keep_for_ms > now_ms {
        return Ok(0); // unbounded, or the horizon has not elapsed yet on this clock
    }
    let cutoff = now_ms - keep_for_ms;
    // COUNT then DELETE over one predicate, the idiom `evict_raw` uses — and `query_ws_retrying` for
    // the same reason: this races the inline drains that write into the same workspace under
    // optimistic MVCC, and the pass is idempotent so a retried run evicts the same rows exactly once.
    let pred = "(dead_at IS NOT NONE AND dead_at < $cutoff) \
                OR (dead_at IS NONE AND sample.ts < $cutoff)";
    let mut resp = store
        .query_ws_retrying(
            ws,
            &format!(
                "SELECT count() FROM {DEAD_LETTER_TABLE} WHERE {pred} GROUP ALL;
                 DELETE {DEAD_LETTER_TABLE} WHERE {pred};"
            ),
            vec![("cutoff".into(), Value::Number(cutoff.into()))],
        )
        .await?;
    let n: Option<i64> = resp
        .take("count")
        .map_err(|e| StoreError::Decode(e.to_string()))?;
    Ok(n.unwrap_or(0).max(0) as usize)
}
