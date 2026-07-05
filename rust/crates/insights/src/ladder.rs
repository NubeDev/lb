//! `ladder` — the **pure** ladder state machine (insight-notify-scope.md).
//!
//! This is the unit-test surface — a pure function with **zero I/O**. All timing is on the
//! injected logical clock (`now`), so tests are deterministic and the same intent sequence + clock
//! always yields the same deliveries (the scope's determinism test). The host's raise path feeds
//! it intents; the digest reactor feeds it ticks (window elapses).
//!
//! ## The state machine in one paragraph (SCOPE: notify-scope.md §"The state machine")
//!
//! Per `(sub, dedup_key)` state at a level `0..=4`. On each input (`intent | tick`):
//!   1. **Breakthrough check** (first): a `Reopen`/`Escalate` intent, OR no prior state row, OR
//!      `last_severity` is strictly less than the new intent's severity ⇒ deliver now, KEEP the
//!      level (a breakthrough does NOT reset the ladder — the noise history stands).
//!   2. **Ack suppression**: intents for an `acked` insight update `pending`/`window_hits` but
//!      never deliver; breakthroughs still apply (escalation/re-open un-suppress by definition).
//!   3. **Escalate**: ≥ `escalation_threshold` deliveries-worth of noise within the current
//!      window ⇒ `level + 1` (clamped at 4).
//!   4. **Decay**: one fully-quiet window at the current level ⇒ `level - 1` (clamped at 0).
//!   5. **L0 cooldown**: at L0, one immediate post per cooldown per key (extra raises within the
//!      cooldown accumulate into `pending` for the next post).
//!   6. **Pinned subs** (`throttle_override`): skip escalate/decay; keep breakthroughs + ack-suppress.
//!
//! **STUB**: the algorithm is the single most load-bearing thing the implementing session owns.
//! The signature, the input enum, the types, and the `Delivery` output are stable — only the body
//! is a `todo!()` so a green-but-lying stub is impossible.

use serde::{Deserialize, Serialize};

use crate::intent::{Intent, IntentKind};
use crate::notify_state::{NotifyState, PendingAccumulator};
use crate::policy::Policy;
use crate::severity::Severity;

/// The five ladder levels as a typed enum (the state row stores `level: u8`; this is the
/// type-safe projection). `L0` immediate … `L4` monthly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Level {
    L0,
    L1,
    L2,
    L3,
    L4,
}

impl Level {
    pub fn from_u8(n: u8) -> Self {
        match n {
            0 => Level::L0,
            1 => Level::L1,
            2 => Level::L2,
            3 => Level::L3,
            _ => Level::L4,
        }
    }
    pub fn to_u8(self) -> u8 {
        self as u8
    }
}

/// One delivery the state machine decided to fire NOW. The host (raise path or digest reactor)
/// turns this into a `channel.post` under the sub's stored principal (fire-time re-checked).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Delivery {
    /// The sub this delivery is for.
    pub sub_id: String,
    /// The insight + key this delivery is about.
    pub insight_id: String,
    pub dedup_key: String,
    /// Why this broke through (or `"l0"` for an immediate post at L0 within the cooldown).
    pub reason: DeliveryReason,
    /// The severity this delivery carries.
    pub severity: Severity,
    /// The logical ts the host should stamp on the resulting `channel.post`.
    pub ts: u64,
}

/// Why a delivery fired. Surfaced so the digest message can say "this broke through because
/// severity escalated" vs "this is the L0 immediate post".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DeliveryReason {
    /// First-ever occurrence of this key on this sub.
    FirstKey,
    /// Severity escalated beyond the previous firing.
    Escalation,
    /// Re-open after `resolved`.
    Reopen,
    /// The L0 immediate post (within the cooldown).
    L0Immediate,
    /// A digest window elapsed with `pending.count > 0` (L1..L4).
    Digest,
}

/// The accumulator for the current window — what the state machine mutates as it processes
/// inputs. Mirrors `PendingAccumulator` but carried alongside the state in the algorithm.
pub type WindowAccumulator = PendingAccumulator;

/// The input to the state machine — an intent (a raise produced a match) or a tick (the digest
/// reactor says a window elapsed). The reactor passes `now`; the raise path passes `now` + the
/// intent + the parent insight's acked-ness.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LadderInput<'a> {
    /// A matched raise arrived. `acked` is the parent insight's post-raise status (suppression).
    Intent {
        intent: &'a Intent,
        acked: bool,
        now: u64,
    },
    /// The digest reactor advanced the clock — windows may have elapsed.
    Tick { now: u64 },
}

/// The pure state-machine step: `(state, input, policy) -> (state', deliveries)`.
///
/// `state: Option<NotifyState>` — `None` means "no row yet for this (sub, key)" (the first-key
/// breakthrough case). Returns the new state (which the host persists) + any deliveries to fire
/// now (which the host posts under the sub's stored principal, re-checked). The function is the
/// ONLY writer of `NotifyState` transitions — all rule-of-thumb windows, escalate/decay math, and
/// breakthrough checks live here.
// SCOPE: docs/scope/insights/insight-notify-scope.md §"The state machine" + §"Example flow"
pub fn ladder_step(
    _state: Option<NotifyState>,
    _input: LadderInput<'_>,
    _policy: &Policy,
    _throttle_override: Option<crate::policy::ThrottleOverride>,
    _muted: bool,
    _member_kill_switch_on: bool,
) -> (NotifyState, Vec<Delivery>) {
    // 1. If `member_kill_switch_on == false` ⇒ deliveries skipped entirely (accounting continues
    //    so re-enabling picks up sane digests). Return (state', []).
    // 2. If `muted` ⇒ same — accounting continues, delivery skipped.
    // 3. Build the state (or seed `NotifyState::default_for(sub_id, dedup_key, now)` if None).
    // 4. Handle `Tick`: for each fully-elapsed window at the current level (clamped to the largest
    //    advance that's sane) — if pending.count > 0 ⇒ one Digest delivery, zero pending, then
    //    decay level by 1 (one quiet window ⇒ level - 1). If pending.count == 0 ⇒ just decay.
    // 5. Handle `Intent`:
    //    a. Breakthrough check FIRST (regardless of throttle_override): kind=Reopen OR
    //       kind=Escalate OR last_severity < intent.severity OR no prior state ⇒ deliver now,
    //       keep the level (a breakthrough does NOT reset the ladder).
    //    b. Ack suppression: if acked (and not a breakthrough) ⇒ update pending/window_hits but
    //       return [] deliveries.
    //    c. Escalate: window_hits += 1; if window_hits >= escalation_threshold (and no
    //       throttle_override pin) ⇒ level = min(level + 1, 4), reset window_hits = 0, advance
    //       window_start = now.
    //    d. At L0 within the cooldown: post now (L0Immediate) + zero the pending that accumulated
    //       within this cooldown.
    //    e. At L1..L4: accumulate into pending (no immediate delivery — the reactor's Tick will
    //       digest it).
    // 6. Update last_severity = intent.severity; return (state', deliveries).
    //
    // This is the algorithm. Keep it pure. The implementing session owns the body; the signature
    // and types are stable so the host + tests wire against them today.
    let _ = (
        IntentKind::Raise,
        Level::L0,
        DeliveryReason::FirstKey,
        WindowAccumulator::default(),
    );
    todo!("insights: ladder state machine — SCOPE: notify-scope.md §The state machine + §Example flow")
}
