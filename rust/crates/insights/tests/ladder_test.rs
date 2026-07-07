//! The pure ladder state machine — `lb_insights::ladder_step` — driven directly with NO I/O
//! (insight-notify-scope.md). Deterministic: the same intent sequence + clock always yields the
//! same deliveries (the scope's determinism test). Real backend (rule 9) is not needed for THIS
//! surface — it's a pure function over `(state, intent|tick, policy, now)`. The integration
//! ladder headline (raise ×10 through the real raise path) lives in `host/tests/insights_test.rs`.

use lb_insights::{
    ladder_step, policy_defaults, Delivery, DeliveryReason, Intent, IntentKind, LadderInput,
    NotifyState, Policy, Severity,
};

/// A minimal intent fixture (the dedup-key/sub identity is arbitrary for the pure fn).
fn intent(kind: IntentKind, severity: Severity, _ts: u64) -> Intent {
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

/// Feed one Intent, delivery allowed (kill switch on, not muted, no throttle pin).
fn step_intent(
    state: Option<NotifyState>,
    kind: IntentKind,
    sev: Severity,
    acked: bool,
    now: u64,
) -> (NotifyState, Vec<Delivery>) {
    ladder_step(
        state,
        LadderInput::Intent {
            intent: &intent(kind, sev, now),
            acked,
            now,
        },
        &policy(),
        None,
        false,
        true,
    )
}

// --- breakthroughs (checked FIRST, regardless of level) ----------------------------------

#[test]
fn first_key_breaks_through_immediately() {
    // No prior state row + a Raise intent ⇒ deliver now (FirstKey), keep the level at L0.
    let (next, deliveries) = step_intent(None, IntentKind::Raise, Severity::Warning, false, 1);
    assert_eq!(deliveries.len(), 1);
    assert_eq!(deliveries[0].reason, DeliveryReason::FirstKey);
    assert_eq!(next.level, 0);
}

#[test]
fn severity_escalation_breaks_through_at_any_level() {
    // At L2 (daily) + last_severity = warning, a critical raise delivers immediately; level
    // UNCHANGED (a breakthrough doesn't reset the ladder).
    let mut state = NotifyState::default_for("sub-1", "key-1", 0);
    state.level = 2;
    state.last_severity = Some(Severity::Warning);
    let (next, deliveries) = step_intent(
        Some(state),
        IntentKind::Raise,
        Severity::Critical,
        false,
        100,
    );
    assert_eq!(deliveries.len(), 1);
    assert_eq!(deliveries[0].reason, DeliveryReason::Escalation);
    assert_eq!(next.level, 2, "breakthrough keeps the level");
}

#[test]
fn reopen_always_breaks_through() {
    let mut state = NotifyState::default_for("sub-1", "key-1", 0);
    state.level = 3;
    state.last_severity = Some(Severity::Critical);
    let (_next, deliveries) = step_intent(
        Some(state),
        IntentKind::Reopen,
        Severity::Critical,
        false,
        100,
    );
    assert_eq!(deliveries.len(), 1);
    assert_eq!(deliveries[0].reason, DeliveryReason::Reopen);
}

#[test]
fn same_severity_raise_does_not_break_through() {
    // At L1+, a same-severity (non-reopen) raise does NOT deliver immediately; it accumulates.
    let mut state = NotifyState::default_for("sub-1", "key-1", 0);
    state.level = 1;
    state.last_severity = Some(Severity::Warning);
    let (next, deliveries) = step_intent(
        Some(state),
        IntentKind::Raise,
        Severity::Warning,
        false,
        100,
    );
    assert!(
        deliveries.is_empty(),
        "no breakthrough at L1 for same severity"
    );
    assert_eq!(next.pending.count, 1, "accumulated into pending");
}

// --- escalate / decay --------------------------------------------------------------------

#[test]
fn escalate_after_threshold_within_window() {
    // ≥ escalation_threshold (default 3) raises within the window ⇒ level + 1. First raise is a
    // first-key breakthrough at L0; the 2nd/3rd within cooldown accumulate then escalate.
    let mut state = None;
    let mut last = NotifyState::default_for("sub-1", "key-1", 0);
    for i in 1..=3u64 {
        let (next, _d) = step_intent(state.take(), IntentKind::Raise, Severity::Warning, false, i);
        last = next;
        state = Some(last.clone());
    }
    assert_eq!(last.level, 1, "3 hits in the window escalated L0 -> L1");
}

#[test]
fn decay_after_one_fully_quiet_window() {
    // A row at L2 with NO pending, ticked past one window ⇒ level - 1.
    let mut state = NotifyState::default_for("sub-1", "key-1", 0);
    state.level = 2;
    let pol = policy();
    let window = pol.windows[2];
    let (next, deliveries) = ladder_step(
        Some(state),
        LadderInput::Tick { now: window + 1 },
        &pol,
        None,
        false,
        true,
    );
    assert!(deliveries.is_empty(), "quiet window emits no digest");
    assert_eq!(next.level, 1, "one quiet window decayed L2 -> L1");
}

#[test]
fn l0_cooldown_accumulates_extras_into_pending() {
    // At L0, one immediate post per cooldown; extra raises within the cooldown accumulate. Use
    // Reopen/first-key only once — after the first immediate, further same-severity raises inside
    // the cooldown go to pending (and would escalate at the threshold, so keep under it).
    let mut pol = policy();
    pol.escalation_threshold = 100; // isolate the cooldown behaviour from escalation
    let (s1, d1) = ladder_step(
        None,
        LadderInput::Intent {
            intent: &intent(IntentKind::Raise, Severity::Info, 0),
            acked: false,
            now: 0,
        },
        &pol,
        None,
        false,
        true,
    );
    assert_eq!(d1.len(), 1, "first raise posts immediately");
    // Second raise 1 minute later (within the 15-min cooldown) ⇒ no post, accumulates.
    let (s2, d2) = ladder_step(
        Some(s1),
        LadderInput::Intent {
            intent: &intent(IntentKind::Raise, Severity::Info, 60_000),
            acked: false,
            now: 60_000,
        },
        &pol,
        None,
        false,
        true,
    );
    assert!(d2.is_empty(), "within cooldown ⇒ no immediate post");
    assert_eq!(s2.pending.count, 1, "the extra accumulated into pending");
}

// --- ack suppression ---------------------------------------------------------------------

#[test]
fn acked_insight_suppresses_delivery_but_accounting_continues() {
    // A non-breakthrough raise on an acked insight at L0 ⇒ no delivery, but pending advances.
    let mut state = NotifyState::default_for("sub-1", "key-1", 0);
    state.last_severity = Some(Severity::Warning);
    let (next, deliveries) =
        step_intent(Some(state), IntentKind::Raise, Severity::Warning, true, 100);
    assert!(deliveries.is_empty(), "acked suppresses delivery");
    assert_eq!(next.pending.count, 1, "accounting continues under ack");

    // But a critical escalation still breaks through the ack.
    let mut escalating = NotifyState::default_for("sub-1", "key-1", 0);
    escalating.last_severity = Some(Severity::Warning);
    let (_n, d2) = step_intent(
        Some(escalating),
        IntentKind::Raise,
        Severity::Critical,
        true,
        101,
    );
    assert_eq!(d2.len(), 1, "escalation un-suppresses even under ack");
}

// --- throttle override (pinned sub) ------------------------------------------------------

#[test]
fn throttle_override_skips_escalate_and_decay_but_keeps_breakthroughs() {
    use lb_insights::ThrottleOverride;
    let pol = policy();
    // Pinned at Daily (L2). Sustained noise must NOT escalate past L2.
    let mut state = Some(NotifyState::default_for("sub-1", "key-1", 0));
    state.as_mut().unwrap().level = 2;
    state.as_mut().unwrap().last_severity = Some(Severity::Warning);
    for i in 1..=5u64 {
        let (next, _d) = ladder_step(
            state.take(),
            LadderInput::Intent {
                intent: &intent(IntentKind::Raise, Severity::Warning, i),
                acked: false,
                now: i,
            },
            &pol,
            Some(ThrottleOverride::Daily),
            false,
            true,
        );
        state = Some(next);
    }
    assert_eq!(state.unwrap().level, 2, "pinned level never escalates");

    // Decay is skipped too: a quiet window at a pinned level stays put.
    let mut quiet = NotifyState::default_for("sub-1", "key-1", 0);
    quiet.level = 2;
    let (after_tick, _d) = ladder_step(
        Some(quiet),
        LadderInput::Tick {
            now: pol.windows[2] + 1,
        },
        &pol,
        Some(ThrottleOverride::Daily),
        false,
        true,
    );
    assert_eq!(after_tick.level, 2, "pinned level never decays");
}

// --- determinism -------------------------------------------------------------------------

#[test]
fn same_input_sequence_and_clock_yield_the_same_deliveries() {
    let run = || {
        let mut state: Option<NotifyState> = None;
        let mut all = Vec::new();
        for (i, kind, sev) in [
            (1u64, IntentKind::Raise, Severity::Warning),
            (2, IntentKind::Raise, Severity::Warning),
            (3, IntentKind::Raise, Severity::Critical),
            (4, IntentKind::Reopen, Severity::Critical),
        ] {
            let (next, d) = step_intent(state.take(), kind, sev, false, i);
            all.extend(d);
            state = Some(next);
        }
        (state.unwrap(), all)
    };
    let a = run();
    let b = run();
    assert_eq!(a, b, "the pure state machine is deterministic");
}
