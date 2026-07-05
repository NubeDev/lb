//! The pure ladder state machine — `lb_insights::ladder_step` — driven directly with NO I/O
//! (insight-notify-scope.md). Deterministic: the same intent sequence + clock always yields the
//! same deliveries (the scope's determinism test). Real backend (rule 9) is not needed for THIS
//! surface — it's a pure function over `(state, intent|tick, policy, now)`. The integration
//! ladder headline (raise ×10 through the real raise path) lives in `host/tests/insights_test.rs`.
//!
//! **SKELETON**: every test is NAMED for a scope-named case + uses `ladder_step` directly.
//! Bodies are `todo!()` so a green-but-lying stub is impossible. The implementing session fills
//! them against `docs/scope/insights/insight-notify-scope.md §"The state machine"`.

use lb_insights::{
    ladder_step, policy_defaults, DeliveryReason, Intent, IntentKind, LadderInput, NotifyState,
    Policy, Severity,
};

/// A minimal intent fixture (the dedup-key/sub identity is arbitrary for the pure fn).
fn intent(kind: IntentKind, severity: Severity, ts: u64) -> Intent {
    Intent {
        sub_id: "sub-1".into(),
        insight_id: "ins-1".into(),
        dedup_key: "key-1".into(),
        severity,
        kind,
    }
}

/// The default policy (compiled `defaults()`).
fn policy() -> Policy {
    policy_defaults()
}

// --- breakthroughs (checked FIRST, regardless of level) ----------------------------------

#[test]
fn first_key_breaks_through_immediately() {
    // SCOPE: notify-scope.md §"The state machine" → Breakthrough. No prior state row + a Raise
    // intent ⇒ deliver now (FirstKey), keep the level at L0.
    let _state: Option<NotifyState> = None;
    let _input = LadderInput::Intent {
        intent: &intent(IntentKind::Raise, Severity::Warning, 1),
        acked: false,
        now: 1,
    };
    let _policy = policy();
    let (_next, deliveries) = ladder_step(_state, _input, &_policy, None, false, true);
    let _ = deliveries;
    // assert deliveries.len() == 1 + reason == FirstKey + level stays L0.
    todo!("insights: first-key breakthrough — SCOPE: notify-scope.md §The state machine")
}

#[test]
fn severity_escalation_breaks_through_at_any_level() {
    // SCOPE: notify-scope.md §"Breakthroughs beat the ladder". At L2 (daily) + last_severity =
    // warning, a critical intent delivers immediately; level UNCHANGED (a breakthrough doesn't
    // reset the ladder).
    todo!("insights: severity-escalation breakthrough — SCOPE: notify-scope.md §The state machine")
}

#[test]
fn reopen_always_breaks_through() {
    // SCOPE: notify-scope.md §"Breakthroughs beat the ladder". A Reopen intent delivers at any
    // level, regardless of cooldown / window.
    todo!("insights: reopen breakthrough — SCOPE: notify-scope.md §The state machine")
}

#[test]
fn same_severity_raise_does_not_break_through() {
    // SCOPE: notify-scope.md — a same-severity (non-reopen) raise at L1+ does NOT deliver
    // immediately; it accumulates into `pending`.
    todo!("insights: same-severity raise accumulates, no breakthrough — SCOPE: notify-scope.md §The state machine")
}

// --- escalate / decay --------------------------------------------------------------------

#[test]
fn escalate_after_threshold_within_window() {
    // SCOPE: notify-scope.md §"Escalate". ≥ escalation_threshold deliveries-worth of noise within
    // the current window ⇒ level + 1 (clamped at 4).
    todo!("insights: escalate at threshold — SCOPE: notify-scope.md §The state machine")
}

#[test]
fn decay_after_one_fully_quiet_window() {
    // SCOPE: notify-scope.md §"Decay". One fully-quiet window at the current level ⇒ level - 1
    // (clamped at 0).
    todo!("insights: decay after one quiet window — SCOPE: notify-scope.md §The state machine")
}

#[test]
fn l0_cooldown_accumulates_extras_into_pending() {
    // SCOPE: notify-scope.md §"L0 immediate". At L0, one immediate post per cooldown per key;
    // extra raises within the cooldown accumulate into `pending` for the next post.
    todo!("insights: L0 cooldown accumulation — SCOPE: notify-scope.md §The state machine")
}

// --- ack suppression ---------------------------------------------------------------------

#[test]
fn acked_insight_suppresses_delivery_but_accounting_continues() {
    // SCOPE: notify-scope.md §"Ack means 'I know'". Intents for an acked insight update
    // pending/window_hits but never deliver; escalation/re-open still break through.
    todo!("insights: ack suppression — SCOPE: notify-scope.md §The state machine")
}

// --- throttle override (pinned sub) ------------------------------------------------------

#[test]
fn throttle_override_skips_escalate_and_decay_but_keeps_breakthroughs() {
    // SCOPE: notify-scope.md §"Per-sub — throttle_override". A pinned sub skips escalate/decay;
    // breakthroughs + ack-suppression still apply.
    todo!("insights: throttle override pins level — SCOPE: notify-scope.md §Settings surface")
}

// --- determinism -------------------------------------------------------------------------

#[test]
fn same_input_sequence_and_clock_yield_the_same_deliveries() {
    // SCOPE: notify-scope.md §"Determinism". The state machine is a pure function — running the
    // same intent sequence + clock twice yields byte-identical deliveries.
    todo!("insights: determinism — SCOPE: notify-scope.md §Determinism")
}
