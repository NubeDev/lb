//! Apply the policies' write-time filters to one drained batch — the bridge between the pure
//! predicates ([`crate::filter`]), the durable anchors ([`crate::filter_state`]), and the commit
//! transaction ([`crate::commit`]).
//!
//! One responsibility: turn `[Staged]` into a per-sample verdict plus the anchor updates to persist.
//! It reads; it never writes — the caller folds the state into its own transaction so an anchor is
//! exactly as durable as the sample that moved it.
//!
//! **Evaluation order within the batch is `(ts, seq)` per `(series, producer)`, never drain order.**
//! The drain returns rows ordered by `seq` across every producer, and `seq` is monotonic per
//! `(series, producer)` ONLY — ordering a series by raw `seq` across producers is the bug in
//! `debugging/ingest/latest-pinned-to-pre-restart-sample.md`. A min-interval or deadband walked in
//! the wrong order would keep the wrong sample, so this pass sorts its own view first.

use std::collections::{BTreeMap, HashMap};

use lb_store::{Store, StoreError};

use crate::filter::{decide, Decision, FilterCounts, LastCommitted};
use crate::filter_state::{read_filter_state, ProducerState};
use crate::retention::{list_policies, resolve_policy};
use crate::staging::Staged;

/// What happens to one staged sample.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Verdict {
    /// Commit the payload as staged.
    Store,
    /// Commit, with the numeric payload replaced by this in-range bound.
    StoreClamped(f64),
    /// Do not commit. The staged row is still dequeued — the sample was accepted, then filtered.
    Dropped,
}

/// The batch's verdicts, tallies, and the anchors to persist.
pub struct FilterOutcome {
    /// Index-aligned with the `staged` slice handed in.
    pub verdicts: Vec<Verdict>,
    pub counts: FilterCounts,
    /// Only the series whose anchors actually moved — nothing else is rewritten.
    pub state: BTreeMap<String, ProducerState>,
}

impl FilterOutcome {
    /// The verdict-everything-stores outcome: what an unfiltered workspace gets, allocation-cheap.
    fn pass_through(n: usize) -> Self {
        Self {
            verdicts: vec![Verdict::Store; n],
            counts: FilterCounts::default(),
            state: BTreeMap::new(),
        }
    }
}

/// Decide the whole batch against `ws`'s retention policies.
///
/// Fast path: a workspace with no policy carrying a live `filter` block does ONE `list_policies`
/// read and returns pass-through — no state query, no sort, no per-sample work.
pub async fn filter_batch(
    store: &Store,
    ws: &str,
    staged: &[Staged],
) -> Result<FilterOutcome, StoreError> {
    let policies = list_policies(store, ws).await?;
    if !policies
        .iter()
        .any(|p| p.filter.is_some_and(|f| !f.is_inert()))
    {
        return Ok(FilterOutcome::pass_through(staged.len()));
    }

    // Resolve each distinct series to its governing filter ONCE (longest-prefix-wins), so the
    // per-sample loop is a map lookup rather than a scan of every policy.
    let mut governing: HashMap<&str, Option<crate::filter::Filter>> = HashMap::new();
    for s in staged {
        governing
            .entry(s.sample.series.as_str())
            .or_insert_with(|| {
                resolve_policy(&policies, &s.sample.series)
                    .and_then(|p| p.filter)
                    .filter(|f| !f.is_inert())
            });
    }

    // Only series under a STATEFUL filter need their anchors read.
    let stateful: Vec<String> = governing
        .iter()
        .filter(|(_, f)| f.is_some_and(|f| f.needs_state()))
        .map(|(name, _)| (*name).to_string())
        .collect();
    let mut anchors = read_filter_state(store, ws, &stateful).await?;

    // Walk in (series, producer, ts, seq) order — see the module note on why drain order is wrong.
    let mut order: Vec<usize> = (0..staged.len()).collect();
    order.sort_by(|&a, &b| {
        let (x, y) = (&staged[a].sample, &staged[b].sample);
        (&x.series, &x.producer, x.ts, x.seq).cmp(&(&y.series, &y.producer, y.ts, y.seq))
    });

    let mut verdicts = vec![Verdict::Store; staged.len()];
    let mut counts = FilterCounts::default();
    let mut moved: BTreeMap<String, ProducerState> = BTreeMap::new();

    for i in order {
        let smp = &staged[i].sample;
        let Some(filter) = governing.get(smp.series.as_str()).copied().flatten() else {
            continue; // no filter governs this series — stores as staged
        };
        let anchor = moved
            .get(&smp.series)
            .and_then(|p| p.get(&smp.producer))
            .or_else(|| anchors.get(&smp.series).and_then(|p| p.get(&smp.producer)))
            .copied();

        match decide(&filter, &smp.payload, smp.ts, anchor.as_ref()) {
            Decision::Drop(reason) => {
                counts.count(reason);
                verdicts[i] = Verdict::Dropped;
                // The anchor does NOT move: it tracks what was COMMITTED, so a run of dropped
                // samples stays measured against the last sample that actually landed. (Advancing it
                // on a drop would let a slow drift past the deadband one step at a time.)
            }
            Decision::Keep => {
                verdicts[i] = Verdict::Store;
                advance(
                    &mut moved,
                    smp.series.clone(),
                    smp.producer.clone(),
                    smp.ts,
                    smp.payload.as_f64(),
                );
            }
            Decision::Clamp(v) => {
                counts.clamped += 1;
                verdicts[i] = Verdict::StoreClamped(v);
                // The CLAMPED value is the anchor — it is what the store now holds, so the next
                // deadband compares against reality rather than the reading that never landed.
                advance(
                    &mut moved,
                    smp.series.clone(),
                    smp.producer.clone(),
                    smp.ts,
                    Some(v),
                );
            }
        }
    }

    // Merge the moved anchors onto each series' existing map: a producer that wrote nothing this
    // batch must keep its anchor, so the persisted value is the FULL map, not just this batch's.
    let mut state = BTreeMap::new();
    for (series, producers) in moved {
        let mut full = anchors.remove(&series).unwrap_or_default();
        full.extend(producers);
        state.insert(series, full);
    }

    Ok(FilterOutcome {
        verdicts,
        counts,
        state,
    })
}

/// Record `(series, producer)`'s new committed anchor. A non-numeric payload still advances `ts`
/// (min-interval keeps working) with `value: None` rather than inventing a number.
fn advance(
    moved: &mut BTreeMap<String, ProducerState>,
    series: String,
    producer: String,
    ts: u64,
    value: Option<f64>,
) {
    moved
        .entry(series)
        .or_default()
        .insert(producer, LastCommitted { ts, value });
}
