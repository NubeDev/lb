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
    state: Option<NotifyState>,
    input: LadderInput<'_>,
    policy: &Policy,
    throttle_override: Option<crate::policy::ThrottleOverride>,
    muted: bool,
    member_kill_switch_on: bool,
) -> (NotifyState, Vec<Delivery>) {
    // Whether this member/sub is allowed to *deliver* right now. Accounting always continues so a
    // re-enable (kill switch back on, unmute) picks up sane digests — only the emitted deliveries
    // are gated (notify-scope §"Member kill switch off" + §"Muted sub").
    let deliver_allowed = member_kill_switch_on && !muted;

    match input {
        LadderInput::Tick { now } => tick(state, policy, throttle_override, deliver_allowed, now),
        LadderInput::Intent { intent, acked, now } => intent_step(
            state,
            intent,
            acked,
            policy,
            throttle_override,
            deliver_allowed,
            now,
        ),
    }
}

/// The window size (logical-ts units) for the current level, honouring a pinned override.
fn window_for(level: u8, policy: &Policy) -> u64 {
    policy.windows[(level as usize).min(4)]
}

/// Advance one input of kind [`LadderInput::Tick`] — the digest reactor says the clock moved. For
/// each fully-elapsed window at the current level: if pending accumulated, emit one `Digest`
/// delivery and zero the pending; either way decay one level (a fully-quiet window at this level ⇒
/// `level - 1`). A pinned sub (`throttle_override`) skips decay but still digests its pending.
fn tick(
    state: Option<NotifyState>,
    policy: &Policy,
    throttle_override: Option<crate::policy::ThrottleOverride>,
    deliver_allowed: bool,
    now: u64,
) -> (NotifyState, Vec<Delivery>) {
    // No state row ⇒ nothing has ever fired on this key; a tick is a no-op. Seed a sane empty row
    // so the caller can persist uniformly.
    let Some(mut st) = state else {
        return (NotifyState::default_for("", "", now), Vec::new());
    };

    let mut deliveries = Vec::new();
    let pinned = throttle_override.is_some();

    // Elapse whole windows one at a time so multi-window gaps decay correctly (a key quiet for
    // three daily windows decays three levels). Bounded by the elapsed span / window size.
    loop {
        let window = window_for(st.level, policy).max(1);
        if now.saturating_sub(st.window_start) < window {
            break;
        }
        if st.pending.count > 0 {
            if deliver_allowed {
                deliveries.push(Delivery {
                    sub_id: st.sub_id.clone(),
                    insight_id: String::new(),
                    dedup_key: st.dedup_key.clone(),
                    reason: DeliveryReason::Digest,
                    severity: st.pending.max_severity.unwrap_or(Severity::Info),
                    ts: now,
                });
                st.last_sent_ts = Some(now);
            }
            st.pending = WindowAccumulator::default();
            // A window that HAD pending is not "fully quiet" — advance the window but don't decay.
            st.window_start = st.window_start.saturating_add(window);
            st.window_hits = 0;
        } else {
            // Fully quiet window ⇒ decay one level (unless pinned).
            if !pinned && st.level > 0 {
                st.level -= 1;
            }
            st.window_start = st.window_start.saturating_add(window);
            st.window_hits = 0;
        }
    }
    (st, deliveries)
}

/// Advance one input of kind [`LadderInput::Intent`] — a matched raise arrived. Breakthrough is
/// checked FIRST (a genuinely-new fact always delivers, keeping the level); otherwise ack
/// suppression, then escalate accounting, then the L0-immediate / L1+-accumulate delivery choice.
fn intent_step(
    state: Option<NotifyState>,
    intent: &Intent,
    acked: bool,
    policy: &Policy,
    throttle_override: Option<crate::policy::ThrottleOverride>,
    deliver_allowed: bool,
    now: u64,
) -> (NotifyState, Vec<Delivery>) {
    let first_key = state.is_none();
    let mut st =
        state.unwrap_or_else(|| NotifyState::default_for(&intent.sub_id, &intent.dedup_key, now));
    let pinned = throttle_override.is_some();

    // Every intent counts toward the window + the pending accumulator (accounting is never gated).
    st.window_hits += 1;
    record_pending(&mut st, intent.severity, now);

    // --- Breakthrough (checked first, regardless of level / override) --------------------------
    let escalation = st
        .last_severity
        .map(|prev| intent.severity.rank() > prev.rank())
        .unwrap_or(false);
    let breakthrough_reason = if first_key {
        Some(DeliveryReason::FirstKey)
    } else if intent.kind == IntentKind::Reopen {
        Some(DeliveryReason::Reopen)
    } else if intent.kind == IntentKind::Escalate || escalation {
        Some(DeliveryReason::Escalation)
    } else {
        None
    };

    st.last_severity = Some(intent.severity);

    if let Some(reason) = breakthrough_reason {
        // A breakthrough delivers now and KEEPS the level (the noise history stands). It consumes
        // the pending it just recorded (it IS the delivery), so the next digest doesn't double it.
        let mut deliveries = Vec::new();
        if deliver_allowed {
            deliveries.push(delivery(&st, intent, reason, now));
            st.last_sent_ts = Some(now);
        }
        st.pending = WindowAccumulator::default();
        return (st, deliveries);
    }

    // --- Ack suppression -----------------------------------------------------------------------
    // An acked insight (and not a breakthrough) accumulates but never delivers.
    if acked {
        return (st, Vec::new());
    }

    // --- Escalate ------------------------------------------------------------------------------
    // Sustained noise within the window climbs the ladder (unless pinned). At/after the threshold,
    // reset the window so escalation is per-window, not cumulative-forever.
    if !pinned && st.window_hits >= policy.escalation_threshold && st.level < 4 {
        st.level += 1;
        st.window_hits = 0;
        st.window_start = now;
    }

    // --- Deliver choice ------------------------------------------------------------------------
    let mut deliveries = Vec::new();
    if st.level == 0 {
        // L0: one immediate post per cooldown per key; extra raises within the cooldown accumulate
        // into pending for the next post.
        let cooled = match st.last_sent_ts {
            None => true, // never delivered on this key ⇒ post the first one now
            Some(prev) => now.saturating_sub(prev) >= policy.cooldown,
        };
        if cooled {
            if deliver_allowed {
                deliveries.push(delivery(&st, intent, DeliveryReason::L0Immediate, now));
                st.last_sent_ts = Some(now);
            }
            // The immediate post consumes the pending accumulated this cooldown.
            st.pending = WindowAccumulator::default();
        }
        // else: still within cooldown ⇒ leave it in pending for the next eligible post.
    }
    // L1..L4: no immediate delivery — the reactor's Tick digests the pending.

    (st, deliveries)
}

/// Fold a firing into the pending accumulator (count, first/last ts, worst severity).
fn record_pending(st: &mut NotifyState, severity: Severity, now: u64) {
    if st.pending.count == 0 {
        st.pending.first_ts = now;
    }
    st.pending.count += 1;
    st.pending.last_ts = now;
    st.pending.max_severity = Some(match st.pending.max_severity {
        Some(prev) => prev.max(severity),
        None => severity,
    });
}

/// Build a per-key delivery from the current state + the triggering intent.
fn delivery(st: &NotifyState, intent: &Intent, reason: DeliveryReason, now: u64) -> Delivery {
    Delivery {
        sub_id: st.sub_id.clone(),
        insight_id: intent.insight_id.clone(),
        dedup_key: st.dedup_key.clone(),
        reason,
        severity: intent.severity,
        ts: now,
    }
}
