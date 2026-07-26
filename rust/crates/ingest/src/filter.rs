//! The write-time normalize predicates — "store or don't store", decided per sample as a batch
//! commits (series-normalize scope). This file is PURE: no store, no async, no I/O. It answers one
//! question — given a filter, a payload, its `ts`, and the last **committed** sample of this
//! `(series, producer)`, does this sample land? [`filter_pass`](crate::filter_pass) owns the batch
//! walk and the state round-trip; [`commit_batch`](crate::commit_batch) owns the transaction.
//!
//! **Why commit and not accept.** The staging append stays index-free and cheap (the
//! drain-backpressure invariant): a deadband needs the last committed value, and reading it on the
//! producer's own append re-couples producer rate to store queries — the exact coupling
//! drain-backpressure removed. At commit the batch is already in hand and the state read is one
//! query for the whole batch.
//!
//! **Counted, never silent.** Every drop increments a per-reason counter that rides the
//! [`CommitPass`](crate::CommitPass) out to the caller. An operator's own policy discarding a
//! `must-deliver` sample is delivered-then-filtered, not lost in the dark.
//!
//! **Evaluation order is cheap → stateful**: `drop` → `range` → `min_interval_ms` → `deadband`.
//! Non-numeric payloads skip the numeric predicates (`range`, `deadband`) untouched — a filter
//! configured for an analog point never silently eats an event/object series that shares its prefix.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// What a `range` violation does. Default `drop`: a −9999 sensor error clamped to −40 is a
/// plausible-looking lie, while dropped-and-counted is honest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RangeMode {
    #[default]
    Drop,
    Clamp,
}

/// An inclusive value band. Either bound may be absent (one-sided).
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Range {
    #[serde(default)]
    pub min: Option<f64>,
    #[serde(default)]
    pub max: Option<f64>,
    #[serde(default)]
    pub mode: RangeMode,
}

/// The change threshold below which a sample is redundant. `abs` is in the series' own units; `pct`
/// is relative to the last committed value.
///
/// When BOTH are set `abs` wins — a fixed floor is the one an operator can reason about at any
/// magnitude, and "whichever fires first" would silently make the tighter of two knobs the only
/// live one. (Rejected: taking the larger delta, which reads as "I set 0.5 and got 5".)
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Deadband {
    #[serde(default)]
    pub abs: Option<f64>,
    #[serde(default)]
    pub pct: Option<f64>,
}

impl Deadband {
    /// The effective threshold against `last` — `None` when neither knob is set (never filters).
    pub fn delta(&self, last: f64) -> Option<f64> {
        match (self.abs, self.pct) {
            (Some(a), _) => Some(a.abs()),
            (None, Some(p)) => Some(last.abs() * p.abs() / 100.0),
            (None, None) => None,
        }
    }
}

/// The write-time filter block on a retention policy. Every field defaults to inert, so a policy row
/// written before this slice existed keeps its exact meaning: store everything.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Filter {
    /// Accept-but-store-nothing mute. The live bus stream is untouched; only the store is silent.
    #[serde(default)]
    pub drop: bool,
    /// Keep at most one stored sample per N ms per `(series, producer)` — the FIRST of each interval.
    #[serde(default)]
    pub min_interval_ms: u64,
    #[serde(default)]
    pub deadband: Option<Deadband>,
    #[serde(default)]
    pub range: Option<Range>,
}

impl Filter {
    /// Does this filter need the last-committed state? A pure `drop`/`range` filter does not, so the
    /// commit path can skip the state round-trip entirely.
    pub fn needs_state(&self) -> bool {
        self.min_interval_ms > 0 || self.deadband.is_some()
    }

    /// Is this filter inert (equivalent to no filter at all)?
    pub fn is_inert(&self) -> bool {
        !self.drop && self.min_interval_ms == 0 && self.deadband.is_none() && self.range.is_none()
    }
}

/// Why a sample was not stored. One counter per reason (per pass, per prefix — per-series
/// granularity ships with the series-size observability slice, not here).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    Muted,
    Range,
    MinInterval,
    Deadband,
}

/// The per-reason tally of one commit pass. `clamped` counts samples that WERE stored, at a bound.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FilterCounts {
    pub muted: usize,
    pub range: usize,
    pub min_interval: usize,
    pub deadband: usize,
    /// Stored, but at a `range` bound rather than as read. Counted because a clamped value is
    /// indistinguishable from a real reading at the bound.
    pub clamped: usize,
}

impl FilterCounts {
    /// Samples the filter refused to store (excludes `clamped`, which stored).
    pub fn dropped(&self) -> usize {
        self.muted + self.range + self.min_interval + self.deadband
    }

    /// Is every counter zero?
    pub fn is_zero(&self) -> bool {
        *self == Self::default()
    }

    /// Tally one drop.
    pub fn count(&mut self, reason: Reason) {
        match reason {
            Reason::Muted => self.muted += 1,
            Reason::Range => self.range += 1,
            Reason::MinInterval => self.min_interval += 1,
            Reason::Deadband => self.deadband += 1,
        }
    }
}

/// The last sample this `(series, producer)` actually COMMITTED — the deadband/min-interval anchor.
/// Persisted on the series' `series_meta` row so it survives a node restart (a process-local cache
/// would re-open the deadband on every reboot, silently storing a burst of redundant samples).
///
/// `value` is `None` for a non-numeric payload: the anchor still advances the `ts` axis
/// (min-interval keeps working) without inventing a number to compare against.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LastCommitted {
    pub ts: u64,
    #[serde(default)]
    pub value: Option<f64>,
}

/// The verdict for one sample.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Decision {
    /// Store the payload as it arrived.
    Keep,
    /// Store, with the numeric payload replaced by this in-range bound.
    Clamp(f64),
    /// Do not store; tally under this reason.
    Drop(Reason),
}

/// Decide one sample against `filter`, given the `(series, producer)`'s last committed anchor.
///
/// Order is cheap → stateful so the common rejection costs least: a muted series never reads state,
/// and a range-rejected sample never consults the deadband. `last` is `None` for the first sample of
/// a `(series, producer)` — the stateful predicates always pass then, so a series' first sample is
/// never filtered away (there is nothing to be redundant against).
pub fn decide(filter: &Filter, payload: &Value, ts: u64, last: Option<&LastCommitted>) -> Decision {
    if filter.drop {
        return Decision::Drop(Reason::Muted);
    }

    // The numeric view of the payload, or `None` for a string/object/array/bool/null payload — the
    // gate that makes the numeric predicates skip an event series untouched.
    let raw = payload.as_f64();
    let mut value = raw;

    if let (Some(range), Some(v)) = (filter.range.as_ref(), raw) {
        let below = range.min.is_some_and(|m| v < m);
        let above = range.max.is_some_and(|m| v > m);
        if below || above {
            match range.mode {
                RangeMode::Drop => return Decision::Drop(Reason::Range),
                // Unwrap is sound: `below`/`above` are only true when that bound is `Some`.
                RangeMode::Clamp => {
                    value = Some(if below {
                        range.min.expect("min bound present when below")
                    } else {
                        range.max.expect("max bound present when above")
                    })
                }
            }
        }
    }

    if filter.min_interval_ms > 0 {
        if let Some(anchor) = last {
            // Keep the FIRST sample of each interval: anything landing before the anchor's interval
            // has elapsed is redundant. Deterministic under a re-drain — the first accepted commit
            // wins, and a replay of the same batch reaches the same verdict against the same anchor
            // (a "keep last" rule would have to rewrite an already-committed row per interval).
            if ts < anchor.ts.saturating_add(filter.min_interval_ms) {
                return Decision::Drop(Reason::MinInterval);
            }
        }
    }

    if let (Some(band), Some(v), Some(anchor)) = (filter.deadband.as_ref(), value, last) {
        if let Some(prev) = anchor.value {
            if let Some(delta) = band.delta(prev) {
                if (v - prev).abs() < delta {
                    return Decision::Drop(Reason::Deadband);
                }
            }
        }
    }

    match (value, raw) {
        (Some(v), Some(r)) if v != r => Decision::Clamp(v),
        _ => Decision::Keep,
    }
}
